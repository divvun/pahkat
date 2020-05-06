use std::borrow::Cow;
use std::fs::{self, create_dir_all};
use std::io;
use std::path::{Path, PathBuf};

use pahkat_types::{package::Version, payload::Payload};
use typed_builder::TypedBuilder;

#[non_exhaustive]
#[derive(Debug, Clone, TypedBuilder)]
pub struct Request<'a> {
    pub repo_path: Cow<'a, Path>,
    pub id: Cow<'a, str>,
    pub platform: Cow<'a, str>,
    pub channel: Option<Cow<'a, str>>,
    pub url: Cow<'a, url::Url>,
    pub version: Cow<'a, Version>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Default, TypedBuilder)]
pub struct PartialRequest<'a> {
    #[builder(default)]
    pub repo_path: Option<&'a Path>,
    #[builder(default)]
    pub id: Option<&'a str>,
    #[builder(default)]
    pub platform: Option<&'a str>,
    #[builder(default)]
    pub channel: Option<Option<&'a str>>,
    #[builder(default)]
    pub url: Option<&'a url::Url>,
    #[builder(default)]
    pub version: Option<&'a Version>,
}

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("Provided path was invalid")]
    PathError(#[source] io::Error),

    #[error("Could not find repository at provided path")]
    NoRepo(#[from] FindRepoError),

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
    let file = dbg!(fs::read_to_string(path.join("index.toml"))).ok()?;
    let repo: pahkat_types::repo::Repository = dbg!(toml::from_str(&file)).ok()?;
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

        let channel = match partial.channel {
            Some(channel) => channel.map(|c| Cow::Borrowed(c)),
            Some(None) => None,
            None => Input::<String>::new()
                .with_prompt("Channel (or none for stable)")
                .interact()
                .map(|v| Cow::Owned(v))
                .ok(),
        };

        let platform = match partial.platform {
            Some(name) => Cow::Borrowed(name),
            None => Cow::Owned(
                Input::<String>::new()
                    .with_prompt("Platform target to update")
                    .interact()
                    .map_err(|_| RequestError::InvalidInput)?,
            ),
        };

        let url = match partial.url {
            Some(description) => Cow::Borrowed(description),
            None => Cow::Owned(
                Input::<url::Url>::new()
                    .with_prompt("New payload url")
                    .interact()
                    .map_err(|_| RequestError::InvalidInput)?,
            ),
        };

        let version = match partial.version {
            Some(tags) => Cow::Borrowed(tags),
            None => Cow::Owned(
                Input::<Version>::new()
                    .with_prompt("New release version")
                    .interact()
                    .map_err(|_| RequestError::InvalidInput)?,
            ),
        };

        Ok(Request {
            repo_path,
            id,
            channel,
            url,
            platform,
            version,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to create directory `{0}`")]
    DirCreateFailed(PathBuf, #[source] io::Error),

    #[error("Failed to write TOML file `{0}`")]
    WriteToml(PathBuf, #[source] io::Error),

    #[error("Failed to serialize TOML for `{0}`")]
    SerializeToml(PathBuf, #[source] toml::ser::Error),

    #[error("Could not find repository at provided path")]
    NoRepo(#[from] FindRepoError),

    #[error("Could not find a release on channel `{0:?}`")]
    NoRelease(Option<String>),

    #[error("Could not find a target for platform`{0}`")]
    NoPlatform(String),
}

pub fn update<'a>(request: Request<'a>) -> anyhow::Result<()> {
    println!("{:?}", request);

    let pkg_dir = find_repo(&request.repo_path)?
        .join("packages")
        .join(&*request.id);

    let pkg_path = pkg_dir.join("index.toml");
    let pkg_file = std::fs::read_to_string(&pkg_path)?;
    let mut package: pahkat_types::package::Descriptor = toml::from_str(&pkg_file)?;

    let channel = request.channel.to_owned().map(|c| c.into_owned());

    let mut release = package
        .release
        .iter_mut()
        .find(|r| r.channel == channel)
        .ok_or_else(|| Error::NoRelease(channel))?;

    // Update version
    dbg!(&release);
    release.version = request.version.clone().into_owned();
    dbg!(&release);

    let mut target = release
        .target
        .iter_mut()
        .find(|t| t.platform == request.platform)
        .ok_or(Error::NoPlatform(request.platform.into_owned()))?;

    dbg!(&target.payload);
    // Update url
    match &mut target.payload {
        Payload::WindowsExecutable(ref mut payload) => {
            payload.url = request.url.into_owned();
        }
        Payload::MacOSPackage(ref mut payload) => {
            payload.url = request.url.into_owned();
        }
        Payload::TarballPackage(ref mut payload) => {
            payload.url = request.url.into_owned();
        }
        _ => panic!(),
    };
    dbg!(&target.payload);

    dbg!(&package);

    // Write the toml
    let data =
        toml::to_string_pretty(&package).map_err(|e| Error::SerializeToml(pkg_path.clone(), e))?;
    fs::write(&pkg_path, data).map_err(|e| Error::WriteToml(pkg_path.to_path_buf(), e))?;

    Ok(())
}
