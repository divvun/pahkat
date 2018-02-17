#[macro_use]
extern crate clap;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate termcolor;
extern crate pathdiff;

use termcolor::Color;

use clap::{Arg, App, AppSettings, SubCommand};
use std::env;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::fmt;

mod cli;
mod types;

use cli::*;
use types::*;

macro_rules! ld_type {
    ($e:expr) => {
        Some(format!("{}", $e).to_owned())
    };
}

const LD_CONTEXT: &'static str = "https://pahkat.org/";

fn default_pkg_id(path: &Path) -> String {
    let c = path.components().last().unwrap();
    c.as_os_str()
        .to_string_lossy()
        .to_string()
        .to_lowercase()
}

fn request_package_data(cur_dir: &Path) -> Package {
    let package_id = prompt_line("Package identifier", &default_pkg_id(cur_dir)).unwrap();
    
    let en_name = prompt_line("Name", "").unwrap();
    let mut name = HashMap::new();
    name.insert("en".to_owned(), en_name);

    let en_description = prompt_line("Description", "").unwrap();
    let mut description = HashMap::new();
    description.insert("en".to_owned(), en_description);

    let version = prompt_line("Version", "0.1.0").unwrap();
    let category = prompt_line("Category", "").unwrap();

    println!("Package languages are languages the installed package supports.");
    let languages: Vec<String> = prompt_line("Package languages (comma-separated)", "en").unwrap()
        .split(",")
        .map(|x| x.trim().to_owned())
        .collect();

    println!("Supported platforms: android, ios, linux, macos, windows");
    println!("Specify platform support like \"windows\" or with version guards \"windows >= 8.1\".");
    let platform_vec: Vec<String> = prompt_line("Platforms (comma-separated)", OS).unwrap()
        .split(",")
        .map(|x| x.trim().to_owned())
        .collect();
    let platform = parse_platform_list(&platform_vec);

    Package {
        _context: Some(LD_CONTEXT.to_owned()),
        _type: ld_type!("Package"),
        id: package_id,
        name: name,
        description: description,
        version: version,
        category: category,
        languages: languages,
        platform: platform,
        dependencies: Default::default(),
        virtual_dependencies: Default::default(),
        installer: None
    }
}

fn request_repo_data() -> Repository {
    let base = {
        let base = prompt_line("Base URL", "").unwrap();
        if !base.ends_with("/") {
            format!("{}/", base)
        } else {
            base
        }
    };
    
    let en_name = prompt_line("Name", "Repository").unwrap();
    let mut name = HashMap::new();
    name.insert("en".to_owned(), en_name);

    let en_description = prompt_line("Description", "").unwrap();
    let mut description = HashMap::new();
    description.insert("en".to_owned(), en_description);

    println!("Supported filters: category, language");
    let primary_filter = prompt_line("Primary Filter", "category").unwrap();

    println!("Supported channels: stable, beta, alpha, nightly");
    let channels: Vec<String> = prompt_line("Channels (comma-separated)", "stable").unwrap()
        .split(",")
        .map(|x| x.trim().to_owned())
        .collect();

    Repository {
        _context: Some(LD_CONTEXT.to_owned()),
        _type: ld_type!("Repository"),
        agent: Some(RepositoryAgent::default()),
        base: base,
        name: name,
        description: description,
        primary_filter: primary_filter,
        channels: channels
    }
}

fn package_init(output_dir: &Path) {
    if open_repo(&output_dir).is_ok() {
        progress(Color::Red, "Error", "Cannot generate package within repository; aborting.").unwrap();
        return;
    }

    if output_dir.join("index.json").exists() {
        progress(Color::Red, "Error", "A file or directory named 'index.json' already exists; aborting.").unwrap();
        return;
    }

    let pkg_data = request_package_data(output_dir);
    let json = serde_json::to_string_pretty(&pkg_data).unwrap();
    
    println!("\n{}\n", json);

    if prompt_question("Save index.json", true) {
        let mut file = File::create(output_dir.join("index.json")).unwrap();
        file.write_all(json.as_bytes()).unwrap();
        file.write(&[b'\n']).unwrap();
    }
}

