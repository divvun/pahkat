use std::cmp::Ordering;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum TimestampError {
    #[error("Must be a UTC timestamp (ending with a Z)")]
    NonUTC,
    #[error("Invalid date format")]
    InvalidDate(#[from] chrono::ParseError),
}

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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd, Ord, Eq, Hash)]
#[repr(transparent)]
pub struct TimestampVersion(DateTime<Utc>);

impl FromStr for TimestampVersion {
    type Err = TimestampError;

    #[inline(always)]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match chrono::DateTime::from_str(s) {
            Ok(v) if s.ends_with('Z') => Ok(v),
            Ok(_) => Err(TimestampError::NonUTC),
            Err(e) => Err(TimestampError::InvalidDate(e)),
        }
        .map(TimestampVersion)
    }
}

impl TimestampVersion {
    #[inline]
    pub fn to_string(&self) -> String {
        self.0.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, Eq)]
#[serde(untagged)]
#[non_exhaustive]
pub enum Version {
    Semantic(SemanticVersion),
    Timestamp(TimestampVersion),
}

#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("Error parsing timestamp version")]
    Timestamp(#[from] TimestampError),
    #[error("Unhandled input: {0}")]
    UnhandledInput(String),
}

impl Version {
    pub fn new(version: &str) -> Result<Self, Error> {
        match version.parse::<TimestampVersion>() {
            Ok(v) => return Ok(Version::Timestamp(v)),
            Err(TimestampError::NonUTC) => return Err(Error::Timestamp(TimestampError::NonUTC)),
            Err(_) => { /* fall through */ }
        }

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
            Version::Timestamp(date) => {
                f.write_str(&date.0.to_rfc3339_opts(SecondsFormat::Millis, true))
            }
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
            (Version::Timestamp(my), Version::Timestamp(other)) => Some(my.cmp(other)),
            (Version::Timestamp(_), Version::Semantic(_)) => Some(Ordering::Greater),
            (Version::Semantic(_), Version::Timestamp(_)) => Some(Ordering::Less),
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

        assert_eq!(my.partial_cmp(&other), Some(Ordering::Greater));
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
