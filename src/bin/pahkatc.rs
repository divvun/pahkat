#[macro_use]
extern crate clap;
extern crate pahkat;
extern crate pahkat_client;
extern crate sentry;

use clap::{App, AppSettings, Arg, SubCommand};
#[cfg(prefix)]
use std::path::{Path, PathBuf};

use pahkat::types::{Package, MacOSInstallTarget};
use pahkat_client::*;
use sentry::integrations::panic::register_panic_handler;
use std::sync::Arc;

#[cfg(prefix)]
use pahkat_client::tarball::*;
#[cfg(target_os = "macos")]
use pahkat_client::macos::*;
#[cfg(windows)]
use pahkat_client::windows::*;

const DSN: &'static str = "https://0a0fc86e9d2447e8b0b807087575e8c6:3d610a0fea7b49d6803061efa16c2ddc@sentry.io/301711";

fn main() {
    std::mem::forget(sentry::init(DSN));
    register_panic_handler();

    let mut app = App::new("Páhkat")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .version(crate_version!())
        .author("Brendan Molloy <brendan@bbqsrc.net>")
        .about("The last package manager. \"Pákhat\" is the nominative plural form for \"packages\" in Northern Sámi.")
        .subcommand(
            SubCommand::with_name("ipc").setting(AppSettings::Hidden)
        );

    if cfg!(target_os = "macos") {
        app = app.subcommand(
            SubCommand::with_name("macos")
            .about("MacOS-specific commands")
            .subcommand(
                SubCommand::with_name("init")
                    .about("Create config")
                    .arg(Arg::with_name("url")
                        .value_name("URL")
                        .help("URL for repository to use.")
                        .short("u")
                        .long("url")
                        .takes_value(true)
                        .multiple(true)
                        .required(true))
                    .arg(Arg::with_name("cache-dir")
                        .value_name("CACHE")
                        .short("c")
                        .long("cache-dir")
                        .takes_value(true))
            )
            .subcommand(
                SubCommand::with_name("list")
                    .about("List packages in repository.")
            )
            .subcommand(
                SubCommand::with_name("install")
                .about("Install a package.")
                .arg(Arg::with_name("package-id")
                    .value_name("PKGID")
                    .help("The package identifier to install")
                    .multiple(true)
                    .required(true))
                .arg(Arg::with_name("user-target")
                    .help("Install into user target")
                    .short("u")
                    .long("user"))
            )
            .subcommand(
                SubCommand::with_name("uninstall")
                .about("Uninstall a package.")
                .arg(Arg::with_name("package-id")
                    .value_name("PKGID")
                    .help("The package identifier to install")
                    .required(true))
                .arg(Arg::with_name("user-target")
                    .help("Install into user target")
                    .short("u")
                    .long("user"))
            )
            .subcommand(
                SubCommand::with_name("status")
                .about("Query status of a package identifier")
                .arg(Arg::with_name("package-id")
                    .value_name("PKGID")
                    .help("The package identifier to query")
                    .required(true))
                .arg(Arg::with_name("user-target")
                    .help("Install into user target")
                    .short("u")
                    .long("user"))
            )
        );
    }

    if cfg!(windows) {
        app = app.subcommand(
            SubCommand::with_name("windows")
            .about("Windows-specific commands")
            .subcommand(
                SubCommand::with_name("init")
                    .about("Create config")
                    .arg(Arg::with_name("url")
                        .value_name("URL")
                        .help("URL for repository to use.")
                        .short("u")
                        .long("url")
                        .multiple(true)
                        .takes_value(true)
                        .required(true))
                    .arg(Arg::with_name("cache-dir")
                        .value_name("CACHE")
                        .short("c")
                        .long("cache-dir")
                        .takes_value(true)
                        .required(true))
            )
            .subcommand(
                SubCommand::with_name("list")
                    .about("List packages in repository.")
            )
            .subcommand(
                SubCommand::with_name("install")
                .about("Install a package.")
                .arg(Arg::with_name("package-id")
                    .value_name("PKGID")
                    .help("The package identifier to install")
                    .required(true))
            )
            .subcommand(
                SubCommand::with_name("uninstall")
                .about("Uninstall a package.")
                .arg(Arg::with_name("package-id")
                    .value_name("PKGID")
                    .help("The package identifier to install")
                    .required(true))
            )
            .subcommand(
                SubCommand::with_name("status")
                .about("Query status of a package identifier")
                .arg(Arg::with_name("package-id")
                    .value_name("PKGID")
                    .help("The package identifier to query")
                    .required(true))
            ));
    }

    if cfg!(prefix) {
        app = app.subcommand(
            SubCommand::with_name("prefix")
                .about("Commands for managing an installation prefix")
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
                    .arg(Arg::with_name("cache-dir")
                        .value_name("CACHE")
                        .help("Cache directory to use.")
                        .short("c")
                        .long("cache-dir")
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
        );
    }

    match app.get_matches().subcommand() {
        #[cfg(feature="ipc")]
        ("ipc", _) => {
            ipc::start();
        },
        #[cfg(target_os="macos")]
        ("macos", Some(matches)) => {
            match matches.subcommand() {
                ("status", Some(matches)) => {
                    let package_id = matches.value_of("package-id").expect("package-id to always exist");
                    let is_user = matches.is_present("user-target");
                    
                    let config = StoreConfig::load_or_default();
                    let repos = config.repos()
                        .iter()
                        .map(|record| Repository::from_url(&record.url).unwrap())
                        .collect::<Vec<_>>();

                    let store = MacOSPackageStore::new(config);
                    let package = match store.find_package(package_id) {
                        Some(v) => v,
                        None => {
                            println!("{}: No package found", &package_id);
                            return;
                        }
                    };
                    let target = match is_user {
                        true => MacOSInstallTarget::User,
                        false => MacOSInstallTarget::System
                    };
                    
                    let status = store.status(&package, target);

                    match status {
                        Ok(v) => println!("{}: {}", &package_id, v),
                        Err(e) => println!("{}: {}", &package_id, e)
                    };
                },
                ("uninstall", Some(matches)) => {
                    let package_id = matches.value_of("package-id").expect("package-id to always exist");
                    let is_user = matches.is_present("user-target");
                    
                    let config = StoreConfig::load_or_default();
                    let repos = config.repos()
                        .iter()
                        .map(|record| Repository::from_url(&record.url).unwrap())
                        .collect::<Vec<_>>();

                    let store = MacOSPackageStore::new(config);
                    let package = match store.find_package(package_id) {
                        Some(v) => v,
                        None => {
                            println!("{}: No package found", &package_id);
                            return;
                        }
                    };

                    let target = match is_user {
                        true => MacOSInstallTarget::User,
                        false => MacOSInstallTarget::System
                    };

                    let status = store.status(&package, target);
                    match status {
                        Ok(PackageStatus::UpToDate) | Ok(PackageStatus::RequiresUpdate) => {
                            let res = store.uninstall(&package, target);

                            match res {
                                Ok(v) => println!("{}: {}", &package_id, v),
                                Err(e) => println!("{}: error - {:?}", &package_id, e)
                            };
                        },
                        _ => {
                            println!("Nothing to do for identifier {}", package_id);
                            return;
                        }
                    }
                }
                ("install", Some(matches)) => {
                    use pahkat_client::repo::PackageRecord;

                    let package_ids = matches.values_of("package-id").expect("package-id to always exist");
                    let is_user = matches.is_present("user-target");
                    
                    let config = StoreConfig::load_or_default();
                    let repos = config.repos()
                        .iter()
                        .map(|record| Repository::from_url(&record.url).unwrap())
                        .collect::<Vec<_>>();

                    let store = MacOSPackageStore::new(config);
                    let target = match is_user {
                        true => MacOSInstallTarget::User,
                        false => MacOSInstallTarget::System
                    };

                    let mut packages = vec![];
                    let mut errors = vec![];

                    for id in package_ids {
                        match store.find_package(id) {
                            Some(v) => packages.push(v),
                            None => errors.push(id)
                        };
                    }

                    if errors.len() > 0 {
                        println!("No packages found for: {}", errors.join(", "));
                        return;
                    }

                    let actions = packages.into_iter().filter(|p| {
                        match store.status(&p, target) {
                            Ok(PackageStatus::NotInstalled) | Ok(PackageStatus::RequiresUpdate) => true,
                            _ => {
                                println!("{} already installed; skipping.", p.id().id);
                                false
                            }
                        }
                    }).map(|p| {
                        PackageAction {
                            package: p,
                            action: PackageActionType::Install,
                            target
                        }
                    }).collect::<Vec<_>>();

                    for action in actions.iter() {
                        let pb = indicatif::ProgressBar::new(0);
                        pb.set_style(indicatif::ProgressStyle::default_bar()
                            .template("{spinner:.green} {prefix} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                            .progress_chars("#>-"));
                        pb.set_prefix(&action.package.id().id);
                        
                        let progress = move |cur, max| {
                            pb.set_length(max);
                            pb.set_position(cur);

                            if cur >= max {
                                pb.finish_and_clear();
                            }
                        };
                        let _pkg_path = store.download(&action.package, progress).unwrap();
                    }

                    let mut tx = PackageTransaction::new(Arc::new(store), actions);
                    tx.process(|key, event| {
                        println!("{}: {:?}", key.id, event);
                    });

                    // impl PackageTransaction {
                    // pub fn new(
                    //     store: Arc<MacOSPackageStore>,
                    //     actions: Vec<PackageAction>

                    // let package = match store.find_package(package_id) {
                    //     Some(v) => v,
                    //     None => {
                    //         println!("{}: No package found", &package_id);
                    //         return;
                    //     }
                    // };

                    // let status = store.status(&package, target);
                    // match status {
                    //     Ok(PackageStatus::NotInstalled) | Ok(PackageStatus::RequiresUpdate) => {
                    //         let pb = indicatif::ProgressBar::new(0);
                    //         pb.set_style(indicatif::ProgressStyle::default_bar()
                    //             .template("{spinner:.green} {prefix} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                    //             .progress_chars("#>-"));
                    //         pb.set_prefix(&package_id);
                            
                    //         let progress = move |cur, max| {
                    //             pb.set_length(max);
                    //             pb.set_position(cur);

                    //             if cur >= max {
                    //                 pb.finish_and_clear();
                    //             }
                    //         };
                    //         let _pkg_path = store.download(&package, progress).unwrap();
                    //         let res = store.install(&package, target);

                    //         match res {
                    //             Ok(v) => println!("{}: {}", &package_id, v),
                    //             Err(e) => println!("{}: error - {:?}", &package_id, e)
                    //         };
                    //     },
                    //     _ => {
                    //         println!("Nothing to do for identifier {}", package_id);
                    //         return;
                    //     }
                    // }
                },
                ("init", Some(matches)) => {
                    let urls = matches.values_of("url").unwrap();
                    let store = StoreConfig::load_or_default();
                    
                    match matches.value_of("cache-dir") {
                        Some(v) => { store.set_cache_path(std::path::PathBuf::from(v)).expect("set cache path"); },
                        None => {}
                    };

                    for url in urls {
                        store.add_repo(RepoRecord {
                            url: url::Url::parse(url).unwrap(),
                            channel: "stable".into()
                        }).expect("add repo");
                    }
                },
                ("list", Some(_matches)) => {
                    let config = StoreConfig::load_or_default();
                    let repos = config.repos()
                        .iter()
                        .map(|record| Repository::from_url(&record.url).unwrap())
                        .collect::<Vec<_>>();
                        
                    for (n, repo) in repos.iter().enumerate() {
                        println!("== Repository {}: {} ==", n, repo.meta().name.get("en").unwrap_or(&String::from("")));
                        let mut packages: Vec<&Package> = repo.packages().values().collect();
                        packages.sort_unstable_by(|a, b| a.id.cmp(&b.id));
                        for pkg in packages {
                            println!("{} {} ({}) — {}", pkg.id,
                                pkg.version,
                                pkg.name.get("en").unwrap_or(&"???".to_owned()),
                                pkg.description.get("en").unwrap_or(&"???".to_owned())
                            );
                        }
                    }
                },
                _ => {}
            }
        },
        #[cfg(windows)]
        ("windows", Some(matches)) => {
            match matches.subcommand() {
                ("init", Some(matches)) => {
                    let url = matches.value_of("url").expect("url to always exist");
                    let cache_dir = matches.value_of("cache-dir").expect("cache-dir to always exist");

                    windows::init(&url, &cache_dir);
                }
                ("list", Some(matches)) => {
                    let config = StoreConfig::load_or_default();
                    let repo = Repository::from_url(&config.url).unwrap();
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
                    let package_id = matches.value_of("package-id").expect("package-id to always exist");
                    let config = StoreConfig::load_or_default();
                    let repo = Repository::from_url(&config.url).expect("repository to load");
                    let package = match repo.packages().get(package_id) {
                        Some(v) => v,
                        None => {
                            println!("{}: No package found", &package_id);
                            return;
                        }
                    };

                    let store = WindowsPackageStore::new(&repo, &config);
                    // TODO: the config is responsible for creating this.
                    let package_cache = store.download_path(&package);
                    println!("{:?}", &package_cache);
                    if !package_cache.exists() {
                        fs::create_dir_all(&package_cache).expect("create package cache never fails");
                    }

                    let status = store.status(&package);
                    match status {
                        Ok(PackageStatus::NotInstalled) | Ok(PackageStatus::RequiresUpdate) => {
                            let pkg_path = package.download(&package_cache, Some(|cur, max| {
                                println!("{}/{} bytes", cur, max);
                            })).expect("download never has a severe error");
                            let res = store.install(package).expect("install never fails");

                            // match res {
                            //     Ok(v) => println!("{}: {:?}", &package_id, v),
                            //     Err(e) => println!("{}: error - {:?}", &package_id, e)
                            // };
                        },
                        _ => {
                            println!("Nothing to do for identifier {}", package_id);
                            return;
                        }
                    }
                },
                ("uninstall", Some(matches)) => {
                    let package_id = matches.value_of("package-id").expect("package-id to always exist");
                    let config = StoreConfig::load_or_default();
                    let repo = Repository::from_url(&config.url).expect("repository to load");
                    let package = match repo.packages().get(package_id) {
                        Some(v) => v,
                        None => {
                            println!("{}: No package found", &package_id);
                            return;
                        }
                    };

                    let store = WindowsPackageStore::new(&repo, &config);
                    let status = store.status(&package);
                    match status {
                        Ok(PackageStatus::UpToDate) | Ok(PackageStatus::RequiresUpdate) => {
                            let res = store.uninstall(package).expect("uninstallation can never fail");

                            // match res {
                            //     Ok(v) => println!("{}: {:?}", &package_id, v),
                            //     Err(e) => println!("{}: error - {:?}", &package_id, e)
                            // };
                        },
                        _ => {
                            println!("Nothing to do for identifier {}", package_id);
                            return;
                        }
                    }

                },
                ("status", Some(matches)) => {
                    let package_id = matches.value_of("package-id").unwrap();
                    let config = StoreConfig::load_or_default();
                    let repo = Repository::from_url(&config.url).unwrap();

                    let package = match repo.packages().get(package_id) {
                        Some(v) => v,
                        None => {
                            println!("{}: No package found", &package_id);
                            return;
                        }
                    };
                    let store = WindowsPackageStore::new(&repo, &config);
                    let status = store.status(&package);

                    match status {
                        Ok(v) => println!("{}: {}", &package_id, v),
                        Err(e) => println!("{}: {}", &package_id, e)
                    };
                },
                _ => {}
            }
        }
        #[cfg(prefix)]
        ("prefix", Some(matches)) => {
            match matches.subcommand() {
                ("init", Some(matches)) => {
                    let prefix = matches.value_of("prefix").unwrap();
                    let url = matches.value_of("url").unwrap();
                    Prefix::create(Path::new(prefix), url).unwrap();
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
                            let pkg_path = package.download(&pkg_dir, Some(|cur, max| println!("{}/{}", cur, max))).unwrap();
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
                }
                _ => {}
            }
        }
        _ => {}
    }
}

#[cfg(prefix)]
fn open_prefix(path: &str) -> Result<Prefix, ()> {
    let prefix = Prefix::open(Path::new(path)).unwrap();
    Ok(prefix)
}