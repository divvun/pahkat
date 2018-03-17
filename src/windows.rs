#![cfg(windows)]

use {Package, PackageStatus, PackageStatusError, Installer};
use std::path::Path;
use winreg::RegKey;
use winreg::enums::*;
use semver;
use std::io;

mod Keys {
    pub const UninstallPath: &'static str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
    pub const DisplayVersion: &'static str = "DisplayVersion";
    pub const SkipVersion: &'static str = "SkipVersion";
    pub const QuietUninstallString: &'static str = "QuietUninstallString";
    pub const UninstallString: &'static str = "UninstallString";
}

struct WindowsPackageStore {

}

/*
public PackageInstallStatus InstallStatus(Package package)
{
    if (package.Installer == null)
    {
        return PackageInstallStatus.ErrorNoInstaller;
    }

    var installer = package.Installer;
    var hklm = _registry.OpenBaseKey(RegistryHive.LocalMachine, RegistryView.Default);
    var path = $@"{Keys.UninstallPath}\{installer.ProductCode}";
    var instKey = hklm.OpenSubKey(path);

    if (instKey == null)
    {
        return PackageInstallStatus.NotInstalled;
    }
    
    var displayVersion = instKey.Get(Keys.DisplayVersion, "");
    if (displayVersion == "")
    {
        return PackageInstallStatus.ErrorParsingVersion;
    }

    var comp = CompareVersion(AssemblyVersion.Create, package.Version, displayVersion);
    if (comp != PackageInstallStatus.ErrorParsingVersion)
    {
        return comp;
    }

    if (SkippedVersion(package) == package.Version)
    {
        return PackageInstallStatus.VersionSkipped;
    }
        
    comp = CompareVersion(SemanticVersion.Create, package.Version, displayVersion);
    if (comp != PackageInstallStatus.ErrorParsingVersion)
    {
        return comp;
    }

    return PackageInstallStatus.ErrorParsingVersion;
}
*/

enum WindowsInstallError {
    Win32Error(io::Error)
}

impl WindowsPackageStore {
    fn status(&self, package: &Package) -> Result<PackageStatus, PackageStatusError> {
        let installer = match package.installer() {
            None => return Err(PackageStatusError::NoInstaller),
            Some(v) => match v {
                &Installer::Windows(ref v) => v,
                _ => return Err(PackageStatusError::WrongInstallerType),
            }
        };

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let path = Path::new(Keys::UninstallPath).join(&installer.product_code);
        let inst_key = match hklm.open_subkey(&path) {
            Err(_) => return Ok(PackageStatus::NotInstalled),
            Ok(v) => v
        };

        let disp_version: String = match inst_key.get_value(Keys::DisplayVersion) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v
        };

        let disp_semver = match semver::Version::parse(&disp_version) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v
        };

        let pkg_semver = match semver::Version::parse(&package.version) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v
        };

        unimplemented!()
    }

    fn install(&self, package: &Package, path: &Path) -> Result<(), ()> {
        unimplemented!()
    }

    fn uninstall(&self, package: &Package) -> Result<(), ()> {
        unimplemented!()
    }
}