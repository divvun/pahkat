use std::convert::TryFrom;

use crate::generated::pahkat as pahkat_fbs;
use types::DependencyKey;

pub(crate) trait DescriptorExt {
    fn name(&self) -> Option<Map<'_, &'_ str, &'_ str>>;
    fn description(&self) -> Option<Map<'_, &'_ str, &'_ str>>;
}

pub(crate) trait TargetExt {
    fn dependencies(&self) -> Option<Map<'_, &'_ str, &'_ str>>;
}

pub(crate) trait PackagesExt<B: AsRef<[u8]>> {
    fn packages(&self) -> Option<Map<'_, &'_ str, pahkat_fbs::Descriptor<&'_ [u8]>>>;
}

impl PackagesExt<&'_ [u8]> for pahkat_fbs::Packages<&'_ [u8]> {
    fn packages(&self) -> Option<Map<'_, &'_ str, pahkat_fbs::Descriptor<&'_ [u8]>>> {
        let keys = self.packages_keys().ok()??;
        let values = self.packages_values().ok()??;
        Some(Map::new(keys, values))
    }
}

impl<B: AsRef<[u8]>> DescriptorExt for pahkat_fbs::Descriptor<B> {
    fn name(&self) -> Option<Map<'_, &'_ str, &'_ str>> {
        let keys = self.name_keys().ok()??;
        let values = self.name_values().ok()??;
        Some(Map::new(keys, values))
    }

    fn description(&self) -> Option<Map<'_, &'_ str, &'_ str>> {
        let keys = self.description_keys().ok()??;
        let values = self.description_values().ok()??;
        Some(Map::new(keys, values))
    }
}

impl<B: AsRef<[u8]>> TargetExt for pahkat_fbs::Target<B> {
    fn dependencies(&self) -> Option<Map<'_, &'_ str, &'_ str>> {
        let keys = self.dependencies_keys().ok()??;
        let values = self.dependencies_values().ok()??;
        Some(Map::new(keys, values))
    }
}

fn build_target<B: AsRef<[u8]>>(
    t: &pahkat_fbs::Target<B>,
) -> Result<pahkat_types::payload::Target, fbs::Error> {
    let platform = t.platform()?.to_string();
    let arch = t.arch()?.map(str::to_string);
    let dependencies = t
        .dependencies()
        .map(|x| {
            let mut out = std::collections::BTreeMap::new();
            for (k, v) in x.iter() {
                out.insert(DependencyKey::from(k), v.to_string());
            }
            out
        })
        .unwrap_or_else(|| Default::default());
    let payload = match t.payload()? {
        pahkat_fbs::Payload::WindowsExecutable(x) => {
            pahkat_types::payload::Payload::WindowsExecutable(
                pahkat_types::payload::windows::Executable::builder()
                    .url(x.url()?.parse::<url::Url>().unwrap())
                    .product_code(x.product_code()?.to_string())
                    .kind(match x.kind()?.unwrap() {
                        pahkat_fbs::WindowsExecutableKind::NONE => None,
                        x => Some(
                            pahkat_fbs::enum_name_windows_executable_kind(x)
                                .to_lowercase()
                                .to_string(),
                        ),
                    })
                    .size(x.size()?.unwrap())
                    .installed_size(x.installed_size()?.unwrap())
                    .build(),
            )
        }
        pahkat_fbs::Payload::MacOSPackage(x) => pahkat_types::payload::Payload::MacOSPackage(
            pahkat_types::payload::macos::Package::builder()
                .url(x.url()?.parse::<url::Url>().unwrap())
                .pkg_id(x.pkg_id()?.to_string())
                .size(x.size()?.unwrap())
                .installed_size(x.installed_size()?.unwrap())
                .build(),
        ),
        pahkat_fbs::Payload::TarballPackage(x) => pahkat_types::payload::Payload::TarballPackage(
            pahkat_types::payload::tarball::Package::builder()
                .url(x.url()?.parse::<url::Url>().unwrap())
                .size(x.size()?.unwrap())
                .installed_size(x.installed_size()?.unwrap())
                .build(),
        ),
    };

    Ok(pahkat_types::payload::Target::builder()
        .platform(platform)
        .arch(arch)
        .dependencies(dependencies)
        .payload(payload)
        .build())
}

