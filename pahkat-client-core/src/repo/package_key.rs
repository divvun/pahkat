use std::convert::TryFrom;
use std::fmt;

use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageKey {
    pub repository_url: Url,
    pub id: String,

    // Query parameters
    pub channel: Option<String>,
    pub platform: Option<String>,
    pub version: Option<String>,
    pub arch: Option<String>,
}

impl fmt::Display for PackageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PackageKeyArgs {
    pub platform: Option<String>,
    pub version: Option<String>,
    pub arch: Option<String>,
}

impl PackageKey {
    #[inline]
    pub fn to_string(&self) -> String {
        String::from(self)
    }

    pub(crate) fn unchecked_new(
        repository_url: Url,
        id: String,
        channel: Option<String>,
        args: Option<PackageKeyArgs>,
    ) -> Self {
        let args = args.unwrap_or_else(|| Default::default());

        PackageKey {
            repository_url,
            id,
            channel,
            platform: args.platform,
            version: args.version,
            arch: args.arch,
        }
    }
}

impl<'a> From<&'a PackageKey> for Url {
    fn from(key: &'a PackageKey) -> Url {
        let mut url = key.repository_url.clone();

        {
            // URL must always be a base, so this is safe (or we really do want to crash.)
            let mut segments = url
                .path_segments_mut()
                .expect("URL was not a base, but must always be");
            segments.pop_if_empty().push("packages").push(&key.id);
        }

        {
            let mut query = url.query_pairs_mut();

            if let Some(ref arch) = key.arch {
                query.append_pair("arch", arch);
            }

            if let Some(ref channel) = key.channel {
                query.append_pair("channel", channel);
            }

            if let Some(ref platform) = key.platform {
                query.append_pair("platform", platform);
            }

            if let Some(ref version) = key.version {
                query.append_pair("version", version);
            }
        }

        url
    }
}

impl<'a> From<&'a PackageKey> for String {
    fn from(key: &'a PackageKey) -> String {
        Url::from(key).to_string()
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum TryFromError {
    #[error("Invalid URL")]
    InvalidUrl,

    #[error("URL must not be a base")]
    BaseForbidden,

    #[error("URL does not contain /packages/ segment")]
    MissingPackagesSegment,

    #[error("Invalid package segment")]
    InvalidPackageSegment,
}

impl<'a> TryFrom<&'a Url> for PackageKey {
    type Error = TryFromError;

    fn try_from(url: &'a Url) -> Result<PackageKey, Self::Error> {
        let query = url.query_pairs();

        let mut url = url.clone();
        url.set_query(None);
        url.set_fragment(None);

        let mut version = None;
        let mut channel = None;
        let mut platform = None;
        let mut arch = None;

        for (k, v) in query {
            match &*k {
                "version" => version = Some(v.to_string()),
                "channel" => channel = Some(v.to_string()),
                "platform" => platform = Some(v.to_string()),
                "arch" => arch = Some(v.to_string()),
                _ => {}
            }
        }

        let (left, id) = {
            // Find first /packages/ segment
            let path_segments = url
                .path_segments()
                .ok_or_else(|| TryFromError::BaseForbidden)?;
            let sides = path_segments.collect::<Vec<_>>();
            let sides = sides.splitn(2, |x| *x == "packages").collect::<Vec<_>>();

            if sides.len() != 2 {
                return Err(TryFromError::MissingPackagesSegment);
            }

            if sides[1].len() != 1 {
                return Err(TryFromError::InvalidPackageSegment);
            }

            let id = sides[1][0].to_string();
            (
                sides[0]
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<String>>(),
                id,
            )
        };

        {
            let mut path_segments = url
                .path_segments_mut()
                .map_err(|_| TryFromError::BaseForbidden)?;
            path_segments.clear();
            path_segments.extend(left);
        }

        Ok(PackageKey {
            repository_url: url,
            id,
            channel,
            version,
            platform,
            arch,
        })
    }
}

impl<'a> TryFrom<&'a str> for PackageKey {
    type Error = TryFromError;

    fn try_from(url: &'a str) -> Result<PackageKey, Self::Error> {
        let url = Url::parse(url).map_err(|_| TryFromError::InvalidUrl)?;
        PackageKey::try_from(&url)
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
        PackageKey::try_from(value).map_err(|e| E::custom(e))
    }
}
