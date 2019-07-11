#![cfg(feature = "prefix")]

extern crate pahkat_types as pahkat;
use clap::{crate_version, App, AppSettings, Arg, SubCommand};
use std::path::Path;
use std::sync::Arc;

use pahkat_client::*;
use pahkat_types::Package;
use sentry::integrations::panic::register_panic_handler;

use pahkat_client::tarball::PrefixPackageStore;
use pahkat_client::{transaction::PackageTransaction, PackageAction};

const DSN: &'static str =
    "https://0a0fc86e9d2447e8b0b807087575e8c6:3d610a0fea7b49d6803061efa16c2ddc@sentry.io/301711";

fn main() {
    better_panic::Settings::debug()
        .most_recent_first(false)
        .lineno_suffix(true)
        .install();
    std::mem::forget(sentry::init(DSN));
    register_panic_handler();

    let app = App::new("Páhkat")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .version(crate_version!())
        .author("Brendan Molloy <brendan@bbqsrc.net>")
        .about("The last package manager. \"Pákhat\" is the nominative plural form for \"packages\" in Northern Sámi.")
        .subcommand(
            SubCommand::with_name("init")
                .about("Create prefix.")
                .arg(
                    Arg::with_name("prefix")
                        .value_name("PREFIX")
                        .help("The prefix for managing repository.")
                        .short("p")
                        .long("prefix")
                        .takes_value(true)
                        .required(true),
                )
                // .arg(
                //     Arg::with_name("url")
                //         .value_name("URL")
                //         .help("URL to repository to use.")
                //         .short("u")
                //         .long("url")
                //         .takes_value(true)
                //         .required(true),
                // )
            ).subcommand(
                SubCommand::with_name("list")
                    .about("List packages in repository.")
                    .arg(
                        Arg::with_name("prefix")
                            .value_name("PREFIX")
                            .help("The prefix for managing repository.")
                            .short("p")
                            .long("prefix")
                            .takes_value(true)
                            .required(true),
                    ),
            ).subcommand(
                SubCommand::with_name("install")
                    .about("Install a package.")
                    .arg(
                        Arg::with_name("prefix")
                            .value_name("PREFIX")
                            .help("The prefix for managing repository.")
                            .short("p")
                            .long("prefix")
                            .takes_value(true)
                            .required(true),
                    )
                    .arg(
                        Arg::with_name("package-id")
                            .value_name("PKGID")
                            .help("The package identifier to install")
                            .multiple(true)
                            .required(true)
                    )
            ).subcommand(
                SubCommand::with_name("uninstall")
                    .about("Uninstall a package.")
                    .arg(
                        Arg::with_name("prefix")
                            .value_name("PREFIX")
                            .help("The prefix for managing repository.")
                            .short("p")
                            .long("prefix")
                            .takes_value(true)
                            .required(true),
                    )
                    .arg(
                        Arg::with_name("package")
                            .value_name("PKGID")
                            .help("The package identifier to uninstall")
                            .required(true),
                    ),
            );

    match app.get_matches().subcommand() {
        ("init", Some(matches)) => {
            let prefix = matches.value_of("prefix").unwrap();
            // let url = matches.value_of("url").unwrap();
            PrefixPackageStore::create(Path::new(prefix)).unwrap();
        }
        ("repo", Some(matches)) => match matches.subcommand() {
            ("add", Some(matches)) => {}
            ("list", Some(matches)) => {}
            ("remove", Some(matches)) => {}
            _ => {}
        },
        ("list", Some(matches)) => {
            let prefix_str = matches.value_of("prefix").unwrap();
            let prefix = PrefixPackageStore::open(prefix_str).unwrap();

            let repos = prefix
                .config()
                .read()
                .unwrap()
                .repos()
                .iter()
                .map(|record| Repository::from_url(&record.url, record.channel.clone()).unwrap())
                .collect::<Vec<_>>();
            for repo in repos {
                println!("Repo: {}", &repo.meta().base);

                let mut packages: Vec<&Package> = repo.packages().values().collect();
                packages.sort_unstable_by(|a, b| a.id.cmp(&b.id));
                for pkg in packages {
                    println!(
                        "{} {} ({}) — {}",
                        pkg.id,
                        pkg.version,
                        pkg.name.get("en").unwrap_or(&"???".to_owned()),
                        pkg.description.get("en").unwrap_or(&"???".to_owned())
                    );
                }
            }
        }
        ("install", Some(matches)) => {
            let prefix_str = matches.value_of("prefix").unwrap();
            let store = PrefixPackageStore::open(prefix_str).unwrap();
            let package_ids = matches
                .values_of("package-id")
                .expect("package-id to always exist");

            // let config = prefix.config();
            // let store = prefix.store();

            let mut keys = vec![];
            for id in package_ids.into_iter() {
                match store.find_package_by_id(id) {
                    Some(v) => keys.push(v.0),
                    None => {
                        eprintln!("No package found with id: {}", id);
                        return;
                    }
                }
            }
            let actions = keys
                .into_iter()
                .map(|k| PackageAction::install(k, ()))
                .collect::<Vec<_>>();

            use pahkat_client::transaction::PackageStore;
            let store = store.into_arc();
            let mut transaction = match PackageTransaction::new(Arc::clone(&store), actions) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{:?}", e);
                    return;
                }
            };

            // Download all of the things
            for action in transaction.actions().iter() {
                let store = &store;
                let pb = indicatif::ProgressBar::new(0);
                pb.set_style(indicatif::ProgressStyle::default_bar()
                    .template("{spinner:.green} {prefix} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                    .progress_chars("#>-"));
                pb.set_prefix(&action.id.id);

                let progress = Box::new(move |cur, max| {
                    pb.set_length(max);
                    pb.set_position(cur);

                    if cur >= max {
                        pb.finish_and_clear();
                    }
                });
                let _pkg_path = store.download(&action.id, progress).unwrap();
            }

            transaction.process(|key, event| {
                println!("{}: {:?}", key.id, event);
            });
        }
        // ("uninstall", Some(matches)) => {
        // let package_id = matches.value_of("package").unwrap();
        // let prefix = open_prefix(matches.value_of("prefix").unwrap()).unwrap();
        // let repo = Repository::from_url(&prefix.config().url, "stable".into()).unwrap();

        // let package = match repo.package(&package_id) {
        //     Some(v) => v,
        //     None => {
        //         println!("No package found with identifier {}.", package_id);
        //         return;
        //     }
        // };

        // let status = match prefix.store().status(package) {
        //     Ok(v) => v,
        //     Err(_) => {
        //         println!("An error occurred checking the status of the package.");
        //         return;
        //     }
        // };

        // match status {
        //     PackageStatus::UpToDate | PackageStatus::RequiresUpdate => {
        //         prefix.store().uninstall(package).unwrap();
        //     }
        //     _ => {
        //         println!("Nothing to do for identifier {}", package_id);
        //         return;
        //     }
        // }
        // }
        _ => {}
    }
}
