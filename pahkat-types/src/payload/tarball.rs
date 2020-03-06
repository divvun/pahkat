use derive_builder::Builder;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    #[serde(rename = "_type")]
    /// Always has value `TarballPackage`.
    pub _type: String,
    pub url: url::Url,
    pub size: usize,
    pub installed_size: usize,
}

impl super::AsDownloadUrl for Package {
    fn as_download_url(&self) -> &url::Url {
        &self.url
    }
}
