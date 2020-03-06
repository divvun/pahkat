use crate::transaction::{PackageStatus, PackageStatusError};

use std::cmp::Ordering;
use std::fmt::{self, Display, Formatter};

use chrono::prelude::*;
use semver::{SemVerError, Version as SemVer};

#[derive(Debug)]
pub enum VersionValidationError {
    NotUTCDateError,
    InvalidInput(SemVerError, chrono::ParseError),
}

#[derive(Debug)]
pub enum Version {
    SemVer(SemVer),
    UtcDate(DateTime<Utc>),
}

pub(crate) fn cmp(
    installed_version: &str,
    candidate_version: &str,
    skipped_version: Option<&str>,
) -> Result<PackageStatus, PackageStatusError> {
    let installed_version = match Version::new(installed_version) {
        Ok(v) => v,
        Err(_) => return Err(PackageStatusError::ParsingVersion),
    };

    let candidate_version = match Version::new(candidate_version) {
        Ok(v) => v,
        Err(_) => return Err(PackageStatusError::ParsingVersion),
    };

    if let Some(skipped_version) = skipped_version {
        match Version::new(skipped_version) {
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

impl Version {
    pub fn new(version: &str) -> Result<Self, VersionValidationError> {
        match version.parse::<DateTime<Utc>>() {
            Ok(date) => {
                if version.ends_with('Z') {
                    Ok(Version::UtcDate(date))
                } else {
                    Err(VersionValidationError::NotUTCDateError)
                }
            }
            Err(date_e) => match SemVer::parse(version) {
                Ok(semver) => Ok(Version::SemVer(semver)),
                Err(semver_e) => Err(VersionValidationError::InvalidInput(semver_e, date_e)),
            },
        }
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let str = match self {
            Version::SemVer(semver) => semver.to_string(),
            Version::UtcDate(date) => date.to_rfc3339_opts(SecondsFormat::Millis, true),
        };

        write!(f, "{}", str)
    }
}

impl Eq for Version {}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Version::SemVer(my), Version::SemVer(other)) => my.cmp(other),
            (Version::UtcDate(my), Version::UtcDate(other)) => my.cmp(other),
            (Version::UtcDate(_), Version::SemVer(_)) => Ordering::Greater,
            (Version::SemVer(_), Version::UtcDate(_)) => Ordering::Less,
        }
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equal_dates() {
        let my = Version::new("2019-01-01T01:30:59Z").unwrap();
        let other = Version::new("2019-01-01T01:30:59.00Z").unwrap();

        assert_eq!(my, other);
    }

    #[test]
    fn test_my_lesser_date() {
        let my = Version::new("2018-01-01T01:30:59Z").unwrap();
        let other = Version::new("2019-01-01T01:30:59Z").unwrap();

        assert_eq!(my < other, true);
        assert_eq!(my > other, false);
    }

    #[test]
    fn test_my_greater_date() {
        let my = Version::new("2018-05-01T01:30:59Z").unwrap();
        let other = Version::new("2018-02-01T01:30:59Z").unwrap();

        assert_eq!(my.cmp(&other), Ordering::Greater);
        assert_eq!(my > other, true);
        assert_eq!(my < other, false);
    }

    #[test]
    fn test_my_date_greater_than_semver() {
        let my = Version::new("2018-05-01T01:30:59Z").unwrap();
        let other = Version::new("2.1.0").unwrap();

        assert_eq!(my > other, true);
        assert_eq!(my < other, false);
    }

    #[test]
    fn test_my_semver_less_than_date() {
        let my = Version::new("1111.23.2323").unwrap();
        let other = Version::new("1980-01-01T00:00:00.000Z").unwrap();

        assert_eq!(my < other, true);
        assert_eq!(my > other, false);
    }

    #[test]
    fn test_equal_semver() {
        let my = Version::new("1.2.3").unwrap();
        let other = Version::new("1.2.3").unwrap();

        assert_eq!(my, other);
    }

    #[test]
    fn test_lesser_my_semver() {
        let my = Version::new("0.1.2").unwrap();
        let other = Version::new("34.1.0").unwrap();

        assert_eq!(my.partial_cmp(&other), Some(Ordering::Less));
        assert_ne!(my, other);
    }

    #[test]
    fn test_greater_my_semver() {
        let my = Version::new("5.1.2").unwrap();
        let other = Version::new("3.12.99").unwrap();

        assert_eq!(my.partial_cmp(&other), Some(Ordering::Greater));
        assert_ne!(my, other);
    }
}
