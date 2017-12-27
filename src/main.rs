#[macro_use]
extern crate clap;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate termcolor;

use termcolor::Color;

use clap::{Arg, App, AppSettings, SubCommand};
use std::env;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

mod cli;
mod types;

use cli::*;
use types::*;

fn cur_dir() -> String {
    env::current_dir().unwrap()
        .components().last().unwrap()
        .as_os_str()
        .to_string_lossy()
        .to_string()
}

fn request_package_data() -> PackageIndex {
    let package_id = prompt_line("Package identifier", &cur_dir().to_lowercase()).unwrap();
    
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

    println!("Supported OSes: windows, macos, linux, ios, android");
    println!("Specify OS support like \"windows\" or with version guards \"windows >= 8.1\".");
    let os_vec: Vec<String> = prompt_line("Operating systems (comma-separated)", OS).unwrap()
        .split(",")
        .map(|x| x.trim().to_owned())
        .collect();
    let os = parse_os_list(&os_vec);

    PackageIndex {
        id: package_id,
        name: name,
        description: description,
        version: version,
        category: category,
        languages: languages,
        os: os,
        dependencies: Default::default(),
        virtual_dependencies: Default::default(),
        installer: None
    }
}

fn request_repo_data() -> RepoIndex {
    let base = prompt_line("Base URL", "").unwrap();
    
    let en_name = prompt_line("Name", &cur_dir()).unwrap();
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

    RepoIndex {
        _type: "https://bahkat.org/Repository".to_owned(),
        agent: Some(RepoAgent::default()),
        base: base,
        name: name,
        description: description,
        primary_filter: primary_filter,
        channels: channels
    }
}

fn package_init() {
    let pkg_data = request_package_data();
    let json = serde_json::to_string_pretty(&pkg_data).unwrap();
    
    println!("\n{}\n", json);

    if prompt_question("Save index.json", true) {
        let mut file = File::create("index.json").unwrap();
        file.write_all(json.as_bytes()).unwrap();
        file.write(&[b'\n']).unwrap();
    }
}

