#[macro_use]
extern crate clap;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use clap::*;
use std::env;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

mod cli;
mod types;

use cli::*;
use types::*;

fn default_pkg_id() -> String {
    env::current_dir().unwrap()
        .components().last().unwrap()
        .as_os_str()
        .to_string_lossy()
        .to_lowercase()
}

fn request_package_data() -> PackageIndex {
    let package_id = prompt_line("Package identifier", &default_pkg_id()).unwrap();
    
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

fn new_package() {
    let pkg_data = request_package_data();
    let json = serde_json::to_string_pretty(&pkg_data).unwrap();
    
    println!("\n{}\n", json);

    if prompt_question("Save index.json", true) {
        let mut file = File::create("index.json").unwrap();
        file.write_all(json.as_bytes()).unwrap();
        file.write(&[b'\n']).unwrap();
    }
}

fn main() {
    let matches = App::new("Báhkat")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .version(crate_version!())
        .author("Brendan Molloy <brendan@bbqsrc.net>")
        .about("The last package manager. \"Bákhat\" is the nominative plural form for \"packages\" in Northern Sámi.")
        .subcommand(
            SubCommand::with_name("init")
                .about("Initialise a package in the current working directory")
        )
        .get_matches();
    
    match matches.subcommand() {
        ("init", _) => {
            new_package()
        },
        _ => {}
    }
}