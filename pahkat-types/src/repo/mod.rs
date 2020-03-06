pub mod index;

use derive_builder::Builder;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Repository {
    Index(index::Index),
    Redirect(Redirect),
}

/// This struct represents the strings for localising tags and is found in the
/// `strings/` directory at the base of a Pahkat repository.
///
/// The TOML file this struct represents is named after the prefix of the given tag,
/// such that a tag of `category:keyboards` would look up `strings/category.toml`.
pub struct Localisation {
    // TODO
}

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
pub struct Redirect {
    #[serde(rename = "_type")]
    pub _type: String,

    pub redirect: String,
}
