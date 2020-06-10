use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(transparent)]
#[repr(transparent)]
struct PayloadType(String);

impl Default for PayloadType {
    fn default() -> Self {
        PayloadType("TarballPackage".into())
    }
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[cfg_attr(feature = "structopt", derive(structopt::StructOpt))]
pub struct Package {
    #[builder(default, setter(skip))]
    #[serde(rename = "type")]
    #[cfg_attr(feature = "structopt", structopt(skip))]
    _type: PayloadType,

    #[cfg_attr(feature = "structopt", structopt(short, long))]
    pub url: url::Url,

    #[cfg_attr(feature = "structopt", structopt(short, long))]
    pub size: u64,

    #[cfg_attr(feature = "structopt", structopt(short, long))]
    pub installed_size: u64,
}

impl super::AsDownloadUrl for Package {
    fn as_download_url(&self) -> &url::Url {
        &self.url
    }
}
