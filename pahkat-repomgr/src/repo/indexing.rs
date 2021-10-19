use fbs::FlatBufferBuilder;
use std::borrow::Cow;
use std::path::Path;
use typed_builder::TypedBuilder;

pub fn index(request: Request<'_>) -> anyhow::Result<()> {
    log::debug!("Attempting to load repo in path: {:?}", &request.path);
    let packages_path = request.path.join("packages");
    std::fs::create_dir_all(&packages_path)?;

    // Attempt to make strings directory if it doesn't exist
    let strings_path = request.path.join("strings");
    std::fs::create_dir_all(&strings_path)?;

    // Find all package descriptor TOMLs
    let packages = std::fs::read_dir(&*packages_path)?
        .filter_map(Result::ok)
        .filter(|x| {
            let v = x.file_type().ok().map(|x| x.is_dir()).unwrap_or(false);
            log::trace!("Attempting {:?} := {:?}", &x, &v);
            v
        })
        .filter_map(|x| {
            let path = x.path().join("index.toml");
            log::trace!("Attempting read to string: {:?}", &path);
            let file = match std::fs::read_to_string(&path) {
                Ok(v) => v,
                Err(e) => {
                    log::error!("Could not handle path: {:?}", &path);
                    log::error!("{}", e);
                    log::error!("Continuing.");
                    return None;
                }
            };
            let package: pahkat_types::package::Package = match toml::from_str(&file) {
                Ok(v) => v,
                Err(e) => {
                    log::error!("Could not parse: {:?}", &path);
                    log::error!("{}", e);
                    log::error!("Continuing.");
                    return None;
                }
            };
            Some(package)
        })
        .collect::<Vec<pahkat_types::package::Package>>();

    let mut builder = FlatBufferBuilder::new();
    let index = build_index(&mut builder, &packages)?;

    std::fs::write(packages_path.join("index.bin"), index)?;
    log::trace!("Finished writing index.bin");

    Ok(())
}

#[non_exhaustive]
#[derive(Debug, Clone, TypedBuilder)]
pub struct Request<'a> {
    pub path: Cow<'a, Path>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Default, TypedBuilder)]
pub struct PartialRequest<'a> {
    #[builder(default)]
    pub path: Option<&'a Path>,
}

impl<'a> crate::Request for Request<'a> {
    type Error = std::convert::Infallible;
    type Partial = PartialRequest<'a>;

    fn new_from_user_input(partial: Self::Partial) -> Result<Self, Self::Error> {
        Ok(Request {
            path: partial
                .path
                .map(Cow::Borrowed)
                .unwrap_or_else(|| Cow::Owned(std::env::current_dir().unwrap())),
        })
    }
}

fn vectorize_strings<'a>(
    keys: Vec<fbs::WIPOffset<&'a str>>,
    builder: &mut FlatBufferBuilder<'a>,
) -> fbs::WIPOffset<fbs::Vector<'a, fbs::ForwardsUOffset<&'a str>>> {
    let len = keys.len();
    builder.start_vector::<fbs::ForwardsUOffset<&'_ str>>(len);
    for key in keys.into_iter().rev() {
        builder.push(key);
    }
    builder.end_vector(len)
}

fn vectorize_lang_map<'a, 'd>(
    lang_map: &'d pahkat_types::LangTagMap<String>,
    lang_keys: &mut std::collections::HashMap<&'d str, fbs::WIPOffset<&'a str>>,
    builder: &mut FlatBufferBuilder<'a>,
) -> (
    Option<fbs::WIPOffset<fbs::Vector<'a, fbs::ForwardsUOffset<&'a str>>>>,
    Option<fbs::WIPOffset<fbs::Vector<'a, fbs::ForwardsUOffset<&'a str>>>>,
) {
    let (name_keys, name_values): (Vec<_>, Vec<_>) = lang_map
        .iter()
        .map(|(key, value)| {
            let lang_key_ref = lang_keys
                .entry(key)
                .or_insert_with(|| builder.create_string(key))
                .clone();
            let value_ref = builder.create_string(value);
            (lang_key_ref, value_ref)
        })
        .unzip();

    let (name_keys_ref, name_values_ref) = if name_keys.is_empty() {
        (None, None)
    } else {
        let keys_ref = vectorize_strings(name_keys, builder);
        let values_ref = vectorize_strings(name_values, builder);
        (Some(keys_ref), Some(values_ref))
    };

    (name_keys_ref, name_values_ref)
}

