pub mod index;

use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
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

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct Redirect {
    #[builder(default = "RepositoryRedirect".into())]
    _type: String,

    pub redirect: url::Url,
}
