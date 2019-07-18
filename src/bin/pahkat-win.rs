#![cfg(feature = "windows")]

extern crate pahkat_types as pahkat;

use clap::{crate_version, App, AppSettings, Arg, SubCommand};

use pahkat_client::transaction::PackageStore;
use pahkat_client::windows::*;
use pahkat_types::{InstallTarget, Package};

use itertools::Itertools;
use sentry::integrations::panic::register_panic_handler;
use std::sync::Arc;

const DSN: &'static str =
    "https://0a0fc86e9d2447e8b0b807087575e8c6:3d610a0fea7b49d6803061efa16c2ddc@sentry.io/301711";

#[cfg(not(windows))]
fn main() {
    std::mem::forget(sentry::init(DSN));
    register_panic_handler();
    log::error!("This is not supported on your OS.");
}

fn arg_pkgid() -> Arg<'static, 'static> {
    Arg::with_name("package-id")
        .value_name("PKGID")
        .help("The package identifier to install")
        .required(true)
        .multiple(true)
}

#[inline(always)]
fn subcommand_init() -> App<'static, 'static> {
    SubCommand::with_name("init").about("Prepare the package manager for first time use.")
}

#[inline(always)]
fn subcommand_install() -> App<'static, 'static> {
    SubCommand::with_name("install")
        .about("Install packages.")
        .arg(arg_pkgid())
        .arg(
            Arg::with_name("user-target")
                .help("Install into user target")
                .short("u")
                .long("user"),
        )
}

#[inline(always)]
fn subcommand_uninstall() -> App<'static, 'static> {
    SubCommand::with_name("uninstall")
        .about("Uninstall packages.")
        .arg(arg_pkgid())
        .arg(
            Arg::with_name("user-target")
                .help("Install into user target")
                .short("u")
                .long("user"),
        )
}

#[inline(always)]
fn subcommand_info() -> App<'static, 'static> {
    SubCommand::with_name("info")
        .about("Show information about a package (or packages).")
        .arg(arg_pkgid())
}

#[inline(always)]
fn subcommand_search() -> App<'static, 'static> {
    SubCommand::with_name("search")
}

#[inline(always)]
fn subcommand_status() -> App<'static, 'static> {
    SubCommand::with_name("status")
}

#[inline(always)]
fn subcommand_download() -> App<'static, 'static> {
    SubCommand::with_name("download")
}

const AUTHOR: &str = "Brendan Molloy <brendan@bbqsrc.net>";
const ABOUT: &str = "The last package manager. \"Pákhat\" is the nominative plural form for \"packages\" in Northern Sámi.";

fn init() {
    let store = WindowsPackageStore::default();
    store.config().write().unwrap().save();
}

#[derive(Debug)]
enum CliError {
    NoPackageFound(String),
}

trait AsStr<T> {
    fn as_str<'s>(&'s self) -> T;
}

impl<'a> AsStr<Option<&'a str>> for Option<&'a String> {
    fn as_str<'s>(&'s self) -> Option<&'a str> {
        match self {
            Some(s) => Some(s.as_str()),
            None => None,
        }
    }
}

use pahkat_client::transaction::{PackageAction, PackageTransaction};

pub type WindowsTarget = pahkat_types::InstallTarget;