fn write_repo_index_virtuals(cur_dir: &Path, index: &Virtuals) {
    let json = serde_json::to_string_pretty(&index).unwrap();
    let pkg_path = cur_dir.join("virtuals");

    progress(Color::Green, "Writing", "virtuals index").unwrap();
    let mut file = File::create(&pkg_path.join("index.json")).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write(&[b'\n']).unwrap();
}

fn generate_repo_index_virtuals(cur_dir: &Path, repo: &Repository) -> Virtuals {
    progress(Color::Green, "Generating", "virtuals index").unwrap();

    let pkg_path = cur_dir.join("virtuals");
    let mut map = HashMap::new();

    for x in fs::read_dir(&pkg_path).unwrap() {
        let path = x.unwrap().path();
        
        if !path.is_dir() {
            continue;
        }

        let indexes: Vec<Virtual> = fs::read_dir(&path).unwrap()
            .map(|x| x.unwrap().path())
            .filter(|path| path.is_dir() && path.join("index.json").exists())
            .map(|path| {
                let file = File::open(path.join("index.json")).unwrap();
                let pkg_index: Virtual = serde_json::from_reader(file)
                    .expect(path.join("index.json").to_str().unwrap());
                let msg = format!("{} {}", &pkg_index.id, &pkg_index.version);
                progress(Color::Yellow, "Inserting", &msg).unwrap();
                pkg_index
            })
            .collect();
        
        for pkg in indexes.into_iter() {
            let entry = map.entry(pkg.id.to_owned()).or_insert(vec![]);
            entry.push(pkg.version);
        }
    }

    Virtuals {
        _context: Some(LD_CONTEXT.to_owned()),
        _type: ld_type!("Virtuals"),
        _id: Some("".to_owned()),
        base: format!("{}virtuals/", &repo.base),
        virtuals: map
    }
}

fn generate_repo_index_packages(cur_dir: &Path, repo: &Repository) -> Packages {
    progress(Color::Green, "Generating", "packages index").unwrap();

    let pkg_path = cur_dir.join("packages");
    let pkgs: Vec<Package> = fs::read_dir(&pkg_path)
        .unwrap()
        .map(|x| {
            x.unwrap().path()
        })
        .filter_map(|path| {
            if !path.is_dir() || !path.join("index.json").exists() {
                if path.ends_with("index.json") {
                    return None;
                }

                let relpath = pathdiff::diff_paths(&*path, cur_dir).unwrap();
                progress(Color::Magenta, "Warning", &format!("{:?} is not a directory; skipping", &relpath)).unwrap();
                return None;
            }

            let index_path = path.join("index.json");
            let file = File::open(&index_path).unwrap();
            let pkg_index: Package = match serde_json::from_reader(file) {
                Ok(x) => x,
                Err(err) => {
                    let relpath = pathdiff::diff_paths(&*index_path, cur_dir).unwrap();
                    progress(Color::Red, "Error", &format!("Error parsing path {:?}:", &relpath)).unwrap();
                    progress(Color::Red, "Error", &format!("{}", err)).unwrap();
                    return None;
                }
            };

            if pkg_index.installer.is_none() {
                progress(Color::Magenta, "Warning", &format!("{} {} has no installer; skipping", &pkg_index.id, &pkg_index.version)).unwrap();
                return None;
            }   
            
            let msg = format!("{} {}", &pkg_index.id, &pkg_index.version);
            progress(Color::Yellow, "Inserting", &msg).unwrap();
            Some(pkg_index)
        })
        .collect();
    
    let mut map = HashMap::new();
    for pkg in pkgs.into_iter() {
        map.insert(pkg.id.to_owned(), pkg);
    }

    Packages {
        _context: Some(LD_CONTEXT.to_owned()),
        _type: ld_type!("Packages"),
        _id: Some("".to_owned()),
        base: format!("{}packages/", &repo.base),
        packages: map
    }
}

fn write_repo_index_packages(cur_dir: &Path, index: &Packages) {
    let pkg_path = cur_dir.join("packages");
    let json = serde_json::to_string_pretty(&index).unwrap();

    progress(Color::Green, "Writing", "packages index").unwrap();
    let mut file = File::create(&pkg_path.join("index.json")).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write(&[b'\n']).unwrap();
}

