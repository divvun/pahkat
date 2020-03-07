use std::fmt;
use std::path::{Path, PathBuf};

use once_cell::sync::{Lazy, OnceCell};
use serde::de::{self, Deserializer, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

#[cfg(target_os = "android")]
pub(crate) static CONTAINER_PATH: OnceCell<PathBuf> = OnceCell::new();

#[cfg(not(target_os = "android"))]
pub(crate) static CONTAINER_PATH: Lazy<OnceCell<PathBuf>> = Lazy::new(|| {
    let c = OnceCell::new();
    if let Some(v) = dirs::home_dir() {
        let _ = c.set(v);
    }
    c
});

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum ConfigPath {
    Container(Url),
    File(Url),
}

#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("Invalid scheme: {0}")]
    InvalidScheme(String),
    #[error("Invalid URL")]
    InvalidUrl,
}

impl ConfigPath {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<ConfigPath, Error> {
        Url::from_file_path(path)
            .map(|url| ConfigPath::File(url))
            .map_err(|_| Error::InvalidUrl)
    }

    pub fn from_url(url: Url) -> Result<ConfigPath, Error> {
        match url.scheme() {
            "file" => Ok(ConfigPath::File(url)),
            "container" => Ok(ConfigPath::Container(url)),
            scheme => Err(Error::InvalidScheme(scheme.to_string())),
        }
    }

    pub fn join<S: AsRef<str>>(&self, item: S) -> ConfigPath {
        let mut url = self.as_url().to_owned();
        log::debug!("{:?}", url);
        url.path_segments_mut().unwrap().push(item.as_ref());
        log::debug!("{:?}", url);
        ConfigPath::from_url(url).unwrap()
    }

    fn container_to_file(&self) -> Option<Url> {
        log::debug!("container_to_file: {:?}", self);
        let url = match self {
            ConfigPath::File(v) => return Some(v.to_owned()),
            ConfigPath::Container(v) => v,
        };

        let container_path = match CONTAINER_PATH.get() {
            Some(v) => v.join(
                url.path_segments()
                    .map(|x| x.collect::<Vec<_>>().join("/"))
                    .unwrap_or("".into()),
            ),
            None => return None,
        };

        let url = Url::from_file_path(container_path);

        log::debug!("url: {:?}", &url);

        url.ok()
    }

    pub fn to_path_buf(&self) -> Option<PathBuf> {
        log::debug!("to_path_buf");
        let url = match self {
            ConfigPath::File(ref v) => v.to_owned(),
            ConfigPath::Container(_v) => self.container_to_file()?,
        };

        log::debug!("Path: {:?}", &url);

        url.to_file_path().ok()
    }

    pub fn as_url(&self) -> &Url {
        match self {
            ConfigPath::File(url) => url,
            ConfigPath::Container(url) => url,
        }
    }
}

impl Serialize for ConfigPath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&*self.as_url().to_string())
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

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a ConfigPath as a URL string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value.starts_with("file:") || value.starts_with("container:") {
            let url = url::Url::parse(value).map_err(|_| E::custom("Invalid URL"))?;
            ConfigPath::from_url(url).map_err(|_| E::custom("Invalid URL scheme"))
        } else {
            ConfigPath::from_path(value).map_err(|_| E::custom("Invalid file path"))
        }
    }
}