impl<'a> TryFrom<&'a pahkat_fbs::Descriptor<&'a [u8]>> for pahkat_types::package::Descriptor {
    type Error = fbs::Error;

    fn try_from(pkg: &'a pahkat_fbs::Descriptor<&'a [u8]>) -> Result<Self, Self::Error> {
        use std::collections::BTreeMap;

        let descriptor = pahkat_types::package::Descriptor::builder()
            .package(
                pahkat_types::package::DescriptorData::builder()
                    .id(pkg.id()?.into())
                    .tags(
                        pkg.tags()?
                            .map(|tags| tags.iter().map(|x| x.unwrap_or("").to_string()).collect())
                            .unwrap_or(vec![]),
                    )
                    .build(),
            )
            .name(
                pkg.name()
                    .map(|x| {
                        let mut out = BTreeMap::new();
                        for (k, v) in x.iter() {
                            out.insert(k.to_string(), v.to_string());
                        }
                        out
                    })
                    .unwrap_or_else(|| Default::default()),
            )
            .description(
                pkg.description()
                    .map(|x| {
                        let mut out = BTreeMap::new();
                        for (k, v) in x.iter() {
                            out.insert(k.to_string(), v.to_string());
                        }
                        out
                    })
                    .unwrap_or_else(|| Default::default()),
            )
            .release(
                pkg.release()?
                    .unwrap()
                    .iter()
                    .filter_map(Result::ok)
                    .map(|x| {
                        let release = pahkat_types::package::Release::builder()
                            .version(
                                pahkat_types::package::version::Version::new(x.version()?).unwrap(),
                            )
                            .channel(x.channel()?.map(|x| x.to_string()))
                            .target(
                                x.target()?
                                    .unwrap()
                                    .iter()
                                    .filter_map(Result::ok)
                                    .map(|t| build_target(&t))
                                    .collect::<Result<Vec<_>, _>>()?,
                            )
                            .build();
                        Ok(release)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
            .build();

        Ok(descriptor)
    }
}

pub struct Map<'a, K, V> {
    keys: fbs::Vector<'a, fbs::ForwardsUOffset<K>>,
    values: fbs::Vector<'a, fbs::ForwardsUOffset<V>>,
    len: usize,
}

impl<'a, K, V> From<Map<'a, K, V>>
    for std::collections::BTreeMap<
        <<fbs::ForwardsUOffset<K> as fbs::Follow<'a>>::Inner as ToOwned>::Owned,
        <<fbs::ForwardsUOffset<V> as fbs::Follow<'a>>::Inner as ToOwned>::Owned,
    >
where
    K: PartialEq,
    fbs::ForwardsUOffset<K>: fbs::Follow<'a>,
    fbs::ForwardsUOffset<V>: fbs::Follow<'a>,
    <fbs::ForwardsUOffset<K> as fbs::Follow<'a>>::Inner: PartialEq + ToOwned,
    <<fbs::ForwardsUOffset<K> as fbs::Follow<'a>>::Inner as ToOwned>::Owned: PartialEq + Ord,
    <fbs::ForwardsUOffset<V> as fbs::Follow<'a>>::Inner: ToOwned,
{
    fn from(
        value: Map<'a, K, V>,
    ) -> std::collections::BTreeMap<
        <<fbs::ForwardsUOffset<K> as fbs::Follow<'a>>::Inner as ToOwned>::Owned,
        <<fbs::ForwardsUOffset<V> as fbs::Follow<'a>>::Inner as ToOwned>::Owned,
    > {
        let mut out = std::collections::BTreeMap::new();
        for (k, v) in value.iter() {
            out.insert(k.to_owned(), v.to_owned());
        }
        out
    }
}

impl<'a, K, V> Map<'a, K, V>
where
    K: PartialEq,
    fbs::ForwardsUOffset<K>: fbs::Follow<'a>,
    fbs::ForwardsUOffset<V>: fbs::Follow<'a>,
    <fbs::ForwardsUOffset<K> as fbs::Follow<'a>>::Inner: PartialEq,
{
    #[inline]
    fn new(
        keys: fbs::Vector<'a, fbs::ForwardsUOffset<K>>,
        values: fbs::Vector<'a, fbs::ForwardsUOffset<V>>,
    ) -> Map<'a, K, V> {
        Map {
            keys,
            values,
            len: keys.len().unwrap_or(0),
        }
    }

    #[inline]
    pub fn iter(
        &self,
    ) -> impl Iterator<
        Item = (
            <fbs::ForwardsUOffset<K> as fbs::Follow<'a>>::Inner,
            <fbs::ForwardsUOffset<V> as fbs::Follow<'a>>::Inner,
        ),
    > {
        self.keys
            .iter()
            .filter_map(Result::ok)
            .zip(self.values.iter().filter_map(Result::ok))
    }

    #[inline]
    pub fn get(
        &self,
        key: <fbs::ForwardsUOffset<K> as fbs::Follow<'a>>::Inner,
    ) -> Option<<fbs::ForwardsUOffset<V> as fbs::Follow<'a>>::Inner> {
        self.keys
            .iter()
            .filter_map(Result::ok)
            .position(|x| x == key)
            .map(|i| self.values.get(i).unwrap())
    }

    #[inline]
    pub fn keys(
        &self,
    ) -> impl Iterator<Item = <fbs::ForwardsUOffset<K> as fbs::Follow<'a>>::Inner> {
        self.keys.iter().filter_map(Result::ok)
    }

    #[inline]
    pub fn key(&self, index: usize) -> Option<<fbs::ForwardsUOffset<K> as fbs::Follow<'a>>::Inner> {
        if index >= self.len {
            None
        } else {
            Some(self.keys.get(index).unwrap())
        }
    }

    #[inline]
    pub fn value(
        &self,
        index: usize,
    ) -> Option<<fbs::ForwardsUOffset<V> as fbs::Follow<'a>>::Inner> {
        if index >= self.len {
            None
        } else {
            Some(self.values.get(index).unwrap())
        }
    }
}
