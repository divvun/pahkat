use url::Url;
use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};

use std::fmt;
use pahkat_types::Repository as RepositoryMeta;

impl AbsolutePackageKey {
    pub fn new(repo: &RepositoryMeta, channel: &str, package_id: &str) -> AbsolutePackageKey {
        AbsolutePackageKey {
            url: Url::parse(&repo.base).expect("repo base url must be valid"),
            id: package_id.to_string(),
            channel: channel.to_string(),
        }
    }

    // TODO impl From trait.
    pub fn to_string(&self) -> String {
        format!("{}packages/{}#{}", self.url, self.id, self.channel)
    }

    pub fn from_string(url: &str) -> Result<AbsolutePackageKey, Box<dyn std::error::Error>> {
        let url = Url::parse(url)?;

        let channel = url.fragment().unwrap().to_string();
        let base = url.join("..")?;
        let id = url.path_segments().unwrap().last().unwrap().to_string();

        Ok(AbsolutePackageKey {
            url: base,
            channel,
            id,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AbsolutePackageKey {
    pub url: Url,
    pub id: String,
    pub channel: String,
}

impl Serialize for AbsolutePackageKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&*self.to_string())
    }
}

impl<'de> Deserialize<'de> for AbsolutePackageKey {
    fn deserialize<D>(deserializer: D) -> Result<AbsolutePackageKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(AbsolutePackageKeyVisitor)
    }
}

struct AbsolutePackageKeyVisitor;

impl<'de> Visitor<'de> for AbsolutePackageKeyVisitor {
    type Value = AbsolutePackageKey;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an AbsolutePackageKey as a URL string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        AbsolutePackageKey::from_string(value).map_err(|_| E::custom("Invalid value"))
    }
}
