use snafu::Snafu;
use std::{io, process};

#[derive(Snafu, Debug)]
#[snafu(visibility = "pub(crate)")]
pub enum InstallError {
    NoPackage,
    NoInstaller,
    WrongInstallerType,
    InvalidFileType,
    PackageNotInCache,
    InvalidUrl {
        source: url::ParseError,
        url: String,
    },
    InstallerFailure {
        source: ProcessError,
    },
}

#[derive(Snafu, Debug)]
pub enum ProcessError {
    Io { source: io::Error },
    Unknown { output: process::Output },
    NotFound,
}
