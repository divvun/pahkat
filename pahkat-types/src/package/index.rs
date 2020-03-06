use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Package {
    Package(super::Package),
    SyntheticPackage(crate::synth::Package),
    PackageRedirect(super::Redirect),
}

#[derive(Debug, Serialize, Deserialize, Clone, Builder)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    #[serde(default = "BTreeMap::new")]
    pub packages: BTreeMap<String, Package>,
}
