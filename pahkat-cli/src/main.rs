mod cli;
mod download;
mod install;
mod status;
mod uninstall;
mod config;

use anyhow::{Context, Result};
use cli::{Args, Platform, ConfigPath};
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[inline]
#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
fn config_path(holder: &dyn ConfigPath) -> Result<PathBuf> {
    holder
        .config_path()
        .map(Path::to_owned)
        .or_else(|| directories::BaseDirs::new().map(|x| x.config_dir().join("Pahkat")))
        .with_context(|| "No default config path could be found")
}

use pahkat_client::{Config, PackageStore};
use std::sync::Arc;

// #[inline(always)]
// #[cfg(feature = "windows")]
// fn store(args: &Args) -> anyhow::Result<Arc<dyn PackageStore<Target=>>> {
//     let config_path = config_path(&args)?;
//     let config = pahkat_client::Config::load(&config_path, pahkat_client::Permission::ReadWrite)?;

//     Arc::new(pahkat_client::WindowsPackageStore::new(config))
// }


#[inline(always)]
#[cfg(feature = "prefix")]
async fn store(config_path: Option<&Path>) -> anyhow::Result<Arc<dyn PackageStore>> {
    let config_path = config_path.ok_or_else(|| anyhow::anyhow!("No prefix path specified"))?;
    let store = pahkat_client::PrefixPackageStore::open(config_path).await?;
    let store = Arc::new(store);

    if store.config().read().unwrap().repos().len() == 0 {
        println!("WARNING: There are no repositories in the given config.");
    }

    Ok(store)
}#

[inline(always)]
#[cfg(feature = "prefix")]
async fn create_store(config_path: Option<&Path>) -> anyhow::Result<Arc<dyn PackageStore>> {
    let config_path = config_path.ok_or_else(|| anyhow::anyhow!("No prefix path specified"))?;
    let store = pahkat_client::PrefixPackageStore::create(config_path).await?;
    let store = Arc::new(store);

    if store.config().read().unwrap().repos().len() == 0 {
        println!("WARNING: There are no repositories in the given config.");
    }

    Ok(store)
}

#[inline(always)]
#[cfg(feature = "macos")]
async fn store(config_path: Option<&Path>) -> anyhow::Result<Arc<dyn PackageStore>> {
    let config = match config_path {
        Some(v) => pahkat_client::Config::load(&v, pahkat_client::Permission::ReadWrite)?,
        None => pahkat_client::Config::load_default()?,
    };
    let store = pahkat_client::MacOSPackageStore::new(config).await;
    let store = Arc::new(store);

    if store.config().read().unwrap().repos().len() == 0 {
        println!("WARNING: There are no repositories in the given config.");
    }

    Ok(store)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = Args::from_args();
    println!("{:?}", &args);

    // match args
    match &args {
        #[cfg(feature = "prefix")]
        cli::Args::Init(a) => {
            create_store(args.config_path()).await?;
        }
        cli::Args::Download(a) => {
            let store = store(args.config_path()).await?;
            download::download(
                store,
                &a.packages,
                &a.output_path
                    .as_ref()
                    .map(|x| x.clone())
                    .unwrap_or_else(|| std::env::current_dir().unwrap()),
            ).await?
        }
        cli::Args::Status(a) => {
            let store = store(args.config_path()).await?;
            status::status(&*store, &a.packages, Default::default())?
        }
        cli::Args::Uninstall(a) => {
            let store = store(args.config_path()).await?;
            uninstall::uninstall(&*store, &a.packages, Default::default())?
        }
        cli::Args::Install(a) => {
            let store = store(args.config_path()).await?;
            install::install(store, &a.packages, Default::default(), &args).await?
        }
        cli::Args::Config(a) => {
            let store = store(args.config_path()).await?;
            config::config(store, a, Default::default(), &args).await?
        }
    }

    Ok(())
}