fn repo_index_virtuals() {
    progress(Color::Green, "Generating", "virtuals index").unwrap();

    let pkg_path = env::current_dir().unwrap().join("virtuals");
    let mut map = HashMap::new();

    for x in fs::read_dir(&pkg_path).unwrap() {
        let path = x.unwrap().path();
        
        if !path.is_dir() {
            continue;
        }

        let indexes: Vec<VirtualIndex> = fs::read_dir(&path).unwrap()
            .map(|x| x.unwrap().path())
            .filter(|path| path.is_dir() && path.join("index.json").exists())
            .map(|path| {
                let file = File::open(path.join("index.json")).unwrap();
                let pkg_index: VirtualIndex = serde_json::from_reader(file)
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

    let json = serde_json::to_string_pretty(&map).unwrap();

    progress(Color::Green, "Writing", "virtuals index").unwrap();
    let mut file = File::create(&pkg_path.join("index.json")).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write(&[b'\n']).unwrap();
}

fn repo_index_packages() {
    progress(Color::Green, "Generating", "packages index").unwrap();

    let pkg_path = env::current_dir().unwrap().join("packages");
    let pkgs: Vec<PackageIndex> = fs::read_dir(&pkg_path)
        .unwrap()
        .map(|x| {
            x.unwrap().path()
        })
        .filter(|path| {
            path.is_dir() && path.join("index.json").exists()
        })
        .map(|path| {
            let file = File::open(path.join("index.json")).unwrap();
            let pkg_index: PackageIndex = serde_json::from_reader(file)
                .expect(path.join("index.json").to_str().unwrap());
            
            let msg = format!("{} {}", &pkg_index.id, &pkg_index.version);
            progress(Color::Yellow, "Inserting", &msg).unwrap();
            pkg_index
        })
        .collect();
    
    let mut map = HashMap::new();
    for pkg in pkgs.into_iter() {
        map.insert(pkg.id.to_owned(), pkg);
    }

    let json = serde_json::to_string_pretty(&map).unwrap();

    progress(Color::Green, "Writing", "packages index").unwrap();
    let mut file = File::create(&pkg_path.join("index.json")).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write(&[b'\n']).unwrap();
}

enum OpenIndexError {
    FileError(std::io::Error),
    JsonError(serde_json::Error)
}

fn open_repo(path: &Path) -> Result<RepoIndex, OpenIndexError> {
    let file = File::open(path.join("index.json"))
        .map_err(|e| OpenIndexError::FileError(e))?;
    let index = serde_json::from_reader(file)
        .map_err(|e| OpenIndexError::JsonError(e))?;
    Ok(index)
}

fn open_package(path: &Path) -> Result<PackageIndex, OpenIndexError> {
    let file = File::open(path.join("index.json"))
        .map_err(|e| OpenIndexError::FileError(e))?;
    let index = serde_json::from_reader(file)
        .map_err(|e| OpenIndexError::JsonError(e))?;
    Ok(index)
}

fn repo_init() {
    if open_repo(&env::current_dir().unwrap()).is_ok() {
        progress(Color::Red, "Error", "Repo already exists; aborting.").unwrap();
        return;
    }

    let repo_data = request_repo_data();
    let json = serde_json::to_string_pretty(&repo_data).unwrap();
    
    println!("\n{}\n", json);

    if !prompt_question("Save index.json and generate repo directories", true) {
        return;
    }

    let mut file = File::create("index.json").unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write(&[b'\n']).unwrap();

    fs::create_dir("packages").unwrap();
    fs::create_dir("virtuals").unwrap();

    repo_index();
}

fn repo_index_meta() {
    progress(Color::Green, "Generating", "repository index").unwrap();

    let repo_path = env::current_dir().unwrap();
    let file = File::open(repo_path.join("index.json")).unwrap();
    let mut repo_index: RepoIndex = serde_json::from_reader(file)
        .expect(repo_path.join("index.json").to_str().unwrap());

    repo_index.agent = Some(RepoAgent::default());
    let json = serde_json::to_string_pretty(&repo_index).unwrap();

    progress(Color::Green, "Writing", "repository index").unwrap();
    let mut file = File::create(&repo_path.join("index.json")).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write(&[b'\n']).unwrap();
}

fn repo_index() {
    if open_repo(&env::current_dir().unwrap()).is_err() {
        progress(Color::Red, "Error", "Repo does not exist or is invalid; aborting.").unwrap();
        return;
    }
    
    repo_index_meta();
    repo_index_packages();
    repo_index_virtuals();
}

fn package_installer(product_code: &str, installer: &str, type_: Option<&str>,
        args: Option<&str>, uninst_args: Option<&str>, url: &str, size: usize, 
        requires_reboot: bool, requires_uninst_reboot: bool) {
    let mut pkg = match open_package(&env::current_dir().unwrap()) {
        Ok(pkg) => pkg,
        Err(_) => {
            progress(Color::Red, "Error", "Package does not exist or is invalid; aborting").unwrap();
            return;
        }
    };

    let installer_file = File::open(installer).expect("Installer could not be opened.");
    let meta = installer_file.metadata().unwrap();
    let installer_size = meta.len() as usize;

    let installer_index = PackageIndexInstaller {
        url: url.to_owned(),
        type_: type_.map(|x| x.to_owned()),
        args: args.map(|x| x.to_owned()),
        uninstall_args: uninst_args.map(|x| x.to_owned()),
        product_code: product_code.to_owned(),
        requires_reboot: requires_reboot,
        requires_uninstall_reboot: requires_uninst_reboot,
        size: installer_size,
        installed_size: size,
        signature: None
    };

    pkg.installer = Some(installer_index);

    let json = serde_json::to_string_pretty(&pkg).unwrap();
    
    println!("\n{}\n", json);

    if !prompt_question("Save index.json", true) {
        return;
    }

    let mut file = File::create("index.json").unwrap();
    file.write_all(json.as_bytes()).unwrap();
    file.write(&[b'\n']).unwrap();
}

fn main() {
    let matches = App::new("Báhkat")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .version(crate_version!())
        .author("Brendan Molloy <brendan@bbqsrc.net>")
        .about("The last package manager. \"Bákhat\" is the nominative plural form for \"packages\" in Northern Sámi.")
        .subcommand(
            SubCommand::with_name("repo")
            .about("Repository-related subcommands")
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .subcommand(
                SubCommand::with_name("init")
                    .about("Initialise a Báhkat repository in the current working directory")
            )
            .subcommand(
                SubCommand::with_name("index")
                    .about("Regenerate packages and virtuals indexes from source indexes")
            )
        )
        .subcommand(
            SubCommand::with_name("init")
                .about("Initialise a package in the current working directory")
        )
        .subcommand(
            // TODO: this currently is hardcoded for Windows only.
            SubCommand::with_name("installer")
                .about("Inject installer data into package index")
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
                    .short("s")
                    .long("silent-args")
                    .takes_value(true)
                )
                .arg(Arg::with_name("uninst-args")
                    .value_name("ARGS")
                    .help("Arguments to uninstaller for it to run silently")
                    .short("S")
                    .long("silent-uninst-args")
                    .takes_value(true)
                )
                .arg(Arg::with_name("url")
                    .value_name("URL")
                    .help("The URL where the installer will be downloaded")
                    .short("u")
                    .long("url")
                    .takes_value(true)
                    .required(true)
                )
                .arg(Arg::with_name("installed-size")
                    .value_name("SIZE")
                    .help("The size on disk when the package is installed")
                    .short("z")
                    .long("size")
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
        )
        .get_matches();
    
    match matches.subcommand() {
        ("init", _) => package_init(),
        ("installer", Some(matches)) => {
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

            package_installer(product_code, installer, type_, args, uninstall_args, url, 
                size, requires_reboot, requires_uninst_reboot);
        }
        ("repo", Some(matches)) => {
            match matches.subcommand() {
                ("init", _) => repo_init(),
                ("index", _) => repo_index(),
                _ => {}
            }
        }
        _ => {}
    }
}
