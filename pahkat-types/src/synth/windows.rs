use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
pub struct RegistryKey {
    #[serde(rename = "type")]
    #[builder(default = "WindowsRegistryKey".into())]
    #[cfg_attr(feature = "poem-openapi", oai(rename = "type"))]
    _type: String,

    pub path: String,
    pub name: String,
}
