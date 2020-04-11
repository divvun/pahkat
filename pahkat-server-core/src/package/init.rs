use std::borrow::Cow;
use std::fs::{self, create_dir_all, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use typed_builder::TypedBuilder;
use url::Url;

use pahkat_types::{
    package::Index as PackagesIndex,
    repo::{Agent, Index, Repository},
    LangTagMap,
};

#[non_exhaustive]
#[derive(Debug, Clone, TypedBuilder)]
pub struct Request<'a> {
    pub repo_path: Cow<'a, Path>,
    pub id: Cow<'a, str>,
    pub name: Cow<'a, str>,
    pub description: Cow<'a, str>,
    pub tags: Cow<'a, [String]>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Default, TypedBuilder)]
pub struct PartialRequest<'a> {
    #[builder(default)]
    pub repo_path: Option<&'a Path>,
    #[builder(default)]
    pub id: Option<&'a str>,
    #[builder(default)]
    pub name: Option<&'a str>,
    #[builder(default)]
    pub description: Option<&'a str>,
    #[builder(default)]
    pub tags: Option<&'a [String]>,
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

        let name = match partial.name {
            Some(name) => Cow::Borrowed(name),
            None => Cow::Owned(
                Input::<String>::new()
                    .with_prompt("Repo name (in English)")
                    .interact()
                    .map_err(|_| RequestError::InvalidInput)?,
            ),
        };

        let description = match partial.description {
            Some(description) => Cow::Borrowed(description),
            None => Cow::Owned(
                Input::<String>::new()
                    .with_prompt("Repo description (in English)")
                    .interact()
                    .map_err(|_| RequestError::InvalidInput)?,
            ),
        };

        let tags = match partial.tags {
            Some(tags) if !tags.is_empty() => Cow::Borrowed(tags),
            _ => {
                let raw_tags = Input::<String>::new()
                    .with_prompt("Tags (optional, space-delimited)")
                    .allow_empty(true)
                    .interact()
                    .map_err(|_| RequestError::InvalidInput)?;
                if raw_tags.trim() != "" {
                    Cow::Owned(raw_tags.split(" ").map(str::to_string).collect::<Vec<_>>())
                } else {
                    Cow::Owned(vec![])
                }
            }
        };

        Ok(Request {
            repo_path,
            id,
            name,
            description,
            tags,
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
}

pub fn init<'a>(request: Request<'a>) -> Result<(), Error> {
    println!("{:?}", request);

    let pkg_dir = find_repo(&request.repo_path)?
        .join("packages")
        .join(&*request.id);

    // Create the basic index.toml file
    let data = pahkat_types::package::DescriptorData::builder()
        .id(request.id.into_owned())
        .tags(request.tags.into_owned())
        .build();
    let package = pahkat_types::package::Descriptor::builder()
        .package(data)
        .name(crate::make_lang_tag_map(request.name.into_owned()))
        .description(crate::make_lang_tag_map(request.description.into_owned()))
        .build();

    // Create the directory for this identifier
    create_dir_all(&pkg_dir).map_err(|e| Error::DirCreateFailed(pkg_dir.clone(), e))?;
    let pkg_path = pkg_dir.join("index.toml");

    // Write the toml
    let data = toml::to_string(&package).map_err(|e| Error::SerializeToml(pkg_path.clone(), e))?;
    fs::write(&pkg_path, data).map_err(|e| Error::WriteToml(pkg_path.to_path_buf(), e))?;

    Ok(())
}