fn create_payload_windows_exe<'a>(
    payload: &pahkat_types::payload::windows::Executable,
    builder: &mut FlatBufferBuilder<'a>,
) -> fbs::WIPOffset<fbs::UnionWIPOffset> {
    let url = builder.create_string(payload.url.as_str());
    let product_code = builder.create_string(payload.product_code.as_str());

    use crate::fbs::pahkat::WindowsExecutableKind;
    let kind = match payload.kind.as_ref().map(|x| &**x) {
        Some("msi") => WindowsExecutableKind::Msi,
        Some("nsis") => WindowsExecutableKind::Nsis,
        Some("inno") => WindowsExecutableKind::Inno,
        _ => WindowsExecutableKind::NONE,
    };

    let args = payload
        .args
        .as_ref()
        .map(|x| builder.create_string(x.as_str()));
    let uninstall_args = payload
        .uninstall_args
        .as_ref()
        .map(|x| builder.create_string(x.as_str()));

    use crate::fbs::pahkat::WindowsExecutableFlag;
    use pahkat_types::payload::windows::RebootSpec;

    let mut flags = 0u8;
    if payload.requires_reboot.contains(&RebootSpec::Install) {
        flags |= WindowsExecutableFlag::RequiresRebootOnInstall as u8;
    }
    if payload.requires_reboot.contains(&RebootSpec::Update) {
        flags |= WindowsExecutableFlag::RequiresRebootOnUpdate as u8;
    }
    if payload.requires_reboot.contains(&RebootSpec::Install) {
        flags |= WindowsExecutableFlag::RequiresRebootOnUninstall as u8;
    }

    let args = crate::fbs::pahkat::WindowsExecutableArgs {
        url,
        product_code,
        flags,
        kind,
        size: payload.size,
        installed_size: payload.installed_size,
        args,
        uninstall_args,
    };

    crate::fbs::pahkat::WindowsExecutable::create(builder, &args).as_union_value()
}

fn create_payload_macos_pkg<'a>(
    payload: &pahkat_types::payload::macos::Package,
    builder: &mut FlatBufferBuilder<'a>,
) -> fbs::WIPOffset<fbs::UnionWIPOffset> {
    let url = builder.create_string(payload.url.as_str());
    let pkg_id = builder.create_string(payload.pkg_id.as_str());

    use crate::fbs::pahkat::MacOSPackageFlag;
    use pahkat_types::payload::macos::RebootSpec;

    let mut flags = 0u8;
    if payload.requires_reboot.contains(&RebootSpec::Install) {
        flags |= MacOSPackageFlag::RequiresRebootOnInstall as u8;
    }
    if payload.requires_reboot.contains(&RebootSpec::Update) {
        flags |= MacOSPackageFlag::RequiresRebootOnUpdate as u8;
    }
    if payload.requires_reboot.contains(&RebootSpec::Install) {
        flags |= MacOSPackageFlag::RequiresRebootOnUninstall as u8;
    }

    use pahkat_types::payload::macos::InstallTarget;

    if payload.targets.is_empty() {
        flags |= MacOSPackageFlag::TargetSystem as u8;
    } else {
        for target in payload.targets.iter() {
            match target {
                InstallTarget::System => flags |= MacOSPackageFlag::TargetSystem as u8,
                InstallTarget::User => flags |= MacOSPackageFlag::TargetUser as u8,
            }
        }
    }

    let args = crate::fbs::pahkat::MacOSPackageArgs {
        url,
        pkg_id,
        flags,
        size: payload.size,
        installed_size: payload.installed_size,
    };

    crate::fbs::pahkat::MacOSPackage::create(builder, &args).as_union_value()
}

