use pahkat::types::*;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use xz2::read::XzDecoder;
use std::fs::{remove_file, read_dir, remove_dir, create_dir_all, File};
use std::cell::RefCell;
use rhai::RegisterFn;
use std::fmt::Display;
use std::str::FromStr;
use std::process::Command;
use std::collections::BTreeMap;

use serde::de::{self, Deserialize, Deserializer};
use plist::serde::{deserialize as deserialize_plist};

use ::*;

pub struct MacOSPackageStore<'a> {
    cache_path: &'a Path
}

fn from_str<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where T: FromStr,
          T::Err: Display,
          D: Deserializer<'de>
{
    let s = String::deserialize(deserializer)?;
    T::from_str(&s).map_err(de::Error::custom)
}

#[derive(Debug, Deserialize)]
struct BundlePlistInfo {
    #[serde(rename = "CFBundleIdentifier")]
    pub identifier: Option<String>,
    #[serde(rename = "CFBundleName")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "from_str", rename = "CFBundleVersion")]
    pub version: usize,
    #[serde(rename = "CFBundleShortVersionString")]
    pub short_version: Option<String>,
}

#[test]
fn test_bundle_plist() {
    let file = File::open("/Users/Brendan/Library/Keyboard Layouts/so.brendan.keyboards.keyboardlayout.brendan.bundle/Contents/Info.plist").unwrap();
    let plist: BundlePlistInfo = deserialize_plist(file).unwrap();
    println!("{:?}", plist);
}

#[derive(Debug, Clone, Copy)]
pub enum MacOSInstallError {
    NoInstaller,
    WrongInstallerType,
    InvalidFileType
}

#[derive(Debug, Clone, Copy)]
pub enum MacOSUninstallError {
    NoInstaller,
    WrongInstallerType,
}

