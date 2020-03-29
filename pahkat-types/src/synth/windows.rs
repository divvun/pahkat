use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
pub struct RegistryKey {
    #[serde(rename = "type")]
    #[builder(default = "WindowsRegistryKey".into())]
    _type: String,

    pub path: String,
    pub name: String,
}
