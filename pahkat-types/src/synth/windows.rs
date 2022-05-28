use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[cfg_attr(feature = "poem-openapi", derive(poem_openapi::Object))]
#[cfg_attr(feature = "async-graphql", derive(async_graphql::SimpleObject))]
pub struct RegistryKey {
    #[serde(rename = "type")]
    #[builder(default = "WindowsRegistryKey".into())]
    #[cfg_attr(feature = "poem-openapi", oai(rename = "type"))]
    #[cfg_attr(feature = "async-graphql", graphql(name = "type"))]
    _type: String,

    pub path: String,
    pub name: String,
}
