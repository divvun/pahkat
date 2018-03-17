#[macro_use]
extern crate clap;
extern crate pahkat;
extern crate pahkat_client;

use clap::{App, AppSettings, Arg, SubCommand};
use std::path::Path;
use std::env;
use std::fs;

use pahkat::types::{Package, MacOSInstallTarget};
use pahkat_client::*;

#[cfg(prefix)]
use pahkat_client::tarball::*;
#[cfg(target_os = "macos")]
use pahkat_client::macos::*;
#[cfg(windows)]
use pahkat_client::windows::*;


fn main() {
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

                    let config_path = env::home_dir().expect("home directory to always exist")
                        .join("Library/Application Support/Pahkat/config.json");
                    let config = StoreConfig::load(&config_path).unwrap();
                    let repo = Repository::from_url(&config.url).unwrap();

                    let package = match repo.packages().get(package_id) {
                        Some(v) => v,
                        None => {
                            println!("{}: No package found", &package_id);
                            return;
                        }
                    };
                    let store = MacOSPackageStore::new(&repo, &config);
                    let status = store.status(&package, MacOSInstallTarget::User);

                    match status {
                        Ok(v) => println!("{}: {}", &package_id, v),
                        Err(e) => println!("{}: {}", &package_id, e)
                    };
                },
                ("uninstall", Some(matches)) => {
                    let package_id = matches.value_of("package-id").unwrap();
                    let is_user = matches.is_present("user-target");

                    let config_path = env::home_dir().unwrap()
                        .join("Library/Application Support/Pahkat/config.json");
                    let config = StoreConfig::load(&config_path).unwrap();
                    let repo = Repository::from_url(&config.url).unwrap();

                    let package = match repo.packages().get(package_id) {
                        Some(v) => v,
                        None => {
                            println!("{}: No package found", &package_id);
                            return;
                        }
                    };

                    let store = MacOSPackageStore::new(&repo, &config);
                    let target = match is_user {
                        true => MacOSInstallTarget::User,
                        false => MacOSInstallTarget::System
                    };

                    let status = store.status(&package, target);
                    match status {
                        Ok(PackageStatus::UpToDate) | Ok(PackageStatus::RequiresUpdate) => {
                            let res = store.uninstall(package, target).unwrap();

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
                }
                ("install", Some(matches)) => {
                    let package_id = matches.value_of("package-id").unwrap();
                    let is_user = matches.is_present("user-target");

                    let config_path = env::home_dir().unwrap()
                        .join("Library/Application Support/Pahkat/config.json");
                    let config = StoreConfig::load(&config_path).unwrap();
                    let repo = Repository::from_url(&config.url).unwrap();

                    let package = match repo.packages().get(package_id) {
                        Some(v) => v,
                        None => {
                            println!("{}: No package found", &package_id);
                            return;
                        }
                    };

                    let store = MacOSPackageStore::new(&repo, &config);
                    let target = match is_user {
                        true => MacOSInstallTarget::User,
                        false => MacOSInstallTarget::System
                    };
                    
                    // TODO: the config is responsible for creating this.
                    let package_cache = store.download_path(&package);
                    if !package_cache.exists() {
                        fs::create_dir_all(&package_cache).unwrap();
                    }

                    let status = store.status(&package, target);
                    match status {
                        Ok(PackageStatus::NotInstalled) | Ok(PackageStatus::RequiresUpdate) => {
                            let pkg_path = package.download(&package_cache,
                                Some(|cur, max| println!("{}/{}", cur, max))).unwrap();
                            let res = store.install(package, target).unwrap();

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
                ("init", Some(matches)) => {
                    let url = matches.value_of("url").unwrap();
                    let cache_dir = matches.value_of("cache-dir").unwrap();

                    macos::init(&url, &cache_dir);
                },
                ("list", Some(matches)) => {
                    let config_path = env::home_dir().unwrap()
                        .join("Library/Application Support/Pahkat/config.json");
                    let config = StoreConfig::load(&config_path).unwrap();
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