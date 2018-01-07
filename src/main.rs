#[macro_use]
extern crate clap;
extern crate bahkat;
extern crate bahkat_client;

use clap::{App, AppSettings, Arg, SubCommand};
use bahkat::types::*;
use bahkat_client::*;
use std::path::Path;

fn main() {
    let matches = App::new("Báhkat")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .version(crate_version!())
        .author("Brendan Molloy <brendan@bbqsrc.net>")
        .about("The last package manager. \"Bákhat\" is the nominative plural form for \"packages\" in Northern Sámi.")
        .subcommand(
            SubCommand::with_name("init")
                .about("Create prefix.")
                .arg(Arg::with_name("prefix")
                    .value_name("PREFIX")
                    .help("The prefix for managing repository.")
                    .short("p")
                    .long("prefix")
                    .takes_value(true)
                    .required(true))
                .arg(Arg::with_name("url")
                    .value_name("URL")
                    .help("URL to repository to use.")
                    .short("u")
                    .long("url")
                    .takes_value(true)
                    .required(true))
        )
        .subcommand(
            SubCommand::with_name("list")
                .about("List packages in repository.")
                .arg(Arg::with_name("prefix")
                    .value_name("PREFIX")
                    .help("The prefix for managing repository.")
                    .short("p")
                    .long("prefix")
                    .takes_value(true)
                    .required(true))
        )
        .subcommand(
            SubCommand::with_name("install")
                .about("Install a package.")
        )
        .get_matches();

    match matches.subcommand() {
        ("init", Some(matches)) => {
            let prefix = matches.value_of("prefix").unwrap();
            let url = matches.value_of("url").unwrap();
            let config = StoreConfig { url: url.to_owned() };
            Prefix::create(Path::new(prefix), config).unwrap();
        },
        ("list", Some(matches)) => {
            let prefix_str = matches.value_of("prefix").unwrap();
            let prefix = Prefix::open(Path::new(prefix_str)).unwrap();

            let repo = Repository::from_url(&prefix.config().url).unwrap();
            let mut packages: Vec<&Package> = repo.packages().values().collect();
            packages.sort_unstable_by(|a, b| a.id.cmp(&b.id));
            for pkg in packages {
                println!("{} {} ({}) — {}", pkg.id,
                    pkg.version,
                    pkg.name.get("en").unwrap_or(&"???".to_owned()),
                    pkg.description.get("en").unwrap_or(&"???".to_owned())
                );
            }
        },
        _ => {}
    }
}