impl<'a> MacOSPackageStore<'a> {
    fn new(cache_path: &'a Path) -> MacOSPackageStore<'a> {
        MacOSPackageStore { cache_path: cache_path }
    }

    // fn install_package(&self, package: &'a Package, installer: &'a MacOSPackageInstaller) -> Result<(), MacOSInstallError> {
    //     if !installer.url.ends_with(".txz") {
    //         return Err(MacOSInstallError::InvalidFileType);
    //     }

        
    //     unimplemented!()
    // }

    // fn uninstall_package(&self, package: &'a Package, installer: &'a MacOSPackageInstaller) -> Result<(), MacOSUninstallError> {
    //     unimplemented!()
    // }

    // fn status_package(&self, package: &'a Package, installer: &'a MacOSPackageInstaller) -> Result<PackageStatus, PackageStatusError> {
    //     unimplemented!()
    // }

    // fn install_bundle(&self, package: &'a Package, installer: &'a MacOSBundleInstaller) -> Result<(), MacOSInstallError> {
    //     if !installer.url.ends_with(".txz") {
    //         return Err(MacOSInstallError::InvalidFileType);
    //     }

    //     // Untar into a temporary directory

    //     // Find target.bundle

    //     // Install into expected directory

    //     unimplemented!()
    // }

    // fn uninstall_bundle(&self, package: &'a Package, installer: &'a MacOSBundleInstaller) -> Result<(), MacOSUninstallError> {
    //     // Find targetted bundle

    //     unimplemented!()
    // }

    // fn status_bundle(&self, package: &'a Package, installer: &'a MacOSBundleInstaller) -> Result<PackageStatus, PackageStatusError> {
    //     if !installer.install_path.starts_with("/") && !installer.install_path.starts_with("~") {
    //         return Err(PackageStatusError::InvalidInstallPath)
    //     }

    //     let path = Path::new(&installer.install_path);

    //     if path.exists() {
    //         let plist_path = path.join("Contents/Info.plist");
    //         let file = File::open(&plist_path)
    //             .map_err(|_| PackageStatusError::InvalidMetadata)?;
    //         let plist: BundlePlistInfo = deserialize_plist(file)
    //             .map_err(|_| PackageStatusError::InvalidMetadata)?;

    //         let short_version = match plist.short_version {
    //             Some(v) => v,
    //             None => return Err(PackageStatusError::ParsingVersion)
    //         };

    //         let installed_version = match semver::Version::parse(&short_version) {
    //             Err(_) => return Err(PackageStatusError::ParsingVersion),
    //             Ok(v) => v
    //         };

    //         let candidate_version = match semver::Version::parse(&package.version) {
    //             Err(_) => return Err(PackageStatusError::ParsingVersion),
    //             Ok(v) => v
    //         };

    //         if candidate_version > installed_version {
    //             Ok(PackageStatus::RequiresUpdate)
    //         } else {
    //             Ok(PackageStatus::UpToDate)
    //         }
    //     } else {
    //         return Ok(PackageStatus::NotInstalled)
    //     }
    // }

    pub fn install(&self, package: &'a Package, target: Option<MacOSInstallTarget>) -> Result<(), MacOSInstallError> {
        let installer = match package.installer() {
            None => return Err(MacOSInstallError::NoInstaller),
            Some(v) => v
        };

        match *installer {
            Installer::MacOSPackage(ref v) => v,
            _ => return Err(MacOSInstallError::WrongInstallerType)
        };

        unimplemented!()
    }

    pub fn uninstall(&self, package: &'a Package, target: Option<MacOSInstallTarget>) -> Result<(), MacOSUninstallError> {
        let installer = match package.installer() {
            None => return Err(MacOSUninstallError::NoInstaller),
            Some(v) => v
        };

        match *installer {
            Installer::MacOSPackage(ref v) => v,
            _ => return Err(MacOSUninstallError::WrongInstallerType)
        }
    }

    pub fn status(&self, package: &'a Package, target: Option<MacOSInstallTarget>) -> Result<PackageStatus, PackageStatusError> {
        let installer = match package.installer() {
            None => return Err(PackageStatusError::NoInstaller),
            Some(v) => v
        };

        match *installer {
            Installer::MacOSPackage(ref v) => v,
            _ => return Err(PackageStatusError::WrongInstallerType)
        }
    }
}

fn get_installed_packages(target: MacOSInstallTarget) -> Result<Vec<String>, io::Error> {
    use std::io::Cursor;
    use std::env;

    let home_dir = env::home_dir().expect("Always find home directory");
    
    let mut args = vec!["--pkgs-plist"];
    if let MacOSInstallTarget::User = target {
        args.push("--volume");
        args.push(&home_dir.to_str().unwrap());
    }

    let output = Command::new("pkgutil").args(&args).output()?;
    let plist_data = String::from_utf8(output.stdout).expect("plist should always be valid UTF-8");
    let cursor = Cursor::new(plist_data);
    let plist: Vec<String> = deserialize_plist(cursor).expect("plist should always be valid");
    return Ok(plist);
}

#[derive(Debug, Deserialize)]
struct MacOSPackageExportPath {
    pub gid: u64,
    #[serde(rename = "install-time")]
    pub install_time: u64,
    pub mode: u64,
    #[serde(rename = "pkg-version")]
    pub pkg_version: String,
    pub pkgid: String,
    pub uid: u64
}

#[derive(Debug, Deserialize)]
struct MacOSPackageExportPlist {
    #[serde(rename = "install-location")]
    pub install_location: String,
    #[serde(rename = "install-time")]
    pub install_time: u64,
    pub paths: BTreeMap<String, MacOSPackageExportPath>,
    #[serde(rename = "pkg-version")]
    pub pkg_version: String,
    pub pkgid: String,
    #[serde(rename = "receipt-plist-version")]
    pub receipt_plist_version: f64,
    pub volume: String
}

impl MacOSPackageExportPlist {
    fn path(&self) -> PathBuf {
        Path::new(&self.volume).join(&self.install_location)
    }