enum OpenIndexError {
    FileError(std::io::Error),
    JsonError(serde_json::Error)
}

impl fmt::Display for OpenIndexError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            OpenIndexError::FileError(ref x) => write!(f, "{}", x),
            OpenIndexError::JsonError(ref x) => write!(f, "{}", x)
        }
    }
}

fn open_repo(path: &Path) -> Result<Repository, OpenIndexError> {
    let file = File::open(path.join("index.json"))
        .map_err(|e| OpenIndexError::FileError(e))?;
    let index = serde_json::from_reader(file)
        .map_err(|e| OpenIndexError::JsonError(e))?;
    Ok(index)
}

fn open_package(path: &Path) -> Result<Package, OpenIndexError> {
    let file = File::open(path.join("index.json"))
        .map_err(|e| OpenIndexError::FileError(e))?;
    let index = serde_json::from_reader(file)
        .map_err(|e| OpenIndexError::JsonError(e))?;
    Ok(index)
}

fn repo_init(cur_dir: &Path) {
    if open_repo(&cur_dir).is_ok() {
        progress(Color::Red, "Error", "Repo already exists; aborting.").unwrap();
        return;
    }

    if cur_dir.join("packages").exists() {
        progress(Color::Red, "Error", "A file or directory named 'packages' already exists; aborting.").unwrap();
        return;
    }

    if cur_dir.join("virtuals").exists() {
        progress(Color::Red, "Error", "A file or directory named 'virtuals' already exists; aborting.").unwrap();
        return;
    }

    let repo_data = request_repo_data();
    let json = serde_json::to_string_pretty(&repo_data).unwrap();
    
    println!("\n{}\n", json);

    if !prompt_question("Save index.json and generate repo directories", true) {
        return;
    }

    if !cur_dir.exists() {
        fs::create_dir(&cur_dir).unwrap();
    }

    let mut file = File::create(cur_dir.join("index.json")).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write(&[b'\n']).unwrap();

    fs::create_dir(cur_dir.join("packages")).unwrap();
    fs::create_dir(cur_dir.join("virtuals")).unwrap();

    repo_index(&cur_dir);
}

fn generate_repo_index_meta(repo_path: &Path) -> Repository {
    progress(Color::Green, "Generating", "repository index").unwrap();

    let file = File::open(repo_path.join("index.json")).unwrap();
    let mut repo_index: Repository = serde_json::from_reader(file)
        .expect(repo_path.join("index.json").to_str().unwrap());

    repo_index._type = ld_type!("Repository");
    repo_index.agent = Some(RepositoryAgent::default());

    repo_index
}

fn write_repo_index_meta(cur_dir: &Path, repo_index: &Repository) {
    let repo_path = cur_dir;
    let json = serde_json::to_string_pretty(&repo_index).unwrap();

    progress(Color::Green, "Writing", "repository index").unwrap();
    let mut file = File::create(&repo_path.join("index.json")).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write(&[b'\n']).unwrap();
}

fn repo_index(cur_dir: &Path) {
    if let Err(err) = open_repo(&cur_dir) {
        progress(Color::Red, "Error", &format!("{}", err)).unwrap();
        progress(Color::Red, "Error", "Repo does not exist or is invalid; aborting.").unwrap();
        return;
    }
    
    if !cur_dir.join("packages").exists() {
        fs::create_dir(cur_dir.join("packages")).unwrap();
    }

    if !cur_dir.join("virtuals").exists() {
        fs::create_dir(cur_dir.join("virtuals")).unwrap();
    }
    
    let repo_index = generate_repo_index_meta(&cur_dir);
    let package_index = generate_repo_index_packages(&cur_dir, &repo_index);
    let virtuals_index = generate_repo_index_virtuals(&cur_dir, &repo_index);

    write_repo_index_meta(&cur_dir, &repo_index);
    write_repo_index_packages(&cur_dir, &package_index);
    write_repo_index_virtuals(&cur_dir, &virtuals_index);
}

