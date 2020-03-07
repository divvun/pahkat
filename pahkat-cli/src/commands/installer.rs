use std::fs::File;
use std::io::Write;
use std::path::Path;

use termcolor::Color;

use pahkat_common::{index_fn, ld_type, open_package};
use pahkat_types::{
    windows::Executable, InstallTarget, Installer, MacOSInstaller, TarballInstaller,
};

use crate::cli::{progress, prompt_question};

pub fn package_tarball_installer(
    file_path: &Path,
    channel: Option<&str>,
    force_yes: bool,
    tarball: &str,
    url: &str,
    size: usize,
) {
    let mut pkg = match open_package(&file_path, channel) {
        Ok(pkg) => pkg,
        Err(err) => {
            progress(Color::Red, "Error", &format!("{}", err)).unwrap();
            progress(
                Color::Red,
                "Error",
                "Package does not exist or is invalid; aborting",
            )
            .unwrap();
            return;
        }
    };

    let installer_file = File::open(tarball).expect("Installer could not be opened.");
    let meta = installer_file.metadata().unwrap();
    let installer_size = meta.len() as usize;

    let installer_index = TarballInstaller {
        _type: ld_type!("TarballInstaller"),
        url: url.to_owned(),
        size: installer_size,
        installed_size: size,
    };

    pkg.installer = Some(Installer::Tarball(installer_index));

    let json = serde_json::to_string_pretty(&pkg).unwrap();

    if !force_yes {
        println!("\n{}\n", json);

        if !prompt_question("Save index.json", true) {
            return;
        }
    }

    let mut file = File::create(file_path.join(index_fn(channel))).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write_all(&[b'\n']).unwrap();
}

pub fn package_macos_installer(
    file_path: &Path,
    channel: Option<&str>,
    version: &str,
    force_yes: bool,
    installer: &str,
    targets: Vec<&str>,
    pkg_id: &str,
    url: &str,
    size: usize,
    requires_reboot: bool,
    requires_uninst_reboot: bool,
) {
    let mut pkg = match open_package(&file_path, channel) {
        Ok(pkg) => pkg,
        Err(err) => {
            progress(Color::Red, "Error", &format!("{}", err)).unwrap();
            progress(
                Color::Red,
                "Error",
                "Package does not exist or is invalid; aborting",
            )
            .unwrap();
            return;
        }
    };

    let installer_file = File::open(installer).expect("Installer could not be opened.");
    let meta = installer_file.metadata().unwrap();
    let installer_size = meta.len() as usize;

    let target_results: Vec<Result<InstallTarget, &str>> = targets
        .iter()
        .map(|x| x.parse::<InstallTarget>().map_err(|_| *x))
        .collect();

    let target_errors: Vec<&str> = target_results
        .iter()
        .filter(|x| x.is_err())
        .map(|x| x.err().unwrap())
        .collect();

    if !target_errors.is_empty() {
        progress(
            Color::Red,
            "Error",
            &format!("Invalid targets supplied: {}", &target_errors.join(", ")),
        )
        .unwrap();
        return;
    }

    let targets: std::collections::BTreeSet<InstallTarget> = target_results
        .iter()
        .filter(|x| x.is_ok())
        .map(|x| x.unwrap())
        .collect();

    let installer_index = MacOSInstaller {
        _type: ld_type!("MacOSInstaller"),
        url: url.to_owned(),
        pkg_id: pkg_id.to_owned(),
        targets,
        requires_reboot,
        requires_uninstall_reboot: requires_uninst_reboot,
        size: installer_size,
        installed_size: size,
        signature: None,
    };

    pkg.version = version.to_owned();
    pkg.installer = Some(Installer::MacOS(installer_index));

    let json = serde_json::to_string_pretty(&pkg).unwrap();

    if !force_yes {
        println!("\n{}\n", json);

        if !prompt_question("Save index.json", true) {
            return;
        }
    }

    // TODO Check dir exists
    let mut file = File::create(file_path.join(index_fn(channel))).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write_all(&[b'\n']).unwrap();
}

pub fn package_windows_installer(
    file_path: &Path,
    channel: Option<&str>,
    force_yes: bool,
    product_code: &str,
    installer: &str,
    type_: Option<&str>,
    args: Option<&str>,
    uninst_args: Option<&str>,
    url: &str,
    size: usize,
    requires_reboot: bool,
    requires_uninst_reboot: bool,
) {
    let mut pkg = match open_package(&file_path, channel) {
        Ok(pkg) => pkg,
        Err(err) => {
            progress(Color::Red, "Error", &format!("{}", err)).unwrap();
            progress(
                Color::Red,
                "Error",
                "Package does not exist or is invalid; aborting",
            )
            .unwrap();
            return;
        }
    };

    let installer_file = File::open(installer).expect("Installer could not be opened.");
    let meta = installer_file.metadata().unwrap();
    let installer_size = meta.len() as usize;

    let installer_index = windows::Executable {
        _type: ld_type!("windows::Executable"),
        url: url.to_owned(),
        installer_type: type_.map(|x| x.to_owned()),
        args: args.map(|x| x.to_owned()),
        uninstall_args: uninst_args.map(|x| x.to_owned()),
        product_code: product_code.to_owned(),
        requires_reboot,
        requires_uninstall_reboot: requires_uninst_reboot,
        size: installer_size,
        installed_size: size,
        signature: None,
    };

    pkg.installer = Some(Installer::Windows(installer_index));

    let json = serde_json::to_string_pretty(&pkg).unwrap();

    if !force_yes {
        println!("\n{}\n", json);

        if !prompt_question("Save index.json", true) {
            return;
        }
    }

    let mut file = File::create(file_path.join(index_fn(channel))).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write_all(&[b'\n']).unwrap();
}
