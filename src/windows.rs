#![cfg(windows)]
use pahkat::types::{
    WindowsInstaller,
    Installer,
    Package,
    Downloadable,
    InstallTarget
};
//use {Package, PackageStatus, PackageStatusError, Installer};
use std::path::{PathBuf, Path};
use winreg::RegKey;
use winreg::enums::*;
use semver;
use std::io;
use super::{Repository, StoreConfig};
use std::process::{self, Command};
use std::ffi::{OsString};
use url;
use std::sync::{Arc, RwLock};
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::*;

// pub fn init(url: &str, cache_dir: &str) {
//     let config = StoreConfig { 
//         url: url.to_owned(),
//         cache_dir: cache_dir.to_owned()
//     };
    
//     let config_path = ::default_config_path().join("config.json");
        
//     if config_path.exists() {
//         println!("Path already exists; aborting.");
//         return;
//     }

//     config.save(&config_path).unwrap();
// }

pub struct PackageTransaction {
    store: Arc<WindowsPackageStore>,
    actions: Arc<Vec<PackageAction>>,
    is_cancelled: Arc<AtomicBool>
}

impl PackageTransaction {
    pub fn new(
        store: Arc<WindowsPackageStore>,
        actions: Vec<PackageAction>
     ) -> Result<PackageTransaction, PackageTransactionError> {
        let mut new_actions: Vec<PackageAction> = vec![];

        for action in actions.iter() {
            let package_key = &action.id;

            let package = match store.resolve_package(&package_key) {
                Some(p) => p,
                None => {
                    return Err(PackageTransactionError::NoPackage(package_key.to_string()));
                }
            };

            if action.action == PackageActionType::Install {
                let dependencies = match store.find_package_dependencies(&action.id, &package, action.target) {
                    Ok(d) => d,
                    Err(e) => return Err(PackageTransactionError::Deps(e))
                };

                for dependency in dependencies.into_iter() {
                    let contradiction = actions.iter().find(|action| {
                        dependency.id == action.id && action.action == PackageActionType::Uninstall
                    });
                    match contradiction {
                        Some(a) => {
                            return Err(PackageTransactionError::ActionContradiction(package_key.to_string()))
                        },
                        None => {
                            if !new_actions.iter().any(|x| x.id == dependency.id) {
                                new_actions.push(PackageAction {
                                    id: dependency.id,
                                    action: PackageActionType::Install,
                                    target: action.target
                                })
                            }
                        }
                    }
                }
            }
            if !new_actions.iter().any(|x| x.id == action.id) {
                new_actions.push(action.clone());
            }
        }

        Ok(PackageTransaction {
            store,
            actions: Arc::new(new_actions),
            is_cancelled: Arc::new(AtomicBool::new(false))
        })
    }

    pub fn actions(&self) -> Arc<Vec<PackageAction>> {
        self.actions.clone()
    }

    pub fn validate(&self) -> bool {
        true
    }

    pub fn process<F>(&mut self, progress: F)
    where F: Fn(AbsolutePackageKey, TransactionEvent) -> () + 'static + Send {
        if !self.validate() {
            // TODO: early return
            return;
        }

        let is_cancelled = self.is_cancelled.clone();
        let store = self.store.clone();
        let actions = self.actions.clone();

        let handle = std::thread::spawn(move || {
            for action in actions.iter() {
                if is_cancelled.load(Ordering::Relaxed) == true {
                    return;
                }

                match action.action {
                    PackageActionType::Install => {
                        progress(action.id.clone(), TransactionEvent::Installing);
                        match store.install(&action.id, action.target) {
                            Ok(_) => progress(action.id.clone(), TransactionEvent::Completed),
                            Err(_) => progress(action.id.clone(), TransactionEvent::Error)
                        };
                    },
                    PackageActionType::Uninstall => {
                        progress(action.id.clone(), TransactionEvent::Uninstalling);
                        match store.uninstall(&action.id, action.target) {
                            Ok(_) => progress(action.id.clone(), TransactionEvent::Completed),
                            Err(_) => progress(action.id.clone(), TransactionEvent::Error)
                        };
                    }
                }
            }

            ()
        });