fn install(package_names: Vec<&str>, target: WindowsTarget) -> Result<(), CliError> {
    let store = Arc::new(WindowsPackageStore::default());
    let package_actions = package_names
        .into_iter()
        .map(|name| match store.find_package_by_id(&name) {
            Some(pkg) => Ok(PackageAction::install(pkg.0, target)),
            None => Err(CliError::NoPackageFound(name.to_string())),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let transaction = PackageTransaction::new(Arc::clone(&store) as _, package_actions).unwrap();

    for action in transaction.actions().iter() {
        let pkg_path = store.download(&action.id, Box::new(|cur, max| {
            log::debug!("{}/{} bytes", cur, max);
        }));
    }

    transaction.process(|key, event| {
        log::debug!("{}: {:?}", key.id, event);
    });

    Ok(())
}

fn uninstall(package_names: Vec<&str>, target: WindowsTarget) -> Result<(), CliError> {
    let store = Arc::new(WindowsPackageStore::default());
    let package_actions = package_names
        .into_iter()
        .map(|name| match store.find_package_by_id(&name) {
            Some(pkg) => Ok(PackageAction::uninstall(pkg.0, target)),
            None => Err(CliError::NoPackageFound(name.to_string())),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let transaction = PackageTransaction::new(Arc::clone(&store) as _, package_actions).unwrap();

    transaction.process(|key, event| {
        log::debug!("{}: {:?}", key.id, event);
    });

    Ok(())
}

fn info() {
    let store = WindowsPackageStore::default();
}

fn search() {
    let store = WindowsPackageStore::default();

    let repos = store.repos();
    let repo_map = repos.read().unwrap();

    repo_map.iter().for_each(|(record, repo)| {
        log::debug!("# {}\n---", repo.meta().name.get("en").as_str().unwrap_or_default());

        repo.packages()
            .values()
            .for_each(|pkg| {
                log::debug!("{} {}\n  {}",
                    pkg.id,
                    pkg.version,
                    pkg.name.get("en").as_str().unwrap_or_default()
                );

                let description = pkg.description.get("en")
                    .map(|x| x.as_str())
                    .unwrap_or("");
                if description != "" {
                    log::debug!("  {}", description);
                }
                log::debug!();
            });
    });
}

fn status() {
    let store = WindowsPackageStore::default();
}

fn download() {
    let store = WindowsPackageStore::default();
}

#[cfg(windows)]
fn main() {
    use pahkat_client::windows::*;

    std::mem::forget(sentry::init(DSN));
    register_panic_handler();

    let mut app = App::new("Páhkat")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .version(crate_version!())
        .author(AUTHOR)
        .about(ABOUT)
        .subcommand(subcommand_init())
        .subcommand(subcommand_install())
        .subcommand(subcommand_uninstall())
        .subcommand(subcommand_info())
        .subcommand(subcommand_search())
        .subcommand(subcommand_status())
        .subcommand(subcommand_download());

    let matches = app.get_matches();

    match matches.subcommand() {
        ("init", Some(matches)) => {
            init();
        }
        ("install", Some(matches)) => {
            let target = if matches.is_present("user") { WindowsTarget::User } else { WindowsTarget::System };
            let package_names = matches.values_of("package-id").unwrap();
            install(package_names.collect(), target);
        }
        ("uninstall", Some(matches)) => {
            let target = if matches.is_present("user") { WindowsTarget::User } else { WindowsTarget::System };
            let package_names = matches.values_of("package-id").unwrap();
            uninstall(package_names.collect(), target);
        }
        ("info", Some(matches)) => {
            info();
        }
        ("search", Some(matches)) => {
            search();
        }
        ("status", Some(matches)) => {
            status();
        }
        ("download", Some(matches)) => {
            download();
        }
        _ => {}
    };
    //     // ("init", Some(matches)) => {
    //     //     let url = matches.value_of("url").expect("url to always exist");
    //     //     let cache_dir = matches.value_of("cache-dir").expect("cache-dir to always exist");

    //     //     windows::init(&url, &cache_dir);
    //     // }
    //     // ("list", Some(matches)) => {
    //     //     let config = StoreConfig::load_or_default();
    //     //     let repo = Repository::from_url(&config.url).unwrap();
    //     //     let mut packages: Vec<&Package> = repo.packages().values().collect();
    //     //     packages.sort_unstable_by(|a, b| a.id.cmp(&b.id));
    //     //     for pkg in packages {
    //     //         log::debug!("{} {} ({}) — {}", pkg.id,
    //     //             pkg.version,
    //     //             pkg.name.get("en").unwrap_or(&"???".to_owned()),
    //     //             pkg.description.get("en").unwrap_or(&"???".to_owned())
    //     //         );
    //     //     }
    //     // },
    //     ("install", Some(matches)) => {
    //         // let package_id = matches.value_of("package-id").expect("package-id to always exist");
    //         // let config = StoreConfig::load_or_default();
    //         // let repo = Repository::from_url(&config.url).expect("repository to load");
    //         // let package = match repo.packages().get(package_id) {
    //         //     Some(v) => v,
    //         //     None => {
    //         //         log::debug!("{}: No package found", &package_id);
    //         //         return;
    //         //     }
    //         // };

    //         // let store = WindowsPackageStore::new(&repo, &config);
    //         // // TODO: the config is responsible for creating this.
    //         // let package_cache = store.download_path(&package);
    //         // log::debug!("{:?}", &package_cache);
    //         // if !package_cache.exists() {
    //         //     fs::create_dir_all(&package_cache).expect("create package cache never fails");
    //         // }

    //         // let status = store.status(&package);
    //         // match status {
    //         //     Ok(PackageStatus::NotInstalled) | Ok(PackageStatus::RequiresUpdate) => {
    //         //         let pkg_path = package.download(&package_cache, Some(|cur, max| {
    //         //             log::debug!("{}/{} bytes", cur, max);
    //         //         })).expect("download never has a severe error");
    //         //         let res = store.install(package).expect("install never fails");

    //         //         // match res {
    //         //         //     Ok(v) => log::debug!("{}: {:?}", &package_id, v),
    //         //         //     Err(e) => log::debug!("{}: error - {:?}", &package_id, e)
    //         //         // };
    //         //     },
    //         //     _ => {
    //         //         log::debug!("Nothing to do for identifier {}", package_id);
    //         //         return;
    //         //     }
    //         // }

    //         let package_ids = matches
    //             .values_of("package-id")
    //             .expect("package-id to always exist");
    //         let is_user = matches.is_present("user-target");

    //         let config = StoreConfig::load_or_default();
    //         let repos = config
    //             .repos()
    //             .iter()
    //             .map(|record| Repository::from_url(&record.url, record.channel.clone()).unwrap())
    //             .collect::<Vec<_>>();

    //         let store = Arc::new(WindowsPackageStore::new(config));
    //         let target = match is_user {
    //             true => InstallTarget::User,
    //             false => InstallTarget::System,
    //         };

    //         let mut keys = vec![];
    //         for id in package_ids.into_iter() {
    //             match store.find_package(id) {
    //                 Some(v) => keys.push(v.0),
    //                 None => {
    //                     log::error!("No package found with id: {}", id);
    //                     return;
    //                 }
    //             }
    //         }
    //         let actions = keys
    //             .into_iter()
    //             .map(|k| PackageAction {
    //                 id: k,
    //                 action: PackageActionType::Install,
    //                 target,
    //             })
    //             .collect::<Vec<_>>();

    //         let mut transaction = match PackageTransaction::new(store.clone(), actions) {
    //             Ok(v) => v,
    //             Err(e) => {
    //                 log::error!("{:?}", e);
    //                 return;
    //             }
    //         };

    //         // Download all of the things
    //         for action in transaction.actions().iter() {
    //             let pb = indicatif::ProgressBar::new(0);
    //             pb.set_style(indicatif::ProgressStyle::default_bar()
    //                         .template("{spinner:.green} {prefix} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
    //                         .progress_chars("#>-"));
    //             pb.set_prefix(&action.id.id);

    //             let progress = move |cur, max| {
    //                 pb.set_length(max);
    //                 pb.set_position(cur);

    //                 if cur >= max {
    //                     pb.finish_and_clear();
    //                 }
    //             };
    //             let _pkg_path = store.clone().download(&action.id, progress).unwrap();
    //         }

    //         transaction.process(|key, event| {
    //             log::debug!("{}: {:?}", key.id, event);
    //         });
    //     }
    //     // ("uninstall", Some(matches)) => {
    //     //     let package_id = matches.value_of("package-id").expect("package-id to always exist");
    //     //     let config = StoreConfig::load_or_default();
    //     //     let repo = Repository::from_url(&config.url).expect("repository to load");
    //     //     let package = match repo.packages().get(package_id) {
    //     //         Some(v) => v,
    //     //         None => {
    //     //             log::debug!("{}: No package found", &package_id);
    //     //             return;
    //     //         }
    //     //     };

    //     //     let store = WindowsPackageStore::new(&repo, &config);
    //     //     let status = store.status(&package);
    //     //     match status {
    //     //         Ok(PackageStatus::UpToDate) | Ok(PackageStatus::RequiresUpdate) => {
    //     //             let res = store.uninstall(package).expect("uninstallation can never fail");

    //     //             // match res {
    //     //             //     Ok(v) => log::debug!("{}: {:?}", &package_id, v),
    //     //             //     Err(e) => log::debug!("{}: error - {:?}", &package_id, e)
    //     //             // };
    //     //         },
    //     //         _ => {
    //     //             log::debug!("Nothing to do for identifier {}", package_id);
    //     //             return;
    //     //         }
    //     //     }

    //     // },
    //     // ("status", Some(matches)) => {
    //     //     let package_id = matches.value_of("package-id").unwrap();
    //     //     let config = StoreConfig::load_or_default();
    //     //     let repo = Repository::from_url(&config.url).unwrap();

    //     //     let package = match repo.packages().get(package_id) {
    //     //         Some(v) => v,
    //     //         None => {
    //     //             log::debug!("{}: No package found", &package_id);
    //     //             return;
    //     //         }
    //     //     };
    //     //     let store = WindowsPackageStore::new(&repo, &config);
    //     //     let status = store.status(&package);

    //     //     match status {
    //     //         Ok(v) => log::debug!("{}: {}", &package_id, v),
    //     //         Err(e) => log::debug!("{}: {}", &package_id, e)
    //     //     };
    //     // },
    //     _ => {}
    // }
}
