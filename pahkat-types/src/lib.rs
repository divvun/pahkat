pub mod package;
pub mod payload;
pub mod repo;
pub mod synth;

/// Will be replaced with a validating Map in the future.
///
/// Keys must be valid BCP 47 language tags.
pub type LangTagMap<T> = std::collections::BTreeMap<String, T>;

/// Will be replaced with a validating Map in the future.
pub type DependencyMap = std::collections::BTreeMap<String, String>;

pub use payload::AsDownloadUrl;
