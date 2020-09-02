use std::cmp::Ordering;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd, Ord, Eq, Hash)]
#[repr(transparent)]
pub struct SemanticVersion(semver::Version);

impl FromStr for SemanticVersion {
    type Err = semver::SemVerError;

    #[inline(always)]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        semver::Version::from_str(s).map(SemanticVersion)
    }
}

impl SemanticVersion {
    #[inline]
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl std::ops::Deref for SemanticVersion {
    type Target = semver::Version;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, Eq)]
#[serde(untagged)]
#[non_exhaustive]
pub enum Version {
    Semantic(SemanticVersion),
}

#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("Unhandled input: {0}")]
    UnhandledInput(String),
}

impl Version {
    pub fn new(version: &str) -> Result<Self, Error> {
        match version.parse::<SemanticVersion>() {
            Ok(v) => return Ok(Version::Semantic(v)),
            Err(_) => { /* fall through */ }
        }

        Err(Error::UnhandledInput(version.to_string()))
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Version::Semantic(semver) => semver.0.fmt(f),
        }
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        if let Some(v) = self.partial_cmp(other) {
            return v == Ordering::Equal;
        }
        false
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Version::Semantic(my), Version::Semantic(other)) => Some(my.cmp(other)),
        }
    }
}

impl FromStr for Version {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Version::new(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equal_dates() {
        let my = Version::new("1.0.0-nightly.20190101T013059Z").unwrap();
        let other = Version::new("1.0.0-nightly.20190101T013059Z").unwrap();

        assert_eq!(my, other);
    }

    #[test]
    fn test_my_lesser_date() {
        let my = Version::new("1.0.0-nightly.20180101T013059Z").unwrap();
        let other = Version::new("1.0.0-nightly.20190101T013059Z").unwrap();

        assert_eq!(my < other, true);
        assert_eq!(my > other, false);
        assert_eq!(my == other, false);
    }

    #[test]
    fn test_my_greater_date() {
        let my = Version::new("1.0.0-nightly.20180501T013059Z").unwrap();
        let other = Version::new("1.0.0-nightly.20180201T013059Z").unwrap();

        assert_eq!(my.partial_cmp(&other), Some(Ordering::Greater));
        assert_eq!(my > other, true);
        assert_eq!(my < other, false);
        assert_eq!(my == other, false);
    }

    #[test]
    fn test_nightly_weaker_than_base_version() {
        let my = Version::new("1.0.0-nightly.20180501T013059Z").unwrap();
        let other = Version::new("1.0.0").unwrap();

        assert_eq!(my.partial_cmp(&other), Some(Ordering::Less));
        assert_eq!(my < other, true);
        assert_eq!(my > other, false);
        assert_eq!(my == other, false);
    }

    #[test]
    fn test_nightly_stronger_than_smaller_base_version() {
        let my = Version::new("1.0.1-nightly.20180501T013059Z").unwrap();
        let other = Version::new("1.0.0").unwrap();

        assert_eq!(my.partial_cmp(&other), Some(Ordering::Greater));
        assert_eq!(my < other, false);
        assert_eq!(my > other, true);
        assert_eq!(my == other, false);
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
