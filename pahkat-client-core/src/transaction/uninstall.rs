use super::install::ProcessError;

#[derive(thiserror::Error, Debug)]
pub enum UninstallError {
    #[error("Payload error")]
    Payload(#[from] crate::repo::PayloadError),

    #[error("Wrong payload type")]
    WrongPayloadType,

    #[error("Package not found in cache (not downloaded?)")]
    PackageNotInCache,

    #[error("Installation process failed")]
    UninstallerFailure(#[from] ProcessError),

    #[error("The package is not installed")]
    NotInstalled,
}
