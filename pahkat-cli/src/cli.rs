pub(crate) mod command;
mod constants;

use std::path::{Path, PathBuf};
use structopt::clap::AppSettings::*;
use structopt::StructOpt;
pub(crate) trait ConfigPath {
    fn config_path(&self) -> Option<&Path>;
}

pub(crate) trait Platform {
    fn platform(&self) -> Option<&str>;
}

use constants::*;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "pahkat",
    bin_name = "pahkat",
    about = "The last package manager.",
    global_settings(&[UnifiedHelpMessage, DisableHelpSubcommand]),
    template(MAIN_TEMPLATE),
    long_version(VERSION),
    version_message("Prints version and license information"),
    usage("pahkat <command>")
)]
pub(crate) enum Args {
    #[structopt(template(SUB_TEMPLATE))]
    Init(command::Init),
    #[structopt(template(SUB_TEMPLATE))]
    Download(command::Download),
    #[structopt(template(SUB_TEMPLATE))]
    Install(command::Install),
    #[structopt(template(SUB_TEMPLATE))]
    Uninstall(command::Uninstall),
    #[structopt(template(SUB_TEMPLATE))]
    Status(command::Status),
    #[structopt(template(SUBC_TEMPLATE))]
    Config(command::Config),
}

impl ConfigPath for Args {
    #[inline]
    fn config_path(&self) -> Option<&Path> {
        match self {
            Args::Init(x) => x.config_path(),
            Args::Download(x) => x.config_path(),
            Args::Install(x) => x.config_path(),
            Args::Uninstall(x) => x.config_path(),
            Args::Config(x) => x.config_path(),
            Args::Status(x) => x.config_path(),
        }
    }
}

impl Platform for Args {
    #[inline]
    fn platform(&self) -> Option<&str> {
        match self {
            Args::Init(x) => x.platform(),
            Args::Download(x) => x.platform(),
            Args::Install(x) => x.platform(),
            Args::Uninstall(x) => x.platform(),
            Args::Status(x) => x.platform(),
            Args::Config(x) => None,
        }
    }
}

#[derive(Debug, StructOpt)]
struct GlobalOpts {
    #[structopt(
        short,
        long,
        parse(from_os_str),
        help = "Path to configuration directory [default: TODO]"
    )]
    config_path: Option<PathBuf>,

    #[cfg_attr(
        windows,
        structopt(short = "P", long, help = "Target platform [default: windows]")
    )]
    #[cfg_attr(
        target_os = "macos",
        structopt(short = "P", long, help = "Target platform [default: macos]")
    )]
    #[cfg_attr(
        target_os = "linux",
        structopt(short = "P", long, help = "Target platform [default: linux]")
    )]
    #[cfg_attr(
        not(any(target_os = "linux", target_os = "macos", windows)),
        structopt(short = "P", long, help = "Target platform")
    )]
    platform: Option<String>,

    #[structopt(short = "C", long, help = "Target channel [default: none]")]
    channel: Option<String>,
}