fn create_payload_tarball_pkg<'a>(
    payload: &pahkat_types::payload::tarball::Package,
    builder: &mut FlatBufferBuilder<'a>,
) -> fbs::WIPOffset<fbs::UnionWIPOffset> {
    log::debug!("Tarball: {}", &payload.url);
    let url = builder.create_string(payload.url.as_str());
    let args = crate::fbs::pahkat::TarballPackageArgs {
        url,
        size: payload.size,
        installed_size: payload.installed_size,
    };

    crate::fbs::pahkat::TarballPackage::create(builder, &args).as_union_value()
}

fn create_targets<'d, 'a>(
    targets: &'d Vec<pahkat_types::payload::Target>,
    builder: &mut FlatBufferBuilder<'a>,
) -> fbs::WIPOffset<fbs::Vector<'a, fbs::ForwardsUOffset<crate::fbs::pahkat::Target<&'a [u8]>>>> {
    let targets = targets
        .iter()
        .map(|target| {
            let platform = builder.create_string(&target.platform);

            // TODO: cache keys
            let (dependencies_keys, dependencies_values): (Vec<_>, Vec<_>) = target
                .dependencies
                .iter()
                .map(|(key, value)| {
                    (
                        builder.create_string(key.as_str()),
                        builder.create_string(&value),
                    )
                })
                .unzip();
            let (dependencies_keys, dependencies_values) = if dependencies_keys.is_empty() {
                (None, None)
            } else {
                (
                    Some(vectorize_strings(dependencies_keys, builder)),
                    Some(vectorize_strings(dependencies_values, builder)),
                )
            };

            let arch = target.arch.as_ref().map(|x| builder.create_string(&x));

            use crate::fbs::pahkat::fbs_gen::PayloadType;
            use pahkat_types::payload::Payload;

            let (payload_type, payload) = match &target.payload {
                Payload::WindowsExecutable(p) => (
                    PayloadType::WindowsExecutable,
                    create_payload_windows_exe(p, builder),
                ),
                Payload::MacOSPackage(p) => (
                    PayloadType::MacOSPackage,
                    create_payload_macos_pkg(p, builder),
                ),
                Payload::TarballPackage(p) => (
                    PayloadType::TarballPackage,
                    create_payload_tarball_pkg(p, builder),
                ),
                _ => panic!("Payload must exist"),
            };

            let args = crate::fbs::pahkat::TargetArgs {
                platform,
                arch,
                dependencies_keys,
                dependencies_values,
                payload_type,
                payload,
            };

            crate::fbs::pahkat::Target::create(builder, &args)
        })
        .collect::<Vec<_>>();

    let len = targets.len();
    builder.start_vector::<fbs::ForwardsUOffset<crate::fbs::pahkat::Target<&'_ [u8]>>>(len);
    for target in targets.into_iter().rev() {
        builder.push(target);
    }
    builder.end_vector(len)
}

fn create_releases<'d, 'a>(
    releases: &'d Vec<pahkat_types::package::Release>,
    release_keys: &mut std::collections::HashMap<String, fbs::WIPOffset<&'a str>>,
    str_keys: &mut std::collections::HashMap<&'d str, fbs::WIPOffset<&'a str>>,
    builder: &mut FlatBufferBuilder<'a>,
) -> fbs::WIPOffset<fbs::Vector<'a, fbs::ForwardsUOffset<crate::fbs::pahkat::Release<&'a [u8]>>>> {
    let releases = releases
        .iter()
        .map(|release| {
            // TODO: handle version type properly
            use pahkat_types::package::version::Version;
            let (version_type, version) = match &release.version {
                // Version::Opaque => 1u8,
                Version::Semantic(v) => (2u8, v.to_string()),
                _ => panic!(),
            };
            let version = *release_keys
                .entry(version.clone())
                .or_insert_with(|| builder.create_string(&*version));
            let channel = release.channel.as_ref().map(|x| {
                *str_keys
                    .entry(&*x)
                    .or_insert_with(|| builder.create_string(&*x))
            });

            let authors = release
                .authors
                .iter()
                .map(|x| {
                    *str_keys
                        .entry(&*x)
                        .or_insert_with(|| builder.create_string(&*x))
                })
                .collect::<Vec<_>>();
            let authors = if authors.is_empty() {
                None
            } else {
                Some(vectorize_strings(authors, builder))
            };

            let license = release.license.as_ref().map(|x| {
                *str_keys
                    .entry(&*x)
                    .or_insert_with(|| builder.create_string(&*x))
            });
            let license_url = release.license_url.as_ref().map(|x| {
                *str_keys
                    .entry(x.as_str())
                    .or_insert_with(|| builder.create_string(x.as_str()))
            });
            let target = Some(create_targets(&release.target, builder));

            let args = crate::fbs::pahkat::ReleaseArgs {
                version_type,
                version,
                channel,
                authors,
                license,
                license_url,
                target,
            };

            crate::fbs::pahkat::Release::create(builder, &args)
        })
        .collect::<Vec<_>>();

    let len = releases.len();
    builder.start_vector::<fbs::ForwardsUOffset<crate::fbs::pahkat::Release<&'_ [u8]>>>(len);
    for release in releases.into_iter().rev() {
        builder.push(release);
    }
    builder.end_vector(len)
}