        handle.join();
    }

    pub fn cancel(&self) -> bool {
        // let prev_value = *self.is_cancelled.read().unwrap();
        // *self.is_cancelled.write().unwrap() = true;
        // prev_value
        unimplemented!()
    }
}

mod sys {
    use winapi::um::shellapi::CommandLineToArgvW;
    use winapi::um::winbase::LocalFree;
    use winapi::ctypes::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::ffi::OsStringExt;
    use std::ffi::{OsString, OsStr};
    use std::ops::Range;
    use std::slice;

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
                while *ptr.offset(len) != 0 { len += 1; }

                // Push it onto the list.
                let ptr = ptr as *const u16;
                let buf = slice::from_raw_parts(ptr, len as usize);
                OsStringExt::from_wide(buf)
            })
        }
        fn size_hint(&self) -> (usize, Option<usize>) { self.range.size_hint() }
    }

    impl ExactSizeIterator for Args {
        fn len(&self) -> usize { self.range.len() }
    }

    impl Drop for Args {
        fn drop(&mut self) {
            unsafe { LocalFree(self.cur as *mut c_void); }
        }
    }

    pub fn args<S: AsRef<OsStr>>(input: S) -> Args {
        let input_vec: Vec<u16> = OsStr::new(&input).encode_wide().chain(Some(0).into_iter()).collect();
        let lp_cmd_line = input_vec.as_ptr();
        let mut args: i32 = 0;
        let arg_list: *mut *mut u16 = unsafe { CommandLineToArgvW(lp_cmd_line, &mut args) };
        Args { range: 0..(args as isize), cur: arg_list }
    }
}


#[derive(Debug, Clone, Serialize)]
pub struct PackageAction {
    pub id: AbsolutePackageKey,
    pub action: PackageActionType,
    pub target: InstallTarget
}

mod Keys {
    pub const UninstallPath: &'static str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
    pub const DisplayVersion: &'static str = "DisplayVersion";
    pub const SkipVersion: &'static str = "SkipVersion";
    pub const QuietUninstallString: &'static str = "QuietUninstallString";
    pub const UninstallString: &'static str = "UninstallString";
}

pub struct WindowsPackageStore {
    repos: Arc<RwLock<HashMap<RepoRecord, Repository>>>,
    config: Arc<RwLock<StoreConfig>>
}

fn installer(package: &Package) -> Result<&WindowsInstaller, PackageStatusError> {
    match package.installer() {
        None => Err(PackageStatusError::NoInstaller),
        Some(v) => match v {
            &Installer::Windows(ref v) => Ok(v),
            _ => Err(PackageStatusError::WrongInstallerType)
        }
    }
}

#[derive(Debug)]
pub enum WindowsInstallError {
    NoPackage,
    NoInstaller,
    WrongInstallerType,
    InvalidInstaller,
    InvalidType,
    PackageNotInCache,
    InvalidUrl(String),
    Process(ProcessError),
}

#[derive(Debug)]
pub enum WindowsUninstallError {
    NoPackage,
    NoInstaller,
    WrongInstallerType,
    InvalidInstaller,
    InvalidType,
    NotInstalled,
    NoUninstString,
    Process(ProcessError)
}

#[derive(Debug)]
pub enum ProcessError {
    Io(io::Error),
    Unknown(process::Output)
}

fn uninstall_regkey(installer: &WindowsInstaller) -> Option<RegKey> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = Path::new(Keys::UninstallPath).join(&installer.product_code);
    match hklm.open_subkey(&path) {
        Err(e) => {
            match hklm.open_subkey_with_flags(&path, KEY_READ | KEY_WOW64_64KEY) {
                Err(e) => None,
                Ok(v) => Some(v)
            }
        }
        Ok(v) => Some(v)
    }
}

impl WindowsPackageStore {
    pub fn new(config: StoreConfig) -> WindowsPackageStore {
        let store = WindowsPackageStore {
            repos: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(config))
        };

        store.refresh_repos();

