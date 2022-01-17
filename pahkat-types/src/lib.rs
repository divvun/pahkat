pub mod package;
pub mod package_key;
pub mod payload;
pub mod repo;
pub mod synth;

use serde::{Deserialize, Serialize};
use std::{str::FromStr, fmt::Display, convert::Infallible};
use url::Url;

/// Will be replaced with a validating Map in the future.
///
/// Keys must be valid BCP 47 language tags.
pub type LangTagMap<T> = std::collections::BTreeMap<String, T>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(untagged)]
#[cfg_attr(feature = "poem-openapi", derive(poem_openapi::Union))]
#[cfg_attr(feature = "poem-openapi", oai(discriminator_name = "type"))]
pub enum DependencyKey {
    Remote(Url),
    Local(String),
}

impl DependencyKey {
    pub fn as_str(&self) -> &str {
        match self {
            DependencyKey::Remote(x) => x.as_str(),
            DependencyKey::Local(x) => &*x,
        }
    }
}

impl Display for DependencyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(f)
    }
}



impl FromStr for DependencyKey {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.into())
    }
}

impl From<String> for DependencyKey {
    fn from(s: String) -> Self {
        if let Ok(value) = s.parse::<Url>() {
            DependencyKey::Remote(value)
        } else {
            DependencyKey::Local(s.to_string())
        }
    }
}

impl From<&str> for DependencyKey {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

/// Will be replaced with a validating Map in the future.
pub type DependencyMap = std::collections::BTreeMap<DependencyKey, String>;

pub use package_key::PackageKey;
pub use payload::AsDownloadUrl;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        let mut names = LangTagMap::new();
        names.insert("en".to_string(), "Test Package".to_string());
        let mut desc = LangTagMap::new();
        desc.insert("en".to_string(), "A test package for testing.".to_string());

        let mut deps = DependencyMap::new();
        deps.insert(
            DependencyKey::Local("some-dependency".to_string()),
            "*".to_string(),
        );

        let package1 = package::Descriptor::builder()
            .package(
                package::DescriptorData::builder()
                    .id("test-package".to_string())
                    .tags(vec!["category:test".to_string(), "language:en".to_string()])
                    .build(),
            )
            .name(names)
            .description(desc)
            .release(vec![
                package::Release::builder()
                    .version(package::Version::new("1.3.0").unwrap())
                    .channel(Some("test".to_string()))
                    .authors(vec!["Test Person <test@example.com>".into()])
                    .license(Some("CC-1.0".to_string()))
                    .target(vec![
                        payload::Target::builder()
                            .platform("windows".to_string())
                            .arch(Some("x86_64".into()))
                            .dependencies(deps.clone())
                            .payload(payload::Payload::WindowsExecutable(
                                payload::windows::Executable::builder()
                                    .url(url::Url::parse("https://example.com/thing.exe").unwrap())
                                    .product_code("{a88c2543-9c04-4fc4-b2bd-bed6daff4341}".into())
                                    .size(1000)
                                    .installed_size(100000)
                                    .build(),
                            ))
                            .build(),
                        payload::Target::builder()
                            .platform("macos".into())
                            .arch(Some("x86_64".into()))
                            .dependencies(deps.clone())
                            .payload(payload::Payload::MacOSPackage(
                                payload::macos::Package::builder()
                                    .url(url::Url::parse("https://example.com/thing.pkg").unwrap())
                                    .pkg_id("com.example.test-package".into())
                                    .size(1000)
                                    .installed_size(100000)
                                    .targets({
                                        let mut map = std::collections::BTreeSet::new();
                                        map.insert(payload::macos::InstallTarget::System);
                                        map.insert(payload::macos::InstallTarget::User);
                                        map
                                    })
                                    .build(),
                            ))
                            .build(),
                        payload::Target::builder()
                            .platform("ios".into())
                            .arch(Some("arm64".into()))
                            .dependencies(deps.clone())
                            .payload(payload::Payload::TarballPackage(
                                payload::tarball::Package::builder()
                                    .url(url::Url::parse("https://example.com/thing.txz").unwrap())
                                    .size(1000)
                                    .installed_size(100000)
                                    .build(),
                            ))
                            .build(),
                    ])
                    .build(),
                package::Release::builder()
                    .version(package::Version::new("1.2.3").unwrap())
                    .channel(Some("test".to_string()))
                    .authors(vec!["Test Person <test@example.com>".into()])
                    .license(Some("CC-1.0".to_string()))
                    .target(vec![
                        payload::Target::builder()
                            .platform("windows".to_string())
                            .arch(Some("x86_64".into()))
                            .dependencies(deps.clone())
                            .payload(payload::Payload::WindowsExecutable(
                                payload::windows::Executable::builder()
                                    .url(url::Url::parse("https://example.com/thing.exe").unwrap())
                                    .product_code("{a88c2543-9c04-4fc4-b2bd-bed6daff4341}".into())
                                    .size(1000)
                                    .installed_size(100000)
                                    .build(),
                            ))
                            .build(),
                        payload::Target::builder()
                            .platform("macos".into())
                            .arch(Some("x86_64".into()))
                            .dependencies(deps.clone())
                            .payload(payload::Payload::MacOSPackage(
                                payload::macos::Package::builder()
                                    .url(url::Url::parse("https://example.com/thing.pkg").unwrap())
                                    .pkg_id("com.example.test-package".into())
                                    .size(1000)
                                    .installed_size(100000)
                                    .build(),
                            ))
                            .build(),
                        payload::Target::builder()
                            .platform("ios".into())
                            .arch(Some("arm64".into()))
                            .dependencies(deps.clone())
                            .payload(payload::Payload::TarballPackage(
                                payload::tarball::Package::builder()
                                    .url(url::Url::parse("https://example.com/thing.txz").unwrap())
                                    .size(1000)
                                    .installed_size(100000)
                                    .build(),
                            ))
                            .build(),
                    ])
                    .build(),
            ])
            .build();