fn build_index<'a>(
    builder: &'a mut FlatBufferBuilder<'a>,
    packages: &[pahkat_types::package::Package],
) -> anyhow::Result<&'a [u8]> {
    let mut owned_keys = std::collections::HashMap::new();
    let mut str_keys = std::collections::HashMap::new();

    // Use the count to create the vectors we need
    let id_refs = packages
        .iter()
        .map(pahkat_types::package::Package::id)
        .map(|id| builder.create_string(id))
        .collect::<Vec<_>>();

    builder.start_vector::<fbs::ForwardsUOffset<&'_ str>>(id_refs.len());
    for id in id_refs.iter().rev() {
        builder.push(id.clone());
    }
    let packages_keys = Some(builder.end_vector(id_refs.len()));

    builder.start_vector::<u8>(id_refs.len());
    for _ in id_refs.iter().rev() {
        builder.push(crate::fbs::pahkat::fbs_gen::PackageType::Descriptor as u8);
    }
    let packages_values_types = Some(builder.end_vector::<u8>(id_refs.len()));

    let packages_values = id_refs
        .iter()
        .zip(packages.iter())
        .map(|(id_ref, package)| {
            let descriptor = match package {
                pahkat_types::package::Package::Concrete(p) => p,
                _ => panic!("Unsupported package type"),
            };

            let tags = if descriptor.package.tags.is_empty() {
                None
            } else {
                let tags = descriptor
                    .package
                    .tags
                    .iter()
                    .map(|x| {
                        *str_keys
                            .entry(&**x)
                            .or_insert_with(|| builder.create_string(&*x))
                    })
                    .collect::<Vec<_>>();
                let len = tags.len();
                builder.start_vector::<fbs::ForwardsUOffset<&'_ str>>(len);
                for tag_ref in tags.into_iter().rev() {
                    builder.push(tag_ref);
                }
                Some(builder.end_vector(len))
            };

            let (name_keys, name_values) =
                vectorize_lang_map(&descriptor.name, &mut str_keys, builder);
            let (description_keys, description_values) =
                vectorize_lang_map(&descriptor.description, &mut str_keys, builder);

            let release =
                create_releases(&descriptor.release, &mut owned_keys, &mut str_keys, builder);

            let args = crate::fbs::pahkat::DescriptorArgs {
                id: id_ref.clone(),
                name_keys,
                name_values,
                description_keys,
                description_values,
                tags,
                release: Some(release),
            };
            crate::fbs::pahkat::Descriptor::create(builder, &args)
        })
        .collect::<Vec<_>>();

    builder.start_vector::<fbs::ForwardsUOffset<crate::fbs::pahkat::Descriptor<&'_ [u8]>>>(
        id_refs.len(),
    );
    for package_value in packages_values.into_iter().rev() {
        builder.push(package_value);
    }
    let packages_values = Some(builder.end_vector(id_refs.len()));

    let args = crate::fbs::pahkat::PackagesArgs {
        packages_values_types,
        packages_keys,
        packages_values,
    };

    let root = crate::fbs::pahkat::Packages::create(builder, &args);

    builder.finish_minimal(root);
    Ok(builder.finished_data())
}