        store
    }

    pub fn config(&self) -> StoreConfig {
        self.config.read().unwrap().clone()
    }

    pub(crate) fn repos_json(&self) -> String {
        serde_json::to_string(&self.repos.read().unwrap().values().collect::<Vec<_>>()).unwrap()
    }
    
    // TODO: review if there is a better place to put this function...
    // fn download_path(&self, url: &str) -> PathBuf {
    //     let mut sha = Sha256::new();
    //     sha.input_str(url);
    //     let hash_id = sha.result_str();
        
    //     self.config.package_cache_path().join(hash_id)
    // }

    fn download_path(&self, url: &str) -> PathBuf {
        let mut sha = Sha256::new();
        sha.input_str(url);
        let hash_id = sha.result_str();
        
        self.config.read().unwrap().package_cache_path().join(hash_id)
    }

    pub fn refresh_repos(&self) {
        let mut repos = HashMap::new();
        let config = self.config.read().unwrap();
        for record in config.repos().iter() {
            match Repository::from_cache_or_url(
                &record.url,
                record.channel.clone(),
                &config.repo_cache_path()
            ) {
                Ok(repo) => { repos.insert(record.clone(), repo); },
                Err(e) => { println!("{:?}", e); }
            };
        }

        *self.repos.write().unwrap() = repos;
    }

    pub fn add_repo(&self, url: String, channel: String) -> Result<(), ()> {
        self.config().add_repo(RepoRecord { url: Url::parse(&url).unwrap(), channel })?;
        self.refresh_repos();
        Ok(())
    }

    pub fn remove_repo(&self, url: String, channel: String) -> Result<(), ()> {
        self.config().remove_repo(RepoRecord { url: Url::parse(&url).unwrap(), channel })?;
        self.refresh_repos();
        Ok(())
    }

    pub fn update_repo(&self, index: usize, url: String, channel: String) -> Result<(), ()> {
        self.config().update_repo(index, RepoRecord { url: Url::parse(&url).unwrap(), channel })?;
        self.refresh_repos();
        Ok(())
    }

    pub fn resolve_package(&self, package_key: &AbsolutePackageKey) -> Option<Package> {
        println!("Resolving package: url: {}, channel: {}", &package_key.url, &package_key.channel);
        for k in self.repos.read().unwrap().keys() {
            println!("{:?}", k);
        }

        self.repos.read().unwrap()
            .get(&RepoRecord {
                url: package_key.url.clone(),
                channel: package_key.channel.clone()
            })
            .and_then(|r| {
                println!("Got repo: {:?}", r);
                for k in r.packages().keys() {
                    println!("Pkg id: {}, {}", &k, k == &package_key.id);
                }

                println!("My pkg id: {}", &package_key.id);
                let pkg = match r.packages().get(&package_key.id) {
                    Some(x) => Some(x.to_owned()),
                    None => None
                };
                println!("Found pkg: {:?}", &pkg);
                pkg
            })
    }

    pub fn find_package(&self, package_id: &str) -> Option<(AbsolutePackageKey, Package)> {
        self.repos.read().unwrap().iter()
            .find_map(|(key, repo)| {
                repo.packages()
                    .get(package_id)
                    .map(|x| (AbsolutePackageKey::new(repo.meta(), &key.channel, package_id), x.to_owned()))
            })
    }

    pub fn find_package_dependencies(&self, key: &AbsolutePackageKey, package: &Package, target: InstallTarget) -> Result<Vec<PackageDependency>, PackageDependencyError> {
        let mut resolved = Vec::<String>::new();
        Ok(self.find_package_dependencies_impl(key, package, target, 0, &mut resolved)?)
    }

    fn find_package_dependencies_impl(
        &self,
        key: &AbsolutePackageKey,
        package: &Package,
        target: InstallTarget,
        level: u8,
        resolved: &mut Vec<String>
    ) -> Result<Vec<PackageDependency>, PackageDependencyError> {
        fn push_if_not_exists(dependency: PackageDependency, result: &mut Vec<PackageDependency>) {
            if result.iter().filter(|d| d.id == dependency.id).count() == 0 {
                result.push(dependency);
            }
        }

        let mut result = Vec::<PackageDependency>::new();

        for (package_id, version) in package.dependencies.iter() {
            // avoid circular references by keeping
            // track of package ids that have already been processed
            if resolved.contains(package_id) {
                continue;
            }
            resolved.push(package_id.clone());

            match self.find_package(package_id.as_str()) {
                Some((ref key, ref package)) => {
                    // add all the dependencies of the dependency
                    // to the list result first
                    for dependency in self.find_package_dependencies_impl(key, package, target, level + 1, resolved)? {
                        push_if_not_exists(dependency, &mut result);
                    }

                    // make sure the version requirement is correct
                    // TODO: equality isn't how version comparisons work.
                    // if package.version.as_str() != version {
                    //     return Err(PackageDependencyError::VersionNotFound);
                    // }

                    match self.status(key, target) {
                        Err(error) => return Err(PackageDependencyError::PackageStatusError(error)),
                        Ok(status) => {
                            match status {
                                PackageStatus::UpToDate => {},
                                _ => {
                                    let dependency = PackageDependency {
                                        id: key.clone(),
                                        version: version.clone(),
                                        level,
                                        status
                                    };
                                    push_if_not_exists(dependency, &mut result);
                                }
                            }
                            
                        }
                    }
                },
                _ => {
                    // the given package id does not exist
                    return Err(PackageDependencyError::PackageNotFound);
                }
            }
        }

        return Ok(result);
    }

    pub fn install(&self, key: &AbsolutePackageKey, target: InstallTarget) -> Result<PackageStatus, WindowsInstallError> {
        let package = match self.resolve_package(key) {
            Some(v) => v,
            None => {
                return Err(WindowsInstallError::NoPackage);
            }
        };

        let installer = match package.installer() {
            None => return Err(WindowsInstallError::NoInstaller),
            Some(v) => v
        };

        let installer = match *installer {
            Installer::Windows(ref v) => v,
            _ => return Err(WindowsInstallError::WrongInstallerType)
        };

        let url = url::Url::parse(&installer.url)
            .map_err(|_| WindowsInstallError::InvalidUrl(installer.url.to_owned()))?;
        let filename = url.path_segments().unwrap().last().unwrap();
        let pkg_path = self.download_path(&url.as_str()).join(filename);

        if !pkg_path.exists() {
            return Err(WindowsInstallError::PackageNotInCache)
        }

        let mut args: Vec<OsString> = match (&installer.installer_type, &installer.args) {
            (_, &Some(ref v)) => sys::args(&v).map(|x| x.clone()).collect(),
            (&Some(ref type_), &None) => {
                let mut arg_str = OsString::new();
                // TODO: generic parameter extensions for windows based on install target
                match type_.as_ref() {
                    "inno" => {
                        arg_str.push(&pkg_path);
                        arg_str.push(" /VERYSILENT /SP- /SUPPRESSMSGBOXES /NORESTART");
                        // TODO: add user-mode installation?
                    }
                    "msi" => {
                        arg_str.push("msiexec /i \"");
                        arg_str.push(&pkg_path);
                        arg_str.push("\" /qn /norestart");
                    }
                    "nsis" => {
                        arg_str.push(&pkg_path);
                        arg_str.push( "/SD");
                        if target == InstallTarget::User {
                            arg_str.push(" /CurrentUser")
                        }
                    }
                    _ => return Err(WindowsInstallError::InvalidType)
                };
                sys::args(&arg_str.as_os_str()).collect()
            }
            _ => return Err(WindowsInstallError::InvalidType)
        };
        let prog = args[0].clone();
        args.remove(0);

        let res = Command::new(&prog)
            .args(&args)
            .output();
        
        let output = match res {
            Ok(v) => v,
            Err(e) => {
                return Err(WindowsInstallError::Process(ProcessError::Io(e)));
            }
        };

        if !output.status.success() {
            return Err(WindowsInstallError::Process(ProcessError::Unknown(output)));
        }
        
        // match install_macos_package(&pkg_path, target) {
        //     Err(e) => return Err(WindowsInstallError::InstallerFailure(e)),
        //     _ => {}
        // };

        Ok(self.status_impl(&installer, key, &package, target).unwrap())
    }

    pub fn uninstall(&self, key: &AbsolutePackageKey, target: InstallTarget) -> Result<PackageStatus, WindowsUninstallError> {
        let package = match self.resolve_package(key) {
            Some(v) => v,
            None => {
                return Err(WindowsUninstallError::NoPackage);
            }
        };

        let installer = match package.installer() {
            None => return Err(WindowsUninstallError::NoInstaller),
            Some(v) => v
        };

        let installer = match installer {
            &Installer::Windows(ref v) => v,
            _ => return Err(WindowsUninstallError::WrongInstallerType)
        };
        
        let regkey = match uninstall_regkey(&installer) {
            Some(v) => v,
            None => return Err(WindowsUninstallError::NotInstalled)
        };

        let uninst_string: String = match regkey.get_value(Keys::QuietUninstallString)
                .or_else(|_| regkey.get_value(Keys::QuietUninstallString)) {
                    Ok(v) => v,
                    Err(_) => return Err(WindowsUninstallError::NoUninstString)
                };

        let mut raw_args: Vec<OsString> = sys::args(&uninst_string).map(|x| x.clone()).collect();
        let prog = raw_args[0].clone();
        raw_args.remove(0);

        let args: Vec<OsString> = match (&installer.installer_type, &installer.uninstall_args) {
            (_, &Some(ref v)) => sys::args(&v).map(|x| x.clone()).collect(),
            (&Some(ref type_), &None) => {
                let arg_str = match type_.as_ref() {
                    "inno" => "/VERYSILENT /SP- /SUPPRESSMSGBOXES /NORESTART".to_owned(),
                    "msi" => format!("/x \"{}\" /qn /norestart", &installer.product_code),
                    "nsis" => "".to_owned(),
                    _ => return Err(WindowsUninstallError::InvalidType)
                };
                sys::args(&arg_str).collect()
            },
            _ => return Err(WindowsUninstallError::InvalidType)
        };

        let res = Command::new(&prog)
            .args(&args)
            .output();
        
        let output = match res {
            Ok(v) => v,
            Err(e) => {
                return Err(WindowsUninstallError::Process(ProcessError::Io(e)));
            }
        };

        if !output.status.success() {
            return Err(WindowsUninstallError::Process(ProcessError::Unknown(output)));
        }

        Ok(self.status_impl(installer, key, &package, target).unwrap())
    }

    pub fn download<F>(&self, key: &AbsolutePackageKey, progress: F) -> Result<PathBuf, crate::download::DownloadError>
            where F: Fn(u64, u64) -> () + Send + 'static {
        let package = match self.resolve_package(key) {
            Some(v) => v,
            None => {
                return Err(crate::download::DownloadError::NoUrl);
            }
        };

        let installer = match package.installer() {
            None => return Err(crate::download::DownloadError::NoUrl),
            Some(v) => v
        };

        let mut disposable = package.download(&self.download_path(&installer.url()), Some(progress))?;
        disposable.wait()
    }

    pub fn status(&self, key: &AbsolutePackageKey, target: InstallTarget) -> Result<PackageStatus, PackageStatusError> {
         let package = match self.resolve_package(key) {
            Some(v) => v,
            None => {
                return Err(PackageStatusError::NoPackage);
            }
        };

        let installer = match package.installer() {
            None => return Err(PackageStatusError::NoInstaller),
            Some(v) => v
        };

        let installer = match installer {
            &Installer::Windows(ref v) => v,
            _ => return Err(PackageStatusError::WrongInstallerType)
        };

        self.status_impl(installer, key, &package, target)
    }

    fn status_impl(&self, installer: &WindowsInstaller, id: &AbsolutePackageKey, package: &Package, target: InstallTarget) -> Result<PackageStatus, PackageStatusError> {
        let inst_key = match uninstall_regkey(&installer) {
            Some(v) => v,
            None => return Ok(PackageStatus::NotInstalled)
        };

        let disp_version: String = match inst_key.get_value(Keys::DisplayVersion) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v
        };

        let installed_version = match semver::Version::parse(&disp_version) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v
        };

        let candidate_version = match semver::Version::parse(&package.version) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v
        };

        // TODO: handle skipped versions
        // TODO: assembly version lol

        if let Some(skipped_version) = self.config().skipped_package(id) {
            match semver::Version::parse(&skipped_version) {
                Err(_) => {}, // No point giving up now
                Ok(v) => {
                    if candidate_version <= v {
                        return Ok(PackageStatus::Skipped)
                    }
                }
            }
        }

        if candidate_version > installed_version {
            Ok(PackageStatus::RequiresUpdate)
        } else {
            Ok(PackageStatus::UpToDate)
        }
    }

    // pub fn install(&self, package: &'a Package) -> Result<PackageStatus, WindowsInstallError> {
    //     let installer = installer(&package).map_err(|_| WindowsInstallError::InvalidInstaller)?;
        
    //     let url = url::Url::parse(&installer.url).unwrap();
    //     let filename = url.path_segments().unwrap().last().unwrap();
    //     let pkg_path = self.download_path(&url.as_str()).join(filename);

    //     if !pkg_path.exists() {
    //         return Err(WindowsInstallError::PackageNotInCache)
    //     }

    //     let mut args: Vec<OsString> = match (&installer.installer_type, &installer.args) {
    //         (_, &Some(ref v)) => sys::args(&v).map(|x| x.clone()).collect(),
    //         (&Some(ref type_), &None) => {
    //             let mut arg_str = OsString::new();
    //             match type_.as_ref() {
    //                 "inno" => {
    //                     arg_str.push(&pkg_path);
    //                     arg_str.push(" /VERYSILENT /SP- /SUPPRESSMSGBOXES /NORESTART");
    //                 }
    //                 "msi" => {
    //                     arg_str.push("msiexec /i \"");
    //                     arg_str.push(&pkg_path);
    //                     arg_str.push("\" /qn /norestart");
    //                 }
    //                 "nsis" => {
    //                     arg_str.push(&pkg_path);
    //                     arg_str.push( "/SD");
    //                 }
    //                 _ => return Err(WindowsInstallError::InvalidType)
    //             };
    //             sys::args(&arg_str.as_os_str()).collect()
    //         }
    //         _ => return Err(WindowsInstallError::InvalidType)
    //     };
    //     let prog = args[0].clone();
    //     args.remove(0);

    //     let res = Command::new(&prog)
    //         .args(&args)
    //         .output();
        
    //     let output = match res {
    //         Ok(v) => v,
    //         Err(e) => {
    //             return Err(WindowsInstallError::Process(ProcessError::Io(e)));
    //         }
    //     };

    //     if !output.status.success() {
    //         return Err(WindowsInstallError::Process(ProcessError::Unknown(output)));
    //     }

    //     self.status(package).map_err(|e| panic!(e))
    // }

    // pub fn uninstall(&self, package: &'a Package) -> Result<PackageStatus, WindowsUninstallError> {
    //     let installer = installer(&package).map_err(|_| WindowsUninstallError::InvalidInstaller)?;
    //     let regkey = match uninstall_regkey(&installer) {
    //         Some(v) => v,
    //         None => return Err(WindowsUninstallError::NotInstalled)
    //     };

    //     let uninst_string: String = match regkey.get_value(Keys::QuietUninstallString)
    //             .or_else(|_| regkey.get_value(Keys::QuietUninstallString)) {
    //                 Ok(v) => v,
    //                 Err(_) => return Err(WindowsUninstallError::NoUninstString)
    //             };

    //     let mut raw_args: Vec<OsString> = sys::args(&uninst_string).map(|x| x.clone()).collect();
    //     let prog = raw_args[0].clone();
    //     raw_args.remove(0);

    //     let args: Vec<OsString> = match (&installer.installer_type, &installer.uninstall_args) {
    //         (_, &Some(ref v)) => sys::args(&v).map(|x| x.clone()).collect(),
    //         (&Some(ref type_), &None) => {
    //             let arg_str = match type_.as_ref() {
    //                 "inno" => "/VERYSILENT /SP- /SUPPRESSMSGBOXES /NORESTART".to_owned(),
    //                 "msi" => format!("/x \"{}\" /qn /norestart", &installer.product_code),
    //                 "nsis" => "".to_owned(),
    //                 _ => return Err(WindowsUninstallError::InvalidType)
    //             };
    //             sys::args(&arg_str).collect()
    //         },
    //         _ => return Err(WindowsUninstallError::InvalidType)
    //     };

    //     let res = Command::new(&prog)
    //         .args(&args)
    //         .output();
        
    //     let output = match res {
    //         Ok(v) => v,
    //         Err(e) => {
    //             return Err(WindowsUninstallError::Process(ProcessError::Io(e)));
    //         }
    //     };

    //     if !output.status.success() {
    //         return Err(WindowsUninstallError::Process(ProcessError::Unknown(output)));
    //     }

    //     // TODO: handle panic
    //     self.status(package).map_err(|e| panic!(e))
    // }
}
