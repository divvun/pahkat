use std::convert::TryFrom;
use std::fmt;
use std::path::{Path, PathBuf};

use once_cell::sync::{Lazy, OnceCell};
use pathos::iri::IriBufExt;
use serde::de::{self, Deserializer, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use thiserror::Error;
use url::Url;

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct ConfigPath(pub(crate) iref::IriBuf);

#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("Invalid scheme: {0}")]
    InvalidScheme(String),
    #[error("Invalid URL")]
    InvalidUrl,
}

impl ConfigPath {
    pub fn join<S: AsRef<str>>(&self, item: S) -> ConfigPath {
        let mut iri = self.0.clone();
        iri.path_mut().push(item.as_ref().try_into().unwrap());
        ConfigPath(iri)
    }

    pub fn to_path_buf(&self) -> Result<PathBuf, pathos::iri::Error> {
        self.0.to_path_buf()
    }
}

impl TryFrom<PathBuf> for ConfigPath {
    type Error = ();

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        use pathos::path::absolute::AbsolutePathBufExt;

        let iri = value
            .to_absolute_path_buf()
            .map_err(|_| ())?
            .to_file_iri()
            .map_err(|_| ())?;
        Ok(ConfigPath(iri))
    }
}

impl Serialize for ConfigPath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&*self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for ConfigPath {
    fn deserialize<D>(deserializer: D) -> Result<ConfigPath, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ConfigPathVisitor)
    }
}

struct ConfigPathVisitor;

impl<'de> Visitor<'de> for ConfigPathVisitor {
    type Value = ConfigPath;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a ConfigPath as a URL string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value.starts_with("file:") || value.starts_with("container:") {
            let url = iref::IriBuf::new(value).map_err(|_| E::custom("Invalid URL"))?;
            if value.starts_with("file:") {
                url.to_path_buf()
                    .map_err(|_| E::custom("File path not absolute"))?;
            }
            Ok(ConfigPath(url))
        } else {
            Err(E::custom("Invalid URL"))
        }
    }
}
