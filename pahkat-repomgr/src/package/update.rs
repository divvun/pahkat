use std::borrow::Cow;
use std::fs::{self};
use std::io;
use std::path::{Path, PathBuf};

use pahkat_types::package::Version;
use typed_builder::TypedBuilder;

#[non_exhaustive]
#[derive(Debug, Clone, TypedBuilder)]
pub struct Request<'a> {
    pub repo_path: Cow<'a, Path>,
    pub id: Cow<'a, str>,
    pub name: Option<Cow<'a, pahkat_types::LangTagMap<String>>>,
    pub description: Option<Cow<'a, pahkat_types::LangTagMap<String>>>,
    pub channel: Option<Cow<'a, str>>,
    pub version: Cow<'a, Version>,
    pub target: Cow<'a, pahkat_types::payload::Target>,
    pub url: Option<Cow<'a, url::Url>>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Default, TypedBuilder)]
pub struct PartialRequest<'a> {
    #[builder(default)]
    pub repo_path: Option<&'a Path>,
    #[builder(default)]
    pub id: Option<&'a str>,
    #[builder(default)]
    pub name: Option<&'a pahkat_types::LangTagMap<String>>,
    #[builder(default)]
    pub description: Option<&'a pahkat_types::LangTagMap<String>>,
    #[builder(default)]
    pub platform: Option<&'a str>,
    #[builder(default)]
    pub arch: Option<&'a str>,
    #[builder(default)]
    pub channel: Option<&'a str>,
    #[builder(default)]
    pub version: Option<&'a Version>,
    #[builder(default)]
    pub payload_path: Option<&'a Path>,
    #[builder(default)]
    pub url: Option<&'a url::Url>,
}

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("Provided path was invalid")]
    PathError(#[source] io::Error),

    #[error("Could not find repository at provided path")]
    NoRepo(#[from] FindRepoError),

    #[error("Could not read payload TOML file")]
    Io(#[from] std::io::Error),

    #[error("Could not read payload TOML file")]
    PayloadToml(#[from] toml::de::Error),

    #[error("Invalid input")]
    InvalidInput,
}

#[derive(Debug, thiserror::Error)]
pub enum FindRepoError {
    #[error("IO error")]
    Io(#[from] io::Error),

    #[error("No repository found for given path")]
    NotFound,
}

fn open_repo(path: &Path) -> Option<pahkat_types::repo::Repository> {
    let file = fs::read_to_string(path.join("index.toml")).ok()?;
    let repo: pahkat_types::repo::Repository = toml::from_str(&file).ok()?;
    Some(repo)
}

fn find_repo(path: &Path) -> Result<&Path, FindRepoError> {
    let mut path = path;

    if path.ends_with("index.toml") {
        path = path.parent().unwrap();
    }

    if let Some(_) = open_repo(path) {
        return Ok(path);
    }

    while let Some(parent) = path.parent() {
        path = parent;
        if let Some(_) = open_repo(path) {
            return Ok(path);
        }
    }

    Err(FindRepoError::NotFound)
}

impl<'a> crate::Request for Request<'a> {
    type Error = RequestError;
    type Partial = PartialRequest<'a>;

    fn new_from_user_input(partial: Self::Partial) -> Result<Self, Self::Error> {
        use dialoguer::Input;

        let repo_path = match partial.repo_path {
            Some(path) => Cow::Borrowed(path),
            None => Input::<String>::new()
                .default(
                    std::env::current_dir()
                        .ok()
                        .and_then(|x| x.to_str().map(str::to_string))
                        .unwrap_or_else(|| ".".into()),
                )
                .with_prompt("Repository Path")
                .interact()
                .map(|p| Cow::Owned(PathBuf::from(p)))
                .map_err(RequestError::PathError)?,
        };

        let _ = find_repo(&repo_path)?;

        let id = match partial.id {
            Some(id) => Cow::Borrowed(id),
            None => Cow::Owned(
                Input::<String>::new()
                    .with_prompt("Package identifier")
                    .interact()
                    .map_err(|_| RequestError::InvalidInput)?,
            ),
        };

        let payload_path = match partial.payload_path {
            Some(path) => Cow::Borrowed(path),
            None => Cow::Owned(
                Input::<String>::new()
                    .with_prompt("Target path (toml)")
                    .interact()
                    .map_err(|_| RequestError::InvalidInput)
                    .map(std::path::PathBuf::from)?,
            ),
        };

        let payload = std::fs::read_to_string(payload_path)?;
        let target: pahkat_types::payload::Target = toml::from_str(&payload)?;

        let channel = match partial.channel {
            Some(channel) => {
                if channel == "" {
                    None
                } else {
                    Some(Cow::Borrowed(channel))
                }
            }
            None => Input::<String>::new()
                .with_prompt("Channel (or none for stable)")
                .allow_empty(true)
                .interact()
                .map_err(|_| RequestError::InvalidInput)
                .map(|v| if v == "" { None } else { Some(Cow::Owned(v)) })?,
        };

        let version = match partial.version {
            Some(tags) => Cow::Borrowed(tags),
            None => Cow::Owned(
                Input::<Version>::new()
                    .with_prompt("Release version")
                    .interact()
                    .map_err(|_| RequestError::InvalidInput)?,
            ),
        };

        Ok(Request {
            repo_path,
            id,
            name: partial.name.map(|x| Cow::Borrowed(x)),
            description: partial.description.map(|x| Cow::Borrowed(x)),
            channel,
            version,
            target: Cow::Owned(target),
            url: partial.url.map(|x| Cow::Borrowed(x)),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to read descriptor index: `{0}`")]
    ReadFailed(PathBuf, #[source] io::Error),

    #[error("Failed to read TOML file `{0}`")]
    ReadToml(PathBuf, #[source] toml::de::Error),

    #[error("Failed to create directory `{0}`")]
    DirCreateFailed(PathBuf, #[source] io::Error),

    #[error("Failed to write TOML file `{0}`")]
    WriteToml(PathBuf, #[source] io::Error),

    #[error("Failed to serialize TOML for `{0}`")]
    SerializeToml(PathBuf, #[source] toml::ser::Error),

    #[error("Could not find repository at provided path")]
    NoRepo(#[from] FindRepoError),
}

pub fn update<'a>(request: Request<'a>) -> Result<(), Error> {
    use std::ops::Deref;
    log::debug!("{:?}", request);

    let pkg_dir = find_repo(&request.repo_path)?
        .join("packages")
        .join(&*request.id);

    let pkg_path = pkg_dir.join("index.toml");
    log::debug!("Loading {}", pkg_path.display());

    let pkg_file =
        std::fs::read_to_string(&pkg_path).map_err(|e| Error::ReadFailed(pkg_path.clone(), e))?;
    let mut descriptor: pahkat_types::package::Descriptor =
        toml::from_str(&pkg_file).map_err(|e| Error::ReadToml(pkg_path.clone(), e))?;

    log::debug!("Loaded {}", pkg_path.display());

    let channel = request.channel.as_ref().map(|x| x.deref().to_string());

    // Update name and description if provided
    if let Some(name) = request.name.as_ref() {
        log::debug!("Setting name to {:?}", &name);
        descriptor.name = name.deref().clone();
    }

    if let Some(description) = request.description.as_ref() {
        log::debug!("Setting description to {:?}", &description);
        descriptor.description = description.deref().clone();
    }

    // Check if a release exists that meets this criteria
    let release = match descriptor
        .release
        .iter_mut()
        .find(|x| &x.version == &*request.version && x.channel == channel)
    {
        Some(release) => {
            log::info!("Found release!");
            release
        }
        None => {
            log::info!("No release; creating.");
            // Insert new releases at front
            descriptor.release.insert(
                0,
                pahkat_types::package::Release::builder()
                    .channel(channel)
                    .version(request.version.deref().clone())
                    .build(),
            );
            descriptor.release.first_mut().unwrap()
        }
    };

    // Check if a target exists that meets this criteria
    let target = match release
        .target
        .iter_mut()
        .find(|x| x.platform == request.target.platform)
    {
        Some(target) => {
            log::info!("Found target!");
            *target = (*request.target).to_owned();
            target
        }
        None => {
            log::info!("No target; creating.");
            release.target.insert(
                0,
                pahkat_types::payload::Target::builder()
                    .dependencies(request.target.dependencies.clone())
                    .platform(request.target.platform.to_string())
                    .payload(request.target.payload.clone())
                    .build(),
            );
            release.target.first_mut().unwrap()
        }
    };

    if let Some(url) = request.url {
        log::info!("Setting URL to {}", &url);
        target.payload.set_url(url.into_owned());
    }

    // Write the toml
    let data = toml::to_string_pretty(&descriptor)
        .map_err(|e| Error::SerializeToml(pkg_path.clone(), e))?;
    fs::write(&pkg_path, data).map_err(|e| Error::WriteToml(pkg_path.to_path_buf(), e))?;
    log::info!("Wrote descriptor to {}", pkg_path.display());

    Ok(())
}
