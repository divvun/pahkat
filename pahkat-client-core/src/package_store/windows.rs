mod sys;

use std::collections::BTreeMap;
use std::convert::{TryFrom, TryInto};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, RwLock};

use dashmap::DashMap;
use hashbrown::HashMap;
use url::Url;
use winreg::enums::*;
use winreg::RegKey;

use crate::package_store::{ImportError, InstallTarget};
use crate::repo::{PackageQuery, RepoDownloadError};
use crate::transaction::{
    install::InstallError, install::ProcessError, uninstall::UninstallError, PackageStatus,
    PackageStatusError, ResolvedDescriptor, ResolvedPackageQuery,
};
use crate::Config;
use crate::{repo::PayloadError, LoadedRepository, PackageKey, PackageStore};
use pahkat_types::{
    package::{Descriptor, Package},
    payload::windows,
};

const UNINSTALL_PATH: &'static str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
const DISPLAY_VERSION: &'static str = "DisplayVersion";
const QUIET_UNINSTALL_STRING: &'static str = "QuietUninstallString";

use super::LocalizedStrings;
use super::{SharedRepos, SharedRepoErrors, SharedStoreConfig};

#[derive(Debug)]
pub struct WindowsPackageStore {
    repos: SharedRepos,
    errors: SharedRepoErrors,
    config: SharedStoreConfig,
}

fn uninstall_regkey(installer: &windows::Executable) -> Option<RegKey> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = Path::new(UNINSTALL_PATH).join(&installer.product_code);
    match hklm.open_subkey(&path) {
        Err(_e) => match hklm.open_subkey_with_flags(&path, KEY_READ | KEY_WOW64_64KEY) {
            Err(_e) => None,
            Ok(v) => Some(v),
        },
        Ok(v) => Some(v),
    }
}

impl PackageStore for WindowsPackageStore {
    fn config(&self) -> SharedStoreConfig {
        Arc::clone(&self.config)
    }

    fn errors(&self) -> super::SharedRepoErrors {
        Arc::clone(&self.errors)
    }

    fn repos(&self) -> SharedRepos {
        Arc::clone(&self.repos)
    }

    fn download(
        &self,
        key: &PackageKey,
    ) -> std::pin::Pin<
        Box<
            dyn futures::stream::Stream<Item = crate::package_store::DownloadEvent>
                + Send
                + Sync
                + 'static,
        >,
    > {
        let repos = self.repos.read().unwrap();
        let query = crate::repo::ReleaseQuery::new(key, &*repos);
        crate::repo::download(&self.config, key, &query, &*repos)
    }

    fn install(
        &self,
        key: &PackageKey,
        install_target: InstallTarget,
    ) -> Result<PackageStatus, InstallError> {
        let repos = self.repos.read().unwrap();
        let query = crate::repo::ReleaseQuery::new(key, &*repos);

        let (target, release, descriptor) =
            crate::repo::resolve_payload(key, &query, &*repos).map_err(InstallError::Payload)?;
        let installer = match target.payload {
            pahkat_types::payload::Payload::WindowsExecutable(v) => v,
            _ => return Err(InstallError::WrongPayloadType),
        };
        let pkg_path =
            crate::repo::download_file_path(&*self.config.read().unwrap(), &installer.url);
        log::debug!("Installing {}: {:?}", &key, &pkg_path);

        if !pkg_path.exists() {
            log::error!("Package path doesn't exist: {:?}", &pkg_path);
            return Err(InstallError::PackageNotInCache);
        }

        let mut args: Vec<OsString> = match (&installer.kind, &installer.args) {
            (_, &Some(ref v)) => sys::args(&v).map(|x| x.clone()).collect(),
            (&Some(ref type_), &None) => {
                let mut arg_str = OsString::new();
                // TODO: generic parameter extensions for windows based on install target
                match type_.as_ref() {
                    "inno" => {
                        arg_str.push("\"");
                        arg_str.push(&pkg_path);
                        arg_str.push("\" /VERYSILENT /SP- /SUPPRESSMSGBOXES /NORESTART");
                        // TODO: add user-mode installation?
                    }
                    "msi" => {
                        arg_str.push("msiexec /i \"");
                        arg_str.push(&pkg_path);
                        arg_str.push("\" /qn /norestart");
                    }
                    "nsis" => {
                        arg_str.push("\"");
                        arg_str.push(&pkg_path);
                        arg_str.push("\" /S");
                        // if target == InstallTarget::User {
                        //     arg_str.push(" /CurrentUser")
                        // }
                    }
                    kind => {
                        log::warn!("Unknown kind: {:?}", &kind);
                    }
                };
                sys::args(&arg_str.as_os_str()).collect()
            }
            _ => sys::args(&OsString::from(pkg_path)).collect(),
        };
        log::debug!("{:?}", &args);
        let prog = args[0].clone();
        args.remove(0);

        // log::debug!("Cmd line: {:?} {:?}", &pkg_path, &args);

        let res = Command::new(&prog).args(&args).output();

        let output = match res {
            Ok(v) => v,
            Err(e) => {
                log::error!("{:?}", e);
                return Err(InstallError::InstallerFailure(ProcessError::Io(e)));
            }
        };

        if !output.status.success() {
            log::error!("{:?}", output);
            return Err(InstallError::InstallerFailure(ProcessError::Unknown(
                output,
            )));
        }

        Ok(self.status_impl(key, &descriptor, install_target).unwrap())
    }

