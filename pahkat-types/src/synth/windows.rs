use derive_builder::Builder;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct RegistryKey {
    #[serde(rename = "_type")]
    _type: String,

    pub path: String,
    pub name: String,
}
