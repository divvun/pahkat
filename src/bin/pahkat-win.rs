extern crate pahkat_types as pahkat;

use clap::{crate_version, App, AppSettings, Arg, SubCommand};

use pahkat_client::*;
use pahkat_types::{InstallTarget, Package};
use sentry::integrations::panic::register_panic_handler;
use std::sync::Arc;

const DSN: &'static str =
    "https://0a0fc86e9d2447e8b0b807087575e8c6:3d610a0fea7b49d6803061efa16c2ddc@sentry.io/301711";

#[cfg(not(windows))]
fn main() {
    eprintln!("This is not supported on your OS.");
}

#[cfg(windows)]
fn main() {
    use pahkat_client::windows::*;

    std::mem::forget(sentry::init(DSN));
    register_panic_handler();

    let mut app = App::new("Páhkat")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .version(crate_version!())
        .author("Brendan Molloy <brendan@bbqsrc.net>")
        .about("The last package manager. \"Pákhat\" is the nominative plural form for \"packages\" in Northern Sámi.")
                .subcommand(
                    SubCommand::with_name("init")
                        .about("Create config")
                        .arg(
                            Arg::with_name("url")
                                .value_name("URL")
                                .help("URL for repository to use.")
                                .short("u")
                                .long("url")
                                .multiple(true)
                                .takes_value(true)
                                .required(true),
                        )
                        .arg(
                            Arg::with_name("cache-dir")
                                .value_name("CACHE")
                                .short("c")
                                .long("cache-dir")
                                .takes_value(true)
                                .required(true),
                        ),
                )
                .subcommand(SubCommand::with_name("list").about("List packages in repository."))
                .subcommand(
                    SubCommand::with_name("install")
                        .about("Install a package.")
                        .arg(
                            Arg::with_name("package-id")
                                .value_name("PKGID")
                                .help("The package identifier to install")
                                .required(true),
                        )
                        .arg(
                            Arg::with_name("user-target")
                                .help("Install into user target")
                                .short("u")
                                .long("user"),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("uninstall")
                        .about("Uninstall a package.")
                        .arg(
                            Arg::with_name("package-id")
                                .value_name("PKGID")
                                .help("The package identifier to install")
                                .required(true),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("status")
                        .about("Query status of a package identifier")
                        .arg(
                            Arg::with_name("package-id")
                                .value_name("PKGID")
                                .help("The package identifier to query")
                                .required(true),
                        ),
                );

    match app.get_matches() {
        // ("init", Some(matches)) => {
        //     let url = matches.value_of("url").expect("url to always exist");
        //     let cache_dir = matches.value_of("cache-dir").expect("cache-dir to always exist");

        //     windows::init(&url, &cache_dir);
        // }
        // ("list", Some(matches)) => {
        //     let config = StoreConfig::load_or_default();
        //     let repo = Repository::from_url(&config.url).unwrap();
        //     let mut packages: Vec<&Package> = repo.packages().values().collect();
        //     packages.sort_unstable_by(|a, b| a.id.cmp(&b.id));
        //     for pkg in packages {
        //         println!("{} {} ({}) — {}", pkg.id,
        //             pkg.version,
        //             pkg.name.get("en").unwrap_or(&"???".to_owned()),
        //             pkg.description.get("en").unwrap_or(&"???".to_owned())
        //         );
        //     }
        // },
        ("install", Some(matches)) => {
            // let package_id = matches.value_of("package-id").expect("package-id to always exist");
            // let config = StoreConfig::load_or_default();
            // let repo = Repository::from_url(&config.url).expect("repository to load");
            // let package = match repo.packages().get(package_id) {
            //     Some(v) => v,
            //     None => {
            //         println!("{}: No package found", &package_id);
            //         return;
            //     }
            // };

            // let store = WindowsPackageStore::new(&repo, &config);
            // // TODO: the config is responsible for creating this.
            // let package_cache = store.download_path(&package);
            // println!("{:?}", &package_cache);
            // if !package_cache.exists() {
            //     fs::create_dir_all(&package_cache).expect("create package cache never fails");
            // }

            // let status = store.status(&package);
            // match status {
            //     Ok(PackageStatus::NotInstalled) | Ok(PackageStatus::RequiresUpdate) => {
            //         let pkg_path = package.download(&package_cache, Some(|cur, max| {
            //             println!("{}/{} bytes", cur, max);
            //         })).expect("download never has a severe error");
            //         let res = store.install(package).expect("install never fails");

            //         // match res {
            //         //     Ok(v) => println!("{}: {:?}", &package_id, v),
            //         //     Err(e) => println!("{}: error - {:?}", &package_id, e)
            //         // };
            //     },
            //     _ => {
            //         println!("Nothing to do for identifier {}", package_id);
            //         return;
            //     }
            // }

            let package_ids = matches
                .values_of("package-id")
                .expect("package-id to always exist");
            let is_user = matches.is_present("user-target");

            let config = StoreConfig::load_or_default();
            let repos = config
                .repos()
                .iter()
                .map(|record| Repository::from_url(&record.url, record.channel.clone()).unwrap())
                .collect::<Vec<_>>();

            let store = Arc::new(WindowsPackageStore::new(config));
            let target = match is_user {
                true => InstallTarget::User,
                false => InstallTarget::System,
            };

            let mut keys = vec![];
            for id in package_ids.into_iter() {
                match store.find_package(id) {
                    Some(v) => keys.push(v.0),
                    None => {
                        eprintln!("No package found with id: {}", id);
                        return;
                    }
                }
            }
            let actions = keys
                .into_iter()
                .map(|k| PackageAction {
                    id: k,
                    action: PackageActionType::Install,
                    target,
                })
                .collect::<Vec<_>>();

            let mut transaction = match PackageTransaction::new(store.clone(), actions) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{:?}", e);
                    return;
                }
            };

            // Download all of the things
            for action in transaction.actions().iter() {
                let pb = indicatif::ProgressBar::new(0);
                pb.set_style(indicatif::ProgressStyle::default_bar()
                            .template("{spinner:.green} {prefix} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                            .progress_chars("#>-"));
                pb.set_prefix(&action.id.id);

                let progress = move |cur, max| {
                    pb.set_length(max);
                    pb.set_position(cur);

                    if cur >= max {
                        pb.finish_and_clear();
                    }
                };
                let _pkg_path = store.clone().download(&action.id, progress).unwrap();
            }

            transaction.process(|key, event| {
                println!("{}: {:?}", key.id, event);
            });
        }
        // ("uninstall", Some(matches)) => {
        //     let package_id = matches.value_of("package-id").expect("package-id to always exist");
        //     let config = StoreConfig::load_or_default();
        //     let repo = Repository::from_url(&config.url).expect("repository to load");
        //     let package = match repo.packages().get(package_id) {
        //         Some(v) => v,
        //         None => {
        //             println!("{}: No package found", &package_id);
        //             return;
        //         }
        //     };

        //     let store = WindowsPackageStore::new(&repo, &config);
        //     let status = store.status(&package);
        //     match status {
        //         Ok(PackageStatus::UpToDate) | Ok(PackageStatus::RequiresUpdate) => {
        //             let res = store.uninstall(package).expect("uninstallation can never fail");

        //             // match res {
        //             //     Ok(v) => println!("{}: {:?}", &package_id, v),
        //             //     Err(e) => println!("{}: error - {:?}", &package_id, e)
        //             // };
        //         },
        //         _ => {
        //             println!("Nothing to do for identifier {}", package_id);
        //             return;
        //         }
        //     }

        // },
        // ("status", Some(matches)) => {
        //     let package_id = matches.value_of("package-id").unwrap();
        //     let config = StoreConfig::load_or_default();
        //     let repo = Repository::from_url(&config.url).unwrap();

        //     let package = match repo.packages().get(package_id) {
        //         Some(v) => v,
        //         None => {
        //             println!("{}: No package found", &package_id);
        //             return;
        //         }
        //     };
        //     let store = WindowsPackageStore::new(&repo, &config);
        //     let status = store.status(&package);

        //     match status {
        //         Ok(v) => println!("{}: {}", &package_id, v),
        //         Err(e) => println!("{}: {}", &package_id, e)
        //     };
        // },
        _ => {}
    }
}
