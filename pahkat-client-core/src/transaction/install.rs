use std::{io, process};

#[derive(thiserror::Error, Debug)]
pub enum InstallError {
    #[error("Payload error")]
    Payload(#[from] crate::repo::PayloadError),

    #[error("Wrong payload type")]
    WrongPayloadType,

    #[error("Package not found in cache (not downloaded?)")]
    PackageNotInCache,

    #[error("Installation process failed")]
    InstallerFailure(#[from] ProcessError),
}

#[derive(thiserror::Error, Debug)]
pub enum ProcessError {
    #[error("IO error")]
    Io(#[from] io::Error),

    #[error("Not found")]
    NotFound,

    #[error("Unknown error")]
    Unknown(process::Output),
}