    fn paths(&self) -> Vec<PathBuf> {
        let base_path = self.path();
        self.paths.keys().map(|p| base_path.join(p)).collect()
    }
}

fn get_package_info(bundle_id: &str, target: MacOSInstallTarget) -> Option<MacOSPackageExportPlist> {
    use std::io::Cursor;
    use std::env;

    let home_dir = env::home_dir().expect("Always find home directory");
    let mut args = vec!["--export-plist", bundle_id];
    if let MacOSInstallTarget::User = target {
        args.push("--volume");
        args.push(&home_dir.to_str().unwrap());
    }

    let res = Command::new("pkgutil").args(&args).output();
    let output = match res {
        Ok(v) => v,
        Err(_) => return None
    };
    if !output.status.success() {
        return None;
    }
    let plist_data = String::from_utf8(output.stdout).expect("plist should always be valid UTF-8");
    let cursor = Cursor::new(plist_data);
    let plist: MacOSPackageExportPlist = deserialize_plist(cursor).expect("plist should always be valid");
    return Some(plist);
}

// #[test]
fn test_get_pkgs() {
    println!("{:?}", get_installed_packages(MacOSInstallTarget::User))
}

// #[test]
fn test_get_pkg_info() {
    let pkg_info = get_package_info("com.oracle.jre", MacOSInstallTarget::System).unwrap();
    // println!("{:?}", &pkg_info);
    println!("{:?}", &pkg_info.paths());
}

#[derive(Debug)]
enum ProcessError {
    Io(io::Error),
    Unknown(String)
}

fn install_macos_package(pkg_path: &Path, target: MacOSInstallTarget) -> Result<(), ProcessError> {
    let target_str = match target {
        MacOSInstallTarget::User => "CurrentUserHomeDirectory",
        MacOSInstallTarget::System => "LocalSystem"
    };

    let args = &[
        "-pkg",
        &pkg_path.to_str().unwrap(),
        "-target",
        target_str
    ];

    let res = Command::new("installer").args(args).output();
    let output = match res {
        Ok(v) => v,
        Err(e) => return Err(ProcessError::Io(e))
    };
    if !output.status.success() {
        return Err(ProcessError::Unknown(String::from_utf8_lossy(&output.stderr).to_string()));
    }
    Ok(())
}

fn uninstall_macos_package(bundle_id: &str, target: MacOSInstallTarget) -> Result<(), ()> {
    use std::fs;
    use std::env;
    
    let package_info = get_package_info(bundle_id, target).unwrap();

    let mut errors = vec![];
    let mut directories = vec![];

    for path in package_info.paths() {
        if path.is_dir() {
            directories.push(path);
            continue;
        }

        println!("Deleting: {:?}", &path);
        fs::remove_file(path).unwrap();
    }

    // Ensure children are deleted first
    directories.sort_unstable_by(|a, b| {
        let a_count = a.to_string_lossy().chars().filter(|x| *x == '/').count();
        let b_count = b.to_string_lossy().chars().filter(|x| *x == '/').count();
        b_count.cmp(&a_count)
    });

    for dir in directories {
        println!("Deleting: {:?}", &dir);
        match fs::remove_dir(dir) {
            Err(e) => errors.push(e),
            Ok(_) => {}
        }
    }

    println!("{:?}", errors);
    
    forget_pkg_id(bundle_id, target).unwrap();

    Ok(())
}

fn forget_pkg_id(bundle_id: &str, target: MacOSInstallTarget) -> Result<(), ()> {
    use std::env;

    let home_dir = env::home_dir().expect("Always find home directory");
    let mut args = vec!["--forget", bundle_id];
    if let MacOSInstallTarget::User = target {
        args.push("--volume");
        args.push(&home_dir.to_str().unwrap());
    }

    let res = Command::new("pkgutil").args(&args).output();
    let output = match res {
        Ok(v) => v,
        Err(e) => {
            println!("{:?}", e);
            return Err(())
        }
    };
    if !output.status.success() {
        println!("{:?}", output.status.code().unwrap());
        return Err(());
    }
    Ok(())
}

#[test]
fn end_to_end() {
    println!("{}", "A");
    install_macos_package(&Path::new("/Users/brendan/git/kbdgen/outputs/sme/North_Sami_Keyboard_1.0.1.unsigned.pkg"), MacOSInstallTarget::User).unwrap();
    println!("{}", "B");
    uninstall_macos_package("no.uit.giella.keyboardlayout.sme", MacOSInstallTarget::User).unwrap()
}
