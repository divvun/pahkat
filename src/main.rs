#[macro_use]
extern crate clap;
extern crate pahkat;
extern crate pahkat_client;

use clap::{App, AppSettings, Arg, SubCommand};
use pahkat::types::*;
use pahkat_client::*;
use std::path::Path;

fn main() {
    let matches = App::new("Páhkat")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .version(crate_version!())
        .author("Brendan Molloy <brendan@bbqsrc.net>")
        .about("The last package manager. \"Pákhat\" is the nominative plural form for \"packages\" in Northern Sámi.")
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
                .arg(Arg::with_name("prefix")
                    .value_name("PREFIX")
                    .help("The prefix for managing repository.")
                    .short("p")
                    .long("prefix")
                    .takes_value(true)
                    .required(true))
                .arg(Arg::with_name("package")
                    .value_name("PKGID")
                    .help("The package identifier to install")
                    .required(true))
        )
        .subcommand(
            SubCommand::with_name("uninstall")
                .about("Uninstall a package.")
                .arg(Arg::with_name("prefix")
                    .value_name("PREFIX")
                    .help("The prefix for managing repository.")
                    .short("p")
                    .long("prefix")
                    .takes_value(true)
                    .required(true))
                .arg(Arg::with_name("package")
                    .value_name("PKGID")
                    .help("The package identifier to uninstall")
                    .required(true))
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
        ("install", Some(matches)) => {
            let package_id = matches.value_of("package").unwrap();
            let prefix = open_prefix(matches.value_of("prefix").unwrap()).unwrap();
            let repo = Repository::from_url(&prefix.config().url).unwrap();

            let package = match repo.package(&package_id) {
                Some(v) => v,
                None => {
                    println!("No package found with identifier {}.", package_id);
                    return;
                }
            };

            let status = match prefix.store().status(package) {
                Ok(v) => v,
                Err(_) => {
                    println!("An error occurred checking the status of the package.");
                    return;
                }
            };

            match status {
                PackageStatus::NotInstalled | PackageStatus::RequiresUpdate => {
                    let pkg_dir = prefix.store().create_cache();
                    let pkg_path = package.download(&pkg_dir).unwrap();
                    prefix.store().install(package, &pkg_path).unwrap();
                },
                _ => {
                    println!("Nothing to do for identifier {}", package_id);
                    return;
                }
            }
        },
        ("uninstall", Some(matches)) => {
            let package_id = matches.value_of("package").unwrap();
            let prefix = open_prefix(matches.value_of("prefix").unwrap()).unwrap();
            let repo = Repository::from_url(&prefix.config().url).unwrap();

            let package = match repo.package(&package_id) {
                Some(v) => v,
                None => {
                    println!("No package found with identifier {}.", package_id);
                    return;
                }
            };

            let status = match prefix.store().status(package) {
                Ok(v) => v,
                Err(_) => {
                    println!("An error occurred checking the status of the package.");
                    return;
                }
            };

            match status {
                PackageStatus::UpToDate | PackageStatus::RequiresUpdate => {
                    prefix.store().uninstall(package).unwrap();
                },
                _ => {
                    println!("Nothing to do for identifier {}", package_id);
                    return;
                }
            }
        },
        _ => {}
    }
}

fn open_prefix(path: &str) -> Result<Prefix, ()> {
    let prefix = Prefix::open(Path::new(path)).unwrap();
    Ok(prefix)
}