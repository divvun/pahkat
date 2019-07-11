use super::install::ProcessError;
use snafu::Snafu;

#[derive(Snafu, Debug)]
pub enum UninstallError {
    NoPackage,
    NoInstaller,
    WrongInstallerType,
    ProcessFailed { source: ProcessError },
}
