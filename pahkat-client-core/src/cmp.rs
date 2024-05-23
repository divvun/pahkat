use crate::transaction::{PackageStatus, PackageStatusError};

use pahkat_types::package::Version;

pub(crate) fn cmp(
    installed_version: &str,
    candidate_version: &Version,
) -> Result<PackageStatus, PackageStatusError> {
    let installed_version = match Version::new(installed_version) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("Can't parse version {}, {:?}", installed_version, e);
            return Err(PackageStatusError::ParsingVersion);
        }
    };

    if candidate_version > &installed_version {
        return Ok(PackageStatus::RequiresUpdate);
    }

    Ok(PackageStatus::UpToDate)
}

#[test]
fn compare_versions() {
    assert_eq!(
        cmp("1.0.0", &Version::new("1.0.1").unwrap()).unwrap(),
        PackageStatus::RequiresUpdate
    );
    assert_eq!(
        cmp(
            "1.0.0",
            &Version::new("1.0.0-nightly.20240516T103300123Z").unwrap()
        )
        .unwrap(),
        PackageStatus::UpToDate
    );
    assert_eq!(
        cmp(
            "1.0.0-nightly.20240516T103300123Z",
            &Version::new("1.0.0").unwrap()
        )
        .unwrap(),
        PackageStatus::RequiresUpdate
    );
    assert_eq!(
        cmp(
            "1.0.0",
            &Version::new("1.0.1-nightly.20240516T103300123Z").unwrap()
        )
        .unwrap(),
        PackageStatus::RequiresUpdate
    );
    assert_eq!(
        cmp(
            "1.0.1-nightly.20240516T103300123Z",
            &Version::new("1.0.1-nightly.20241231T103300123Z").unwrap()
        )
        .unwrap(),
        PackageStatus::RequiresUpdate
    );
}