fn package_tarball_installer(file_path: &Path, force_yes: bool, tarball: &str, url: &str, size: usize) {
    let mut pkg = match open_package(&file_path) {
        Ok(pkg) => pkg,
        Err(err) => {
            progress(Color::Red, "Error", &format!("{}", err)).unwrap();
            progress(Color::Red, "Error", "Package does not exist or is invalid; aborting").unwrap();
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
        installed_size: size
    };

    pkg.installer = Some(Installer::Tarball(installer_index));

    let json = serde_json::to_string_pretty(&pkg).unwrap();
    
    if !force_yes {
        println!("\n{}\n", json);

        if !prompt_question("Save index.json", true) {
            return;
        }
    }

    let mut file = File::create(file_path.join("index.json")).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write(&[b'\n']).unwrap();
}

fn package_macos_installer(file_path: &Path, force_yes: bool, installer: &str, targets: Vec<&str>, pkg_id: &str,
        url: &str, size: usize, requires_reboot: bool, requires_uninst_reboot: bool) {
    let mut pkg = match open_package(&file_path) {
        Ok(pkg) => pkg,
        Err(err) => {
            progress(Color::Red, "Error", &format!("{}", err)).unwrap();
            progress(Color::Red, "Error", "Package does not exist or is invalid; aborting").unwrap();
            return;
        }
    };

    let installer_file = File::open(installer).expect("Installer could not be opened.");
    let meta = installer_file.metadata().unwrap();
    let installer_size = meta.len() as usize;

    let target_results: Vec<Result<MacOSInstallTarget, &str>> = targets.iter()
        .map(|x| x.parse::<MacOSInstallTarget>().map_err(|_| *x))
        .collect();

    let target_errors: Vec<&str> = target_results.iter().filter(|x| x.is_err()).map(|x| x.err().unwrap()).collect();

    if target_errors.len() > 0 {
        progress(Color::Red, "Error", &format!("Invalid targets supplied: {}", &target_errors.join(", "))).unwrap();
        return;
    }

    let targets: std::collections::BTreeSet<MacOSInstallTarget> = target_results.iter()
        .filter(|x| x.is_ok())
        .map(|x| x.unwrap())
        .collect();

    let installer_index = MacOSInstaller {
        _type: ld_type!("MacOSInstaller"),
        url: url.to_owned(),
        pkg_id: pkg_id.to_owned(),
        targets: targets,
        requires_reboot: requires_reboot,
        requires_uninstall_reboot: requires_uninst_reboot,
        size: installer_size,
        installed_size: size,
        signature: None
    };

    pkg.installer = Some(Installer::MacOSPackage(installer_index));

    let json = serde_json::to_string_pretty(&pkg).unwrap();
    
    if !force_yes {
        println!("\n{}\n", json);

        if !prompt_question("Save index.json", true) {
            return;
        }
    }

    // TODO Check dir exists
    let mut file = File::create(file_path.join("index.json")).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write(&[b'\n']).unwrap();
}

fn package_windows_installer(file_path: &Path, force_yes: bool, product_code: &str, installer: &str, type_: Option<&str>,
        args: Option<&str>, uninst_args: Option<&str>, url: &str, size: usize, 
        requires_reboot: bool, requires_uninst_reboot: bool) {
    let mut pkg = match open_package(&file_path) {
        Ok(pkg) => pkg,
        Err(err) => {
            progress(Color::Red, "Error", &format!("{}", err)).unwrap();
            progress(Color::Red, "Error", "Package does not exist or is invalid; aborting").unwrap();
            return;
        }
    };

    let installer_file = File::open(installer).expect("Installer could not be opened.");
    let meta = installer_file.metadata().unwrap();
    let installer_size = meta.len() as usize;

    let installer_index = WindowsInstaller {
        _type: ld_type!("WindowsInstaller"),
        url: url.to_owned(),
        installer_type: type_.map(|x| x.to_owned()),
        args: args.map(|x| x.to_owned()),
        uninstall_args: uninst_args.map(|x| x.to_owned()),
        product_code: product_code.to_owned(),
        requires_reboot: requires_reboot,
        requires_uninstall_reboot: requires_uninst_reboot,
        size: installer_size,
        installed_size: size,
        signature: None
    };

    pkg.installer = Some(Installer::Windows(installer_index));

    let json = serde_json::to_string_pretty(&pkg).unwrap();
    
    if !force_yes {
        println!("\n{}\n", json);

        if !prompt_question("Save index.json", true) {
            return;
        }
    }

    let mut file = File::create(file_path.join("index.json")).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write(&[b'\n']).unwrap();
}

fn main() {
    let matches = App::new("Páhkat")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .version(crate_version!())
        .author("Brendan Molloy <brendan@bbqsrc.net>")
        .about("The last package manager. \"Pákhat\" is the nominative plural form for \"packages\" in Northern Sámi.")
        .subcommand(
            SubCommand::with_name("repo")
            .about("Repository-related subcommands")
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .arg(Arg::with_name("path")
                .value_name("PATH")
                .help("The repository root directory (default: current working directory)")
                .short("p")
                .long("path")
                .takes_value(true)
            )
            .subcommand(
                SubCommand::with_name("init")
                    .about("Initialise a Páhkat repository in the current working directory")
            )
            .subcommand(
                SubCommand::with_name("index")
                    .about("Regenerate packages and virtuals indexes from source indexes")
            )
        )
        .subcommand(
            SubCommand::with_name("init")
            .about("Initialise a package to the specified directory")
            .arg(Arg::with_name("output")
                .value_name("OUTPUT")
                .help("The installer index output directory (default: current working directory)")
                .short("o")
                .long("output")
                .takes_value(true)
            )
        )
        .subcommand(
            SubCommand::with_name("installer")
                .about("Inject installer data into package index")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .arg(Arg::with_name("path")
                    .value_name("PATH")
                    .help("The package index root directory (default: current working directory)")
                    .short("p")
                    .long("path")
                    .takes_value(true)
                )
                .subcommand(SubCommand::with_name("macos")
                    .about("Inject macOS .pkg installer data into package index")
                    .arg(Arg::with_name("package")
                        .value_name("PKG")
                        .help("The package file (.pkg)")
                        .short("i")
                        .long("package")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("pkg-id")
                        .value_name("PKGID")
                        .help("The bundle identifier for the installed package (eg, com.example.package)")
                        .short("c")
                        .long("pkgid")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("targets")
                        .value_name("TARGETS")
                        .help("The supported targets for installation, comma-delimited (options: system, user)")
                        .short("o")
                        .long("targets")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("requires-reboot")
                        .help("Installer requires reboot after installation")
                        .short("r")
                        .long("reboot")
                    )
                    .arg(Arg::with_name("requires-uninst-reboot")
                        .help("Uninstaller requires reboot after installation")
                        .short("R")
                        .long("uninst-reboot")
                    )
                    .arg(Arg::with_name("url")
                        .value_name("URL")
                        .help("The URL where the installer will be downloaded from")
                        .short("u")
                        .long("url")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("installed-size")
                        .value_name("SIZE")
                        .help("The size on disk when the package is installed")
                        .short("s")
                        .long("size")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("skip-confirmation")
                        .help("Skip confirmation step")
                        .short("y")
                        .long("yes")
                    ))
                .subcommand(SubCommand::with_name("windows")
                    .about("Inject Windows installer data into package index")
                    .arg(Arg::with_name("product-code")
                        .value_name("PRODUCT_CODE")
                        .help("The product code that identifies the installer in the registry")
                        .short("c")
                        .long("code")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("installer")
                        .value_name("INSTALLER")
                        .help("The installer .msi/.exe")
                        .short("i")
                        .long("installer")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("type")
                        .value_name("TYPE")
                        .help("Type of installer to autoconfigure silent install and uninstall (supported: msi, inno)")
                        .short("t")
                        .long("type")
                        .takes_value(true)
                    )
                    .arg(Arg::with_name("args")
                        .value_name("ARGS")
                        .help("Arguments to installer for it to run silently")
                        .short("a")
                        .long("args")
                        .takes_value(true)
                    )
                    .arg(Arg::with_name("uninst-args")
                        .value_name("ARGS")
                        .help("Arguments to uninstaller for it to run silently")
                        .short("A")
                        .long("uninst-args")
                        .takes_value(true)
                    )
                    .arg(Arg::with_name("requires-reboot")
                        .help("Installer requires reboot after installation")
                        .short("r")
                        .long("reboot")
                    )
                    .arg(Arg::with_name("requires-uninst-reboot")
                        .help("Uninstaller requires reboot after installation")
                        .short("R")
                        .long("uninst-reboot")
                    )
                    .arg(Arg::with_name("url")
                        .value_name("URL")
                        .help("The URL where the installer will be downloaded from")
                        .short("u")
                        .long("url")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("installed-size")
                        .value_name("SIZE")
                        .help("The size on disk when the package is installed")
                        .short("s")
                        .long("size")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("skip-confirmation")
                        .help("Skip confirmation step")
                        .short("y")
                        .long("yes")
                    ))
                .subcommand(SubCommand::with_name("tarball")
                    .about("Inject tarball install data into package index")
                    .arg(Arg::with_name("tarball")
                        .value_name("TARBALL")
                        .help("The 'installer' tarball (.txz)")
                        .short("i")
                        .long("tarball")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("url")
                        .value_name("URL")
                        .help("The URL where the installer will be downloaded from")
                        .short("u")
                        .long("url")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("installed-size")
                        .value_name("SIZE")
                        .help("The size on disk when the package is installed")
                        .short("s")
                        .long("size")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::with_name("skip-confirmation")
                        .help("Skip confirmation step")
                        .short("y")
                        .long("yes")
                    ))
        )
        .get_matches();
    
    match matches.subcommand() {
        ("init", Some(matches)) => {
            let current_dir = &env::current_dir().unwrap();
            let output: &Path = matches.value_of("output")
                .map_or(&current_dir, |v| Path::new(v));
            package_init(&output)
        },
        ("installer", Some(matches)) => {
            let current_dir = &env::current_dir().unwrap();
            let path: &Path = matches.value_of("path")
                .map_or(&current_dir, |v| Path::new(v));

            match matches.subcommand() {
                ("macos", Some(matches)) => {
                    let installer = matches.value_of("package").unwrap();
                    let targets: Vec<&str> = matches.value_of("targets").unwrap().split(",").collect();
                    let pkg_id = matches.value_of("pkg-id").unwrap();
                    let url = matches.value_of("url").unwrap();
                    let size = matches.value_of("installed-size").unwrap()
                        .parse::<usize>().unwrap();
                    let requires_reboot = matches.is_present("requires-reboot");
                    let requires_uninst_reboot = matches.is_present("requires-uninst-reboot");
                    let skip_confirm = matches.is_present("skip-confirmation");

                    package_macos_installer(path, skip_confirm, installer, targets, pkg_id, url, size, requires_reboot, 
                        requires_uninst_reboot);
                },
                ("windows", Some(matches)) => {
                    let product_code = matches.value_of("product-code").unwrap();
                    let type_ = matches.value_of("type");
                    let installer = matches.value_of("installer").unwrap();
                    let args = matches.value_of("args");
                    let uninstall_args = matches.value_of("uninst-args");
                    let url = matches.value_of("url").unwrap();
                    let size = matches.value_of("installed-size").unwrap()
                        .parse::<usize>().unwrap();
                    let requires_reboot = matches.is_present("requires-reboot");
                    let requires_uninst_reboot = matches.is_present("requires-uninst-reboot");
                    let skip_confirm = matches.is_present("skip-confirmation");

                    package_windows_installer(path, skip_confirm, product_code, installer, type_, args, uninstall_args, url, 
                        size, requires_reboot, requires_uninst_reboot);
                },
                ("tarball", Some(matches)) => {
                    let tarball = matches.value_of("tarball").unwrap();
                    let url = matches.value_of("url").unwrap();
                    let size = matches.value_of("installed-size").unwrap()
                        .parse::<usize>().unwrap();
                    let skip_confirm = matches.is_present("skip-confirmation");

                    package_tarball_installer(path, skip_confirm, tarball, url, size);
                },
                _ => {}
            }
        }
        ("repo", Some(matches)) => {
            let current_dir = &env::current_dir().unwrap();
            let path: &Path = matches.value_of("path")
                .map_or(&current_dir, |v| Path::new(v));

            match matches.subcommand() {
                ("init", _) => repo_init(&path),
                ("index", _) => repo_index(&path),
                _ => {}
            }
        }
        _ => {}
    }
}
