use chrono::prelude::*;

use crate::{PackageStatus, PackageStatusError};

pub(crate) fn semver_cmp(
    installed_version: &str,
    candidate_version: &str,
    skipped_version: Option<&str>,
) -> Result<PackageStatus, PackageStatusError> {
    let installed_version = match semver::Version::parse(installed_version) {
        Err(_) => return Err(PackageStatusError::ParsingVersion),
        Ok(v) => v,
    };

    let candidate_version = match semver::Version::parse(candidate_version) {
        Err(_) => return Err(PackageStatusError::ParsingVersion),
        Ok(v) => v,
    };

    if let Some(skipped_version) = skipped_version {
        match semver::Version::parse(&skipped_version) {
            Err(_) => {} // No point giving up now
            Ok(v) => {
                if candidate_version <= v {
                    return Ok(PackageStatus::Skipped);
                }
            }
        }
    }

    if candidate_version > installed_version {
        Ok(PackageStatus::RequiresUpdate)
    } else {
        Ok(PackageStatus::UpToDate)
    }
}

pub(crate) fn iso8601_cmp(
    installed_version: &str,
    candidate_version: &str,
    skipped_version: Option<&str>,
) -> Result<PackageStatus, PackageStatusError> {
    let installed_version = match installed_version.parse::<DateTime<Utc>>() {
        Ok(v) => v,
        Err(_) => return Err(PackageStatusError::ParsingVersion),
    };

    let candidate_version = match candidate_version.parse::<DateTime<Utc>>() {
        Ok(v) => v,
        Err(_) => return Err(PackageStatusError::ParsingVersion),
    };

    if let Some(skipped_version) = skipped_version {
        match skipped_version.parse::<DateTime<Utc>>() {
            Err(_) => {} // No point giving up now
            Ok(v) => {
                if candidate_version <= v {
                    return Ok(PackageStatus::Skipped);
                }
            }
        }
    }

    if candidate_version > installed_version {
        Ok(PackageStatus::RequiresUpdate)
    } else {
        Ok(PackageStatus::UpToDate)
    }
}
