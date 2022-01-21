use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[cfg_attr(feature = "structopt", derive(structopt::StructOpt))]
#[cfg_attr(feature = "poem-openapi", derive(poem_openapi::Object))]
#[cfg_attr(feature = "poem-openapi", oai(rename = "TarballPackage"))]
pub struct Package {
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
