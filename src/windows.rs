#![cfg(windows)]

use {Package, PackageAction, PackageStore, PackageStatus, PackageStatusError, Installer};
use std::path::Path;
use winreg::RegKey;
use winreg::enums::*;
use semver;

mod Keys {
    const UninstallPath: &'static str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
    const DisplayVersion: &'static str = "DisplayVersion";
    const SkipVersion: &'static str = "SkipVersion";
    const QuietUninstallString: &'static str = "QuietUninstallString";
    const UninstallString: &'static str = "UninstallString";
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
    Win32Error(std::io::Error)
}

impl<'a> PackageStore<'a> for WindowsPackageStore {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    type StatusResult = Result<PackageStatus, PackageStatusError>;
    type InstallResult = Result<PackageAction<'a>, ()>;
    type UninstallResult = Result<PackageAction<'a>, ()>;

    fn status(&self, package: &'a Package) -> Self::StatusResult {
        let installer = match package.installer() {
            None => return Err(PackageStatusError::NoInstaller),
            Some(v) => match v {
                Installer::Tarball(_) => return Err(PackageStatusError::WrongInstallerType),
                Installer::Windows(v) => v
            }
        };

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let path = Path::new(Keys::UninstallPath).join(&installer.product_code);
        let inst_key = match hklm.open_subkey(&path) {
            Err(_) => return Ok(PackageStatus::NotInstalled),
            Ok(v) => v
        };

        let disp_version: String = match key.get_value(Keys::DisplayVersion) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v
        }

        let disp_semver = match semver::Version::parse(&display_version) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v
        }

        let pkg_semver = match semver::Version::parse(&package.version) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v
        }

        unimplemented!()
    }

    fn install(&self, package: &'a Package, path: &'a Path) -> Self::InstallResult {
        

        unimplemented!()
    }
    fn uninstall(&self, package: &'a Package) -> Self::UninstallResult {
        unimplemented!()
    }
}