    fn uninstall(
        &self,
        key: &PackageKey,
        install_target: InstallTarget,
    ) -> Result<PackageStatus, UninstallError> {
        let repos = self.repos.read().unwrap();
        let query = crate::repo::ReleaseQuery::new(key, &*repos);

        let (target, release, descriptor) =
            crate::repo::resolve_payload(key, &query, &*repos).map_err(UninstallError::Payload)?;
        let installer = match target.payload {
            pahkat_types::payload::Payload::WindowsExecutable(v) => v,
            _ => return Err(UninstallError::WrongPayloadType),
        };

        let regkey = match uninstall_regkey(&installer) {
            Some(v) => v,
            None => return Err(UninstallError::NotInstalled),
        };

        let uninst_string: String = match regkey
            .get_value(QUIET_UNINSTALL_STRING)
            .or_else(|_| regkey.get_value(QUIET_UNINSTALL_STRING))
        {
            Ok(v) => v,
            Err(_) => {
                return Err(UninstallError::Payload(PayloadError::CriteriaUnmet(
                    "No compatible uninstallation method found.".into(),
                )))
            }
        };

        let mut raw_args: Vec<OsString> = sys::args(&uninst_string).map(|x| x.clone()).collect();
        let prog = raw_args[0].clone();
        raw_args.remove(0);

        let args: Vec<OsString> = match (&installer.kind, &installer.uninstall_args) {
            (_, &Some(ref v)) => sys::args(&v).map(|x| x.clone()).collect(),
            (&Some(ref type_), &None) => {
                let arg_str = match type_.as_ref() {
                    "inno" => "/VERYSILENT /SP- /SUPPRESSMSGBOXES /NORESTART".to_owned(),
                    "msi" => format!("/x \"{}\" /qn /norestart", &installer.product_code),
                    "nsis" => "/S".to_owned(),
                    _ => {
                        return Err(UninstallError::Payload(PayloadError::CriteriaUnmet(
                            "Invalid type specified for package installer.".into(),
                        )))
                    }
                };
                sys::args(&arg_str).collect()
            }
            _ => {
                return Err(UninstallError::Payload(PayloadError::CriteriaUnmet(
                    "Invalid type specified for package installer.".into(),
                )))
            }
        };

        let res = Command::new(&prog).args(&args).output();

        let output = match res {
            Ok(v) => v,
            Err(e) => {
                log::error!("{:?}", e);
                return Err(UninstallError::UninstallerFailure(ProcessError::Io(e)));
            }
        };

        if !output.status.success() {
            log::error!("{:?}", output);
            return Err(UninstallError::UninstallerFailure(ProcessError::Unknown(
                output,
            )));
        }

        Ok(self.status_impl(key, &descriptor, install_target).unwrap())
    }

