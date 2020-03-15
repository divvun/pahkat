use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    #[builder(default = "TarballPackage".into())]
    _type: String,

    pub url: url::Url,
    pub size: usize,
    pub installed_size: usize,
}

impl super::AsDownloadUrl for Package {
    fn as_download_url(&self) -> &url::Url {
        &self.url
    }
}
