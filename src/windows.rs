#![cfg(windows)]
use pahkat::types::{
    WindowsInstaller,
    Installer,
    Package,
    Downloadable
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

mod Keys {
    pub const UninstallPath: &'static str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
    pub const DisplayVersion: &'static str = "DisplayVersion";
    pub const SkipVersion: &'static str = "SkipVersion";
    pub const QuietUninstallString: &'static str = "QuietUninstallString";
    pub const UninstallString: &'static str = "UninstallString";
}

pub struct WindowsPackageStore<'a> {
    repo: &'a Repository,
    config: &'a StoreConfig
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
pub enum InstallError {
    InvalidInstaller,
    InvalidType,
    PackageNotInCache,
    Process(ProcessError),
}

#[derive(Debug)]
pub enum UninstallError {
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
        Err(e) => None,
        Ok(v) => Some(v)
    }
}

impl<'a> WindowsPackageStore<'a> {
    pub fn new(repo: &'a Repository, config: &'a StoreConfig) -> WindowsPackageStore<'a> {
        WindowsPackageStore { repo: repo, config: config }
    }
    
    // TODO: review if there is a better place to put this function...
    fn download_path(&self, url: &str) -> PathBuf {
        let mut sha = Sha256::new();
        sha.input_str(url);
        let hash_id = sha.result_str();
        
        self.config.package_cache_path().join(hash_id)
    }

    pub fn status(&self, package: &'a Package) -> Result<PackageStatus, PackageStatusError> {
        let installer = installer(&package)?;

        eprintln!("Checking status...");

        let inst_key = match uninstall_regkey(&installer) {
            Some(v) => v,
            None => return Ok(PackageStatus::NotInstalled)
        };

        eprintln!("WAT");

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

        if candidate_version > installed_version {
            Ok(PackageStatus::RequiresUpdate)
        } else {
            Ok(PackageStatus::UpToDate)
        }
    }

    pub fn install(&self, package: &'a Package) -> Result<PackageStatus, InstallError> {
        let installer = installer(&package).map_err(|_| InstallError::InvalidInstaller)?;
        
        let url = url::Url::parse(&installer.url).unwrap();
        let filename = url.path_segments().unwrap().last().unwrap();
        let pkg_path = self.download_path(&url.as_str()).join(filename);

        if !pkg_path.exists() {
            return Err(InstallError::PackageNotInCache)
        }

        let mut args: Vec<OsString> = match (&installer.installer_type, &installer.args) {
            (_, &Some(ref v)) => sys::args(&v).map(|x| x.clone()).collect(),
            (&Some(ref type_), &None) => {
                let mut arg_str = OsString::new();
                match type_.as_ref() {
                    "inno" => {
                        arg_str.push(&pkg_path);
                        arg_str.push(" /VERYSILENT /SP- /SUPPRESSMSGBOXES /NORESTART");
                    }
                    "msi" => {
                        arg_str.push("msiexec /i \"");
                        arg_str.push(&pkg_path);
                        arg_str.push("\" /qn /norestart");
                    }
                    "nsis" => {
                        arg_str.push(&pkg_path);
                        arg_str.push( "/SD");
                    }
                    _ => return Err(InstallError::InvalidType)
                };
                sys::args(&arg_str.as_os_str()).collect()
            }
            _ => return Err(InstallError::InvalidType)
        };
        let prog = args[0].clone();
        args.remove(0);

        let res = Command::new(&prog)
            .args(&args)
            .output();
        
        let output = match res {
            Ok(v) => v,
            Err(e) => {
                return Err(InstallError::Process(ProcessError::Io(e)));
            }
        };

        if !output.status.success() {
            return Err(InstallError::Process(ProcessError::Unknown(output)));
        }

        self.status(package).map_err(|e| panic!(e))
    }

    pub fn uninstall(&self, package: &'a Package) -> Result<PackageStatus, UninstallError> {
        let installer = installer(&package).map_err(|_| UninstallError::InvalidInstaller)?;
        let regkey = match uninstall_regkey(&installer) {
            Some(v) => v,
            None => return Err(UninstallError::NotInstalled)
        };

        let uninst_string: String = match regkey.get_value(Keys::QuietUninstallString)
                .or_else(|_| regkey.get_value(Keys::QuietUninstallString)) {
                    Ok(v) => v,
                    Err(_) => return Err(UninstallError::NoUninstString)
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
                    _ => return Err(UninstallError::InvalidType)
                };
                sys::args(&arg_str).collect()
            },
            _ => return Err(UninstallError::InvalidType)
        };

        let res = Command::new(&prog)
            .args(&args)
            .output();
        
        let output = match res {
            Ok(v) => v,
            Err(e) => {
                return Err(UninstallError::Process(ProcessError::Io(e)));
            }
        };

        if !output.status.success() {
            return Err(UninstallError::Process(ProcessError::Unknown(output)));
        }

        // TODO: handle panic
        self.status(package).map_err(|e| panic!(e))
    }
}
