use super::install::ProcessError;
use snafu::Snafu;

#[derive(Snafu, Debug)]
pub enum UninstallError {
    NoPackage,
    NoInstaller,
    NotInstalled,
    WrongInstallerType,
    ProcessFailed { source: ProcessError },
    PlatformFailure { message: &'static str },
}
