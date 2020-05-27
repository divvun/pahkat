use serde_derive::Deserialize;

use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct TomlConfig {
    pub artifacts_dir: PathBuf,
    pub url_prefix: String,
    pub db_path: Option<String>,
}
