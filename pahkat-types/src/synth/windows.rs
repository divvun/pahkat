use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct RegistryKey {
    #[builder(default = "WindowsRegistryKey".into())]
    _type: String,

    pub path: String,
    pub name: String,
}
