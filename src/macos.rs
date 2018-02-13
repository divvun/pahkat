use pahkat::types::*;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use xz2::read::XzDecoder;
use std::fs::{remove_file, read_dir, remove_dir, create_dir_all, File};
use std::cell::RefCell;
use rhai::RegisterFn;

use ::*;

struct MacOSPackageStore {
    
}

impl MacOSPackageStore {
    fn install_package<'a>(&self, package: &'a Package, installer: &'a MacOSPackageInstaller, path: &Path) -> Result<PackageAction<'a>, ()> {
        unimplemented!()
    }

    fn install_bundle<'a>(&self, package: &'a Package, installer: &'a MacOSBundleInstaller, path: &Path) -> Result<PackageAction<'a>, ()> {
        unimplemented!()
    }

    fn uninstall_package<'a>(&self, package: &'a Package, installer: &'a MacOSPackageInstaller) -> Result<PackageAction<'a>, ()> {
        unimplemented!()
    }

    fn uninstall_bundle<'a>(&self, package: &'a Package, installer: &'a MacOSBundleInstaller) -> Result<PackageAction<'a>, ()> {
        unimplemented!()
    }

    fn status_package<'a>(&self, package: &'a Package, installer: &'a MacOSPackageInstaller) -> Result<PackageStatus, PackageStatusError> {
        unimplemented!()
    }

    fn status_bundle<'a>(&self, package: &'a Package, installer: &'a MacOSBundleInstaller) -> Result<PackageStatus, PackageStatusError> {
        unimplemented!()
    }
}

impl<'a> PackageStore<'a> for MacOSPackageStore {
    type StatusResult = Result<PackageStatus, PackageStatusError>;
    type InstallResult = Result<PackageAction<'a>, ()>;
    type UninstallResult = Result<PackageAction<'a>, ()>;

    fn install(&self, package: &'a Package, path: &Path) -> Self::InstallResult {
        let installer = match package.installer() {
            None => return Err(()),
            Some(v) => v
        };

        match *installer {
            Installer::MacOSPackage(ref v) => self.install_package(package, &v, path),
            Installer::MacOSBundle(ref v) => self.install_bundle(package, &v, path),
            _ => return Err(())
        };

        unimplemented!()
    }

    fn uninstall(&self, package: &'a Package) -> Self::UninstallResult {
        let installer = match package.installer() {
            None => return Err(()),
            Some(v) => v
        };

        match *installer {
            Installer::MacOSPackage(ref v) => self.uninstall_package(package, &v),
            Installer::MacOSBundle(ref v) => self.uninstall_bundle(package, &v),
            _ => return Err(())
        }
    }

    fn status(&self, package: &'a Package) -> Self::StatusResult {
        let installer = match package.installer() {
            None => return Err(PackageStatusError::NoInstaller),
            Some(v) => v
        };

        match *installer {
            Installer::MacOSPackage(ref v) => self.status_package(package, &v),
            Installer::MacOSBundle(ref v) => self.status_bundle(package, &v),
            _ => return Err(PackageStatusError::WrongInstallerType)
        }
    }
}