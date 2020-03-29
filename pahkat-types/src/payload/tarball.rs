use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
pub struct Package {
    #[builder(default = "TarballPackage".into())]
    #[serde(rename = "type")]
    _type: String,

    pub url: url::Url,
    pub size: u64,
    pub installed_size: u64,
}

impl super::AsDownloadUrl for Package {
    fn as_download_url(&self) -> &url::Url {
        &self.url
    }
}
