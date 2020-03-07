use crate::transaction::{PackageStatus, PackageStatusError};

use pahkat_types::package::Version;

pub(crate) fn cmp(
    installed_version: &str,
    candidate_version: &Version,
) -> Result<PackageStatus, PackageStatusError> {
    let installed_version = match Version::new(installed_version) {
        Ok(v) => v,
        Err(_) => return Err(PackageStatusError::ParsingVersion),
    };

    if candidate_version > &installed_version {
        Ok(PackageStatus::RequiresUpdate)
    } else {
        Ok(PackageStatus::UpToDate)
    }
}
