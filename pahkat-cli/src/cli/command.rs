pub(crate) mod config;

use std::path::{Path, PathBuf};
use structopt::StructOpt;

use crate::cli::constants::*;

#[derive(Debug, StructOpt)]
#[structopt(about = "Download packages into a specified directory")]
pub struct Download {
    #[structopt(required = true, help = "Packages to download")]
    pub packages: Vec<String>,

    #[structopt(
        short,
        long = "output",
        help = "Output directory [default: configured cache]",
        parse(from_os_str)
    )]
    pub output_path: Option<PathBuf>,

    #[structopt(flatten)]
    global_opts: super::GlobalOpts,
}

#[derive(Debug, StructOpt)]
#[structopt(about = "Install packages from configured repositories")]
pub struct Install {
    #[structopt(required = true, help = "Packages to install")]
    pub packages: Vec<String>,
    #[structopt(flatten)]
    global_opts: super::GlobalOpts,
}

#[derive(Debug, StructOpt)]
#[structopt(about = "Uninstall previously installed packages")]
pub struct Uninstall {
    #[structopt(required = true, help = "Packages to uninstall")]
    pub packages: Vec<String>,
    #[structopt(flatten)]
    global_opts: super::GlobalOpts,
}

#[derive(Debug, StructOpt)]
#[structopt(about = "Initialize configuration")]
pub struct Init {
    #[structopt(flatten)]
    global_opts: super::GlobalOpts,
}

#[derive(Debug, StructOpt)]
#[structopt(about = "Manage package manager configuration and settings")]
pub enum Config {
    #[structopt(template(SUBC_TEMPLATE))]
    Repo(config::Repo),
}

#[derive(Debug, StructOpt)]
#[structopt(about = "Query status of given packages")]
pub struct Status {
    #[structopt(help = "Packages to query status of")]
    pub packages: Vec<String>,
    #[structopt(flatten)]
    global_opts: super::GlobalOpts,
}

use crate::{ConfigPath, Platform};

impl ConfigPath for Download {
    #[inline]
    fn config_path(&self) -> Option<&Path> {
        self.global_opts.config_path.as_ref().map(PathBuf::as_path)
    }
}

impl Platform for Download {
    #[inline]
    fn platform(&self) -> Option<&str> {
        self.global_opts.platform.as_ref().map(|x| &**x)
    }
}

impl ConfigPath for Install {
    #[inline]
    fn config_path(&self) -> Option<&Path> {
        self.global_opts.config_path.as_ref().map(PathBuf::as_path)
    }
}

impl Platform for Install {
    #[inline]
    fn platform(&self) -> Option<&str> {
        self.global_opts.platform.as_ref().map(|x| &**x)
    }
}

impl ConfigPath for Uninstall {
    #[inline]
    fn config_path(&self) -> Option<&Path> {
        self.global_opts.config_path.as_ref().map(PathBuf::as_path)
    }
}

impl Platform for Uninstall {
    #[inline]
    fn platform(&self) -> Option<&str> {
        self.global_opts.platform.as_ref().map(|x| &**x)
    }
}

impl ConfigPath for Status {
    #[inline]
    fn config_path(&self) -> Option<&Path> {
        self.global_opts.config_path.as_ref().map(PathBuf::as_path)
    }
}

impl Platform for Status {
    #[inline]
    fn platform(&self) -> Option<&str> {
        self.global_opts.platform.as_ref().map(|x| &**x)
    }
}

impl ConfigPath for Init {
    #[inline]
    fn config_path(&self) -> Option<&Path> {
        self.global_opts.config_path.as_ref().map(PathBuf::as_path)
    }
}

impl Platform for Init {
    #[inline]
    fn platform(&self) -> Option<&str> {
        self.global_opts.platform.as_ref().map(|x| &**x)
    }
}

impl ConfigPath for Config {
    #[inline]
    fn config_path(&self) -> Option<&Path> {
        match self {
            Config::Repo(x) => x.config_path(),
        }
    }
}
