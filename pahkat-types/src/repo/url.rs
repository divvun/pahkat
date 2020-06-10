use std::{fmt, str::FromStr};

use ::url::Url;
use serde::{de, de::Visitor, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct RepoUrl(Url);

impl std::fmt::Display for RepoUrl {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::ops::Deref for RepoUrl {
    type Target = Url;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum RepoUrlError {
    #[error("Repositories must be `https`. Got: {0}")]
    InvalidScheme(String),

    #[error("URL has no path segments. (Likely an invalid URL)")]
    NoPathSegments,

    #[error("URL is invalid.")]
    InvalidURL(#[from] url::ParseError),
}

impl RepoUrl {
    pub fn new(mut url: Url) -> Result<RepoUrl, RepoUrlError> {
        if url.scheme() != "https" {
            return Err(RepoUrlError::InvalidScheme(url.scheme().to_string()));
        }

        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| RepoUrlError::NoPathSegments)?;
            segments.pop_if_empty().push("");
        }

        Ok(RepoUrl(url))
    }

    pub fn into_inner(self) -> Url {
        self.0
    }
}

impl FromStr for RepoUrl {
    type Err = RepoUrlError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(s)?;
        RepoUrl::new(url)
    }
}

impl Serialize for RepoUrl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&*self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for RepoUrl {
    fn deserialize<D>(deserializer: D) -> Result<RepoUrl, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(RepoUrlVisitor)
    }
}

struct RepoUrlVisitor;

impl<'de> Visitor<'de> for RepoUrlVisitor {
    type Value = RepoUrl;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a RepoUrl as a URL string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let url = Url::parse(value).map_err(|_| E::custom("Invalid URL"))?;
        RepoUrl::new(url).map_err(|_| E::custom("Invalid URL"))
    }
}