        let mut names = LangTagMap::new();
        names.insert("en".to_string(), "Another Package".to_string());
        let mut desc = LangTagMap::new();
        desc.insert(
            "en".to_string(),
            "A second test package for testing.".to_string(),
        );

        let mut deps = DependencyMap::new();
        deps.insert(
            DependencyKey::Local("some-other-dependency".to_string()),
            "*".to_string(),
        );

        let package2 = package::Descriptor::builder()
            .package(
                package::DescriptorData::builder()
                    .id("another-package".to_string())
                    .tags(vec!["category:test".to_string(), "language:en".to_string()])
                    .build(),
            )
            .name(names)
            .description(desc)
            .release(vec![
                package::Release::builder()
                    .version(package::Version::new("2.0.0-beta.3").unwrap())
                    .channel(Some("test".to_string()))
                    .authors(vec!["Test Person <test@example.com>".into()])
                    .license(Some("CC-1.0".to_string()))
                    .target(vec![
                        payload::Target::builder()
                            .platform("windows".to_string())
                            .arch(Some("x86_64".into()))
                            .dependencies(deps.clone())
                            .payload(payload::Payload::WindowsExecutable(
                                payload::windows::Executable::builder()
                                    .url(url::Url::parse("https://example.com/thing.exe").unwrap())
                                    .product_code("{a88c2543-9c04-4fc4-b2bd-bed6daff4341}".into())
                                    .size(1000)
                                    .installed_size(100000)
                                    .build(),
                            ))
                            .build(),
                        payload::Target::builder()
                            .platform("macos".into())
                            .arch(Some("x86_64".into()))
                            .dependencies(deps.clone())
                            .payload(payload::Payload::MacOSPackage(
                                payload::macos::Package::builder()
                                    .url(url::Url::parse("https://example.com/thing.pkg").unwrap())
                                    .pkg_id("com.example.test-package".into())
                                    .size(1000)
                                    .installed_size(100000)
                                    .build(),
                            ))
                            .build(),
                        payload::Target::builder()
                            .platform("ios".into())
                            .arch(Some("arm64".into()))
                            .dependencies(deps.clone())
                            .payload(payload::Payload::TarballPackage(
                                payload::tarball::Package::builder()
                                    .url(url::Url::parse("https://example.com/thing.txz").unwrap())
                                    .size(1000)
                                    .installed_size(100000)
                                    .build(),
                            ))
                            .build(),
                    ])
                    .build(),
                package::Release::builder()
                    .version(package::Version::new("2.0.0-alpha.13").unwrap())
                    .channel(Some("test".to_string()))
                    .authors(vec!["Test Person <test@example.com>".into()])
                    .license(Some("CC-1.0".to_string()))
                    .target(vec![
                        payload::Target::builder()
                            .platform("windows".to_string())
                            .arch(Some("x86_64".into()))
                            .dependencies(deps.clone())
                            .payload(payload::Payload::WindowsExecutable(
                                payload::windows::Executable::builder()
                                    .url(url::Url::parse("https://example.com/thing.exe").unwrap())
                                    .product_code("{a88c2543-9c04-4fc4-b2bd-bed6daff4341}".into())
                                    .size(1000)
                                    .installed_size(100000)
                                    .build(),
                            ))
                            .build(),
                        payload::Target::builder()
                            .platform("macos".into())
                            .arch(Some("x86_64".into()))
                            .dependencies(deps.clone())
                            .payload(payload::Payload::MacOSPackage(
                                payload::macos::Package::builder()
                                    .url(url::Url::parse("https://example.com/thing.pkg").unwrap())
                                    .pkg_id("com.example.test-package".into())
                                    .size(1000)
                                    .installed_size(100000)
                                    .build(),
                            ))
                            .build(),
                        payload::Target::builder()
                            .platform("ios".into())
                            .arch(Some("arm64".into()))
                            .dependencies(deps.clone())
                            .payload(payload::Payload::TarballPackage(
                                payload::tarball::Package::builder()
                                    .url(url::Url::parse("https://example.com/thing.txz").unwrap())
                                    .size(1000)
                                    .installed_size(100000)
                                    .build(),
                            ))
                            .build(),
                    ])
                    .build(),
            ])
            .build();

        println!("{}", toml::to_string_pretty(&package1).unwrap());
        println!("{}", toml::to_string_pretty(&package2).unwrap());
        println!(
            "{}",
            serde_json::to_string_pretty(&[&package1, &package2]).unwrap()
        );
    }
}

pub mod fbs {
    include!(concat!(env!("OUT_DIR"), "/index.rs"));
}