use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};
use std::convert::TryFrom;
use url::Url;

use pahkat_types::Repository as RepositoryMeta;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageKey {
    pub url: Url,
    pub id: String,
    pub channel: String,
}

impl fmt::Display for PackageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

impl PackageKey {
    pub fn new(repo: &RepositoryMeta, channel: &str, package_id: &str) -> PackageKey {
        PackageKey {
            url: Url::parse(&repo.base).expect("repo base url must be valid"),
            id: package_id.to_string(),
            channel: channel.to_string(),
        }
    }

    #[inline]
    pub fn to_string(&self) -> String {
        String::from(self)
    }

    #[inline]
    pub fn from_string(url: &str) -> Result<PackageKey, TryFromStringError> {
        PackageKey::try_from(url)
    }
}

impl<'a> From<&'a PackageKey> for String {
    fn from(key: &'a PackageKey) -> String {
        format!("{}packages/{}#{}", key.url, key.id, key.channel)
    }
}

#[derive(Debug, Clone)]
pub enum TryFromStringError {
    InvalidUrl,
}

impl std::error::Error for TryFromStringError {}

impl std::fmt::Display for TryFromStringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid URL")
    }
}

impl<'a> TryFrom<&'a str> for PackageKey {
    type Error = TryFromStringError;

    fn try_from(url: &'a str) -> Result<PackageKey, Self::Error> {
        let url = Url::parse(url).map_err(|_| TryFromStringError::InvalidUrl)?;

        let channel = url
            .fragment()
            .ok_or(TryFromStringError::InvalidUrl)?
            .to_string();
        let base = url.join("..").map_err(|_| TryFromStringError::InvalidUrl)?;
        let id = url
            .path_segments()
            .ok_or(TryFromStringError::InvalidUrl)?
            .last()
            .ok_or(TryFromStringError::InvalidUrl)?
            .to_string();

        Ok(PackageKey {
            url: base,
            channel,
            id,
        })
    }
}

impl Serialize for PackageKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&*self.to_string())
    }
}

impl<'de> Deserialize<'de> for PackageKey {
    fn deserialize<D>(deserializer: D) -> Result<PackageKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(PackageKeyVisitor)
    }
}

struct PackageKeyVisitor;

impl<'de> Visitor<'de> for PackageKeyVisitor {
    type Value = PackageKey;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an PackageKey as a URL string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        PackageKey::from_string(value).map_err(|_| E::custom("Invalid value"))
    }
}
