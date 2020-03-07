// #![cfg(windows)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, RwLock};

use hashbrown::HashMap;
use indexmap::IndexMap;
use pahkat_types::payload::windows::InstallTarget;
use url::Url;
use winreg::enums::*;
use winreg::RegKey;

use crate::transaction::{
    install::InstallError, install::ProcessError, uninstall::UninstallError, PackageStatus,
    PackageStatusError,
};
use crate::Config;
use crate::{repo::PayloadError, LoadedRepository, PackageKey, PackageStore};
use pahkat_types::{
    package::{Descriptor, Package},
    payload::windows,
};

mod sys {
    use std::ffi::{OsStr, OsString};
    use std::ops::Range;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::ffi::OsStringExt;
    use std::slice;
    use winapi::ctypes::c_void;
    use winapi::um::shellapi::CommandLineToArgvW;
    use winapi::um::winbase::LocalFree;

    // https://github.com/rust-lang/rust/blob/f76d9bcfc2c269452522fbbe19f66fe653325646/src/libstd/sys/windows/os.rs#L286-L289
    pub struct Args {
        range: Range<isize>,
        cur: *mut *mut u16,
    }

    impl Iterator for Args {
        type Item = OsString;
        fn next(&mut self) -> Option<OsString> {
            self.range.next().map(|i| unsafe {
                let ptr = *self.cur.offset(i);
                let mut len = 0;
                while *ptr.offset(len) != 0 {
                    len += 1;
                }

                // Push it onto the list.
                let ptr = ptr as *const u16;
                let buf = slice::from_raw_parts(ptr, len as usize);
                OsStringExt::from_wide(buf)
            })
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            self.range.size_hint()
        }
    }

    impl ExactSizeIterator for Args {
        fn len(&self) -> usize {
            self.range.len()
        }
    }

    impl Drop for Args {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.cur as *mut c_void);
            }
        }
    }

    pub fn args<S: AsRef<OsStr>>(input: S) -> Args {
        let input_vec: Vec<u16> = OsStr::new(&input)
            .encode_wide()
            .chain(Some(0).into_iter())
            .collect();
        let lp_cmd_line = input_vec.as_ptr();
        let mut args: i32 = 0;
        let arg_list: *mut *mut u16 = unsafe { CommandLineToArgvW(lp_cmd_line, &mut args) };
        Args {
            range: 0..(args as isize),
            cur: arg_list,
        }
    }
}

mod keys {
    pub const UNINSTALL_PATH: &'static str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
    pub const DISPLAY_VERSION: &'static str = "DisplayVersion";
    pub const QUIET_UNINSTALL_STRING: &'static str = "QuietUninstallString";
}

type SharedStoreConfig = Arc<RwLock<Config>>;
type SharedRepos = Arc<RwLock<HashMap<Url, LoadedRepository>>>;

#[derive(Debug)]
pub struct WindowsPackageStore {
    repos: SharedRepos,
    config: SharedStoreConfig,
}

fn uninstall_regkey(installer: &windows::Executable) -> Option<RegKey> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = Path::new(keys::UNINSTALL_PATH).join(&installer.product_code);
    match hklm.open_subkey(&path) {
        Err(_e) => match hklm.open_subkey_with_flags(&path, KEY_READ | KEY_WOW64_64KEY) {
            Err(_e) => None,
            Ok(v) => Some(v),
        },
        Ok(v) => Some(v),
    }
}

impl PackageStore for WindowsPackageStore {
    type Target = InstallTarget;

    fn config(&self) -> SharedStoreConfig {
        Arc::clone(&self.config)
    }

    fn repos(&self) -> SharedRepos {
        Arc::clone(&self.repos)
    }

    fn download(
        &self,
        key: &PackageKey,
        progress: Box<dyn Fn(u64, u64) -> bool + Send + 'static>,
    ) -> Result<PathBuf, crate::download::DownloadError> {
        let query = crate::repo::ReleaseQuery::from(key);
        let repos = self.repos.read().unwrap();
        crate::repo::download(&self.config, key, query, &*repos, progress)
    }

    fn install(
        &self,
        key: &PackageKey,
        install_target: &Self::Target,
    ) -> Result<PackageStatus, InstallError> {
        let query = crate::repo::ReleaseQuery::from(key);
        let repos = self.repos.read().unwrap();

        let (target, release, descriptor) =
            crate::repo::resolve_payload(key, query, &*repos).map_err(InstallError::Payload)?;
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
        install_target: &Self::Target,
    ) -> Result<PackageStatus, UninstallError> {
        let query = crate::repo::ReleaseQuery::from(key);
        let repos = self.repos.read().unwrap();

        let (target, release, descriptor) =
            crate::repo::resolve_payload(key, query, &*repos).map_err(UninstallError::Payload)?;
        let installer = match target.payload {
            pahkat_types::payload::Payload::WindowsExecutable(v) => v,
            _ => return Err(UninstallError::WrongPayloadType),
        };

        let regkey = match uninstall_regkey(&installer) {
            Some(v) => v,
            None => return Err(UninstallError::NotInstalled),
        };

        let uninst_string: String = match regkey
            .get_value(keys::QUIET_UNINSTALL_STRING)
            .or_else(|_| regkey.get_value(keys::QUIET_UNINSTALL_STRING))
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
        install_target: &InstallTarget,
    ) -> Result<PackageStatus, PackageStatusError> {
        log::debug!("status: {}, target: {:?}", &key.to_string(), install_target);

        let query = crate::repo::ReleaseQuery::from(key);
        let repos = self.repos.read().unwrap();

        let (target, release, descriptor) = crate::repo::resolve_payload(key, query, &*repos)
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

    fn refresh_repos(&self) {
        *self.repos.write().unwrap() = crate::repo::refresh_repos(&self.config);
    }

    fn clear_cache(&self) {
        crate::repo::clear_cache(&self.config)
    }

    fn import(
        &self,
        key: &PackageKey,
        installer_path: &Path,
    ) -> Result<PathBuf, crate::package_store::ImportError> {
        unimplemented!()
    }

    fn all_statuses(
        &self,
        repo_url: &Url,
        target: &Self::Target,
    ) -> BTreeMap<String, Result<PackageStatus, PackageStatusError>> {
        unimplemented!()
    }
}

use std::convert::{TryFrom, TryInto};

impl WindowsPackageStore {
    pub fn new(config: Config) -> WindowsPackageStore {
        let store = WindowsPackageStore {
            repos: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(config)),
        };

        store.refresh_repos();

        store
    }

    pub fn config(&self) -> SharedStoreConfig {
        Arc::clone(&self.config)
    }

    fn status_impl(
        &self,
        id: &PackageKey,
        package: &Descriptor,
        _target: &InstallTarget,
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

        let disp_version: String = match inst_key.get_value(keys::DISPLAY_VERSION) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v,
        };

        let status = crate::cmp::cmp(&disp_version, &response.release.version);

        log::debug!("Status: {:?}", &status);
        status
    }
}