    fn status(
        &self,
        key: &PackageKey,
        install_target: InstallTarget,
    ) -> Result<PackageStatus, PackageStatusError> {
        log::debug!("status: {}, target: {:?}", &key.to_string(), install_target);

        let repos = self.repos.read().unwrap();
        let query = crate::repo::ReleaseQuery::new(key, &*repos);

        let (target, release, descriptor) = crate::repo::resolve_payload(key, &query, &*repos)
            .map_err(PackageStatusError::Payload)?;
        let installer = match target.payload {
            pahkat_types::payload::Payload::WindowsExecutable(v) => v,
            _ => return Err(PackageStatusError::WrongPayloadType),
        };

        self.status_impl(key, &descriptor, install_target)
    }

    fn find_package_by_key(&self, key: &PackageKey) -> Option<Package> {
        let repos = self.repos.read().unwrap();
        crate::repo::find_package_by_key(key, &*repos)
    }

    fn find_package_by_id(&self, package_id: &str) -> Option<(PackageKey, Package)> {
        let repos = self.repos.read().unwrap();
        crate::repo::find_package_by_id(self, package_id, &*repos)
    }

    fn refresh_repos(&self) -> crate::package_store::Future<Result<(), HashMap<Url, RepoDownloadError>>> {
        let config = self.config().read().unwrap().clone();
        let repos = self.repos();
        Box::pin(async move {
            let (result, errors) = crate::repo::refresh_repos(config).await;
            *repos.write().unwrap() = result;
            if errors.is_empty() {
                Ok(())
            } else {
                Err(errors)
            }
        })
    }

    fn clear_cache(&self) {
        crate::repo::clear_cache(&self.config)
    }

    fn import(&self, key: &PackageKey, installer_path: &Path) -> Result<PathBuf, ImportError> {
        let repos = self.repos.read().unwrap();
        let query = crate::repo::ReleaseQuery::new(key, &*repos);
        crate::repo::import(&self.config, key, &query, &*repos, installer_path)
    }

    fn all_statuses(
        &self,
        repo_url: &Url,
        target: InstallTarget,
    ) -> BTreeMap<String, Result<PackageStatus, PackageStatusError>> {
        crate::repo::all_statuses(self, repo_url, target)
    }

    fn strings(
        &self,
        language: String,
    ) -> crate::package_store::Future<HashMap<Url, LocalizedStrings>> {
        let repos = self.repos.read().unwrap();
        let urls = repos.keys().cloned().collect::<Vec<_>>();

        Box::pin(crate::repo::strings(urls, language))
    }

    fn resolve_package_query(&self, query: PackageQuery, install_target: &[InstallTarget]) -> ResolvedPackageQuery {
        let repos = self.repos();
        let repos = repos.read().unwrap();
        crate::repo::resolve_package_query(self, &query, install_target, &*repos)
    }
}

impl WindowsPackageStore {
    pub async fn new(config: Config) -> WindowsPackageStore {
        let store = WindowsPackageStore {
            repos: Default::default(),
            errors: Default::default(),
            config: Arc::new(RwLock::new(config)),
        };

        // We ignore errors here.
        let _ = store.refresh_repos().await;

        store
    }

    pub fn config(&self) -> SharedStoreConfig {
        Arc::clone(&self.config)
    }

    fn status_impl(
        &self,
        id: &PackageKey,
        package: &Descriptor,
        _target: InstallTarget,
    ) -> Result<PackageStatus, PackageStatusError> {
        let mut query = crate::repo::ReleaseQuery::default();
        query.arch = None;

        let (response, inst_key) = match query
            .iter(package)
            .filter_map(|x| match x.target.payload {
                pahkat_types::payload::Payload::WindowsExecutable(ref v) => Some((x, v)),
                _ => None,
            })
            .find_map(|(x, v)| uninstall_regkey(&v).map(|i| (x, i)))
        {
            Some(v) => v,
            None => return Ok(PackageStatus::NotInstalled),
        };

        let disp_version: String = match inst_key.get_value(DISPLAY_VERSION) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v,
        };

        let status = crate::cmp::cmp(&disp_version, &response.release.version);

        log::debug!("Status: {:?}", &status);
        status
    }
}
