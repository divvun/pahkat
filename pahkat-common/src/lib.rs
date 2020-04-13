// #[macro_use]
// extern crate diesel;

// use std::collections::BTreeMap;
// use std::fmt;
// use std::fs;
// use std::fs::File;
// use std::io::Write;
// use std::path::{Path, PathBuf};

// use directories::ProjectDirs;
// use serde::{Deserialize, Serialize};

// // use pahkat_types::{Installer, Package, Packages, Repository, RepositoryAgent, Virtual, Virtuals};

// pub mod database;
// pub mod models;
// pub mod version;

// pub trait ProgressOutput {
//     fn info(&self, msg: &str);
//     fn generating(&self, thing: &str);
//     fn writing(&self, thing: &str);
//     fn inserting(&self, id: &str, version: &str);
//     fn error(&self, thing: &str);
//     fn warn(&self, thing: &str);
// }

// #[derive(Debug)]
// pub enum OpenIndexError {
//     FileError(std::io::Error),
//     JsonError(serde_json::Error),
// }

// #[derive(Debug)]
// pub enum DatabaseError {
//     PoolError(r2d2::Error),
//     OperationError(diesel::result::Error),
//     InputError(String, uuid::Error),
// }

// #[derive(Clone, Debug, Serialize, Deserialize)]
// pub struct UploadParams {
//     pub channel: String,
//     pub version: String,
//     pub installer: Installer,
// }

// impl From<diesel::result::Error> for DatabaseError {
//     fn from(item: diesel::result::Error) -> Self {
//         DatabaseError::OperationError(item)
//     }
// }

// impl From<r2d2::Error> for DatabaseError {
//     fn from(item: r2d2::Error) -> Self {
//         DatabaseError::PoolError(item)
//     }
// }

// impl fmt::Display for OpenIndexError {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match *self {
//             OpenIndexError::FileError(ref x) => write!(f, "{}", x),
//             OpenIndexError::JsonError(ref x) => write!(f, "{}", x),
//         }
//     }
// }

// pub fn db_path() -> PathBuf {
//     let mut db_path = ProjectDirs::from("no", "uit", "pahkat-server")
//         .expect("No home directory found")
//         .data_dir()
//         .to_owned();
//     db_path.push("db.sqlite3");

//     db_path
// }

// pub fn open_repo(path: &Path) -> Result<Repository, OpenIndexError> {
//     let file = File::open(path.join("index.json")).map_err(OpenIndexError::FileError)?;
//     let index = serde_json::from_reader(file).map_err(OpenIndexError::JsonError)?;
//     Ok(index)
// }

// pub fn open_package(path: &Path, channel: Option<&str>) -> Result<Package, OpenIndexError> {
//     let file = File::open(path.join(index_fn(channel))).map_err(OpenIndexError::FileError)?;
//     let index = serde_json::from_reader(file).map_err(OpenIndexError::JsonError)?;
//     Ok(index)
// }

// pub fn repo_index<T: ProgressOutput>(cur_dir: &Path, output: &T) {
//     if let Err(err) = open_repo(&cur_dir) {
//         output.error(&format!("{}", err));
//         output.error("Repo does not exist or is invalid; aborting.");
//         return;
//     }

//     if !cur_dir.join("packages").exists() {
//         fs::create_dir(cur_dir.join("packages")).unwrap();
//     }

//     if !cur_dir.join("virtuals").exists() {
//         fs::create_dir(cur_dir.join("virtuals")).unwrap();
//     }

//     // TODO: would be nice if this were transactional

//     let repo_index = generate_repo_index_meta(&cur_dir, output);
//     write_repo_index_meta(&cur_dir, &repo_index, output);

//     for channel in repo_index.channels.iter() {
//         let channel: Option<&str> = if channel == &repo_index.default_channel {
//             None
//         } else {
//             Some(&*channel)
//         };

//         let package_index = generate_repo_index_packages(&cur_dir, &repo_index, channel, output);
//         write_repo_index_packages(&cur_dir, &repo_index, &package_index, channel, output);

//         let virtuals_index = generate_repo_index_virtuals(&cur_dir, &repo_index, channel, output);
//         write_repo_index_virtuals(&cur_dir, &repo_index, &virtuals_index, channel, output);
//     }
// }

// fn write_index<T: ProgressOutput, U: Serialize>(
//     cur_dir: &Path,
//     repo: &Repository,
//     index: &U,
//     channel: Option<&str>,
//     output: &T,
//     name: &str,
// ) {
//     let json = serde_json::to_string_pretty(index).unwrap();
//     let pkg_path = cur_dir.join(name);

//     output.writing(&format!(
//         "{} {} index",
//         channel.unwrap_or(&repo.default_channel),
//         name
//     ));
//     let mut file = File::create(&pkg_path.join(index_fn(channel))).unwrap();
//     file.write_all(json.as_bytes()).unwrap();
//     file.write_all(&[b'\n']).unwrap();
// }

// fn generate_repo_index_meta<T: ProgressOutput>(repo_path: &Path, output: &T) -> Repository {
//     output.generating("repository index");

//     let file = File::open(repo_path.join("index.json")).unwrap();
//     let mut repo_index: Repository =
//         serde_json::from_reader(file).expect(repo_path.join("index.json").to_str().unwrap());

//     repo_index._type = ld_type!("Repository");
//     repo_index.agent = RepositoryAgent::default();

//     repo_index
// }

// fn write_repo_index_meta<T: ProgressOutput>(repo_path: &Path, repo_index: &Repository, output: &T) {
//     let json = serde_json::to_string_pretty(&repo_index).unwrap();

//     output.writing("repository index");
//     let mut file = File::create(&repo_path.join("index.json")).unwrap();
//     file.write_all(json.as_bytes()).unwrap();
//     file.write_all(&[b'\n']).unwrap();
// }

// fn generate_repo_index_packages<T: ProgressOutput>(
//     cur_dir: &Path,
//     repo: &Repository,
//     channel: Option<&str>,
//     output: &T,
// ) -> Packages {
//     output.generating(&format!(
//         "{} packages index",
//         channel.unwrap_or(&repo.default_channel)
//     ));

//     let pkg_path = cur_dir.join("packages");
//     let pkgs: Vec<Package> = fs::read_dir(&pkg_path)
//         .unwrap()
//         .map(|x| x.unwrap().path())
//         .filter_map(|path| {
//             if !path.is_dir() {
//                 if let Some(ex) = path.extension() {
//                     if ex == "json" {
//                         return None;
//                     }
//                 }

//                 let relpath = pathdiff::diff_paths(&*path, cur_dir).unwrap();
//                 output.warn(&format!("{:?} is not a directory; skipping", &relpath));
//                 return None;
//             }

//             if !path.join(index_fn(channel)).exists() {
//                 if channel.is_none() {
//                     let relpath = pathdiff::diff_paths(&*path, cur_dir).unwrap();
//                     output.warn(&format!(
//                         "{:?} does not contain {:?}; skipping",
//                         &relpath,
//                         index_fn(channel)
//                     ));
//                 }
//                 return None;
//             }

//             let index_path = path.join(index_fn(channel));
//             let file = File::open(&index_path).unwrap();
//             let pkg_index: Package = match serde_json::from_reader(file) {
//                 Ok(x) => x,
//                 Err(err) => {
//                     let relpath = pathdiff::diff_paths(&*index_path, cur_dir).unwrap();
//                     output.error(&format!("Error parsing path {:?}:", &relpath));
//                     output.error(&format!("{}", err));
//                     return None;
//                 }
//             };

//             if pkg_index.installer.is_none() {
//                 output.warn(&format!(
//                     "{} {} has no installer; skipping",
//                     &pkg_index.id, &pkg_index.version
//                 ));
//                 return None;
//             }

//             output.inserting(&pkg_index.id, &pkg_index.version);
//             Some(pkg_index)
//         })
//         .collect();

//     let mut map = BTreeMap::new();
//     for pkg in pkgs.into_iter() {
//         map.insert(pkg.id.to_owned(), pkg);
//     }

//     if map.is_empty() {
//         output.info("no packages found");
//     }

//     Packages {
//         _context: Some(LD_CONTEXT.to_owned()),
//         _type: ld_type!("Packages"),
//         _id: Some("".to_owned()),
//         base: format!("{}packages/", &repo.base),
//         channel: channel.unwrap_or(&repo.default_channel).to_string(),
//         packages: map,
//     }
// }

// fn write_repo_index_packages<T: ProgressOutput>(
//     cur_dir: &Path,
//     repo: &Repository,
//     index: &Packages,
//     channel: Option<&str>,
//     output: &T,
// ) {
//     write_index(cur_dir, repo, index, channel, output, "packages");
// }

// fn generate_repo_index_virtuals<T: ProgressOutput>(
//     cur_dir: &Path,
//     repo: &Repository,
//     channel: Option<&str>,
//     output: &T,
// ) -> Virtuals {
//     output.generating(&format!(
//         "{} virtuals index",
//         channel.unwrap_or(&repo.default_channel)
//     ));

//     let virtuals_path = cur_dir.join("virtuals");
//     let virtuals: Vec<Virtual> = fs::read_dir(&virtuals_path)
//         .unwrap()
//         .map(|x| x.unwrap().path())
//         .filter_map(|path| {
//             if !path.is_dir() {
//                 if let Some(ex) = path.extension() {
//                     if ex == "json" {
//                         return None;
//                     }
//                 }

//                 let relpath = pathdiff::diff_paths(&*path, cur_dir).unwrap();
//                 output.warn(&format!("{:?} is not a directory; skipping", &relpath));
//                 return None;
//             }

//             if !path.join(index_fn(channel)).exists() {
//                 if channel.is_none() {
//                     let relpath = pathdiff::diff_paths(&*path, cur_dir).unwrap();
//                     output.warn(&format!(
//                         "{:?} does not contain {:?}; skipping",
//                         &relpath,
//                         index_fn(channel)
//                     ));
//                 }
//                 return None;
//             }

//             let index_path = path.join(index_fn(channel));
//             let file = File::open(&index_path).unwrap();
//             let virtual_index: Virtual = match serde_json::from_reader(file) {
//                 Ok(x) => x,
//                 Err(err) => {
//                     let relpath = pathdiff::diff_paths(&*index_path, cur_dir).unwrap();
//                     output.error(&format!("Error parsing path {:?}:", &relpath));
//                     output.error(&format!("{}", err));
//                     return None;
//                 }
//             };

//             output.inserting(&virtual_index.id, &virtual_index.version);
//             Some(virtual_index)
//         })
//         .collect();

//     let mut map = BTreeMap::new();
//     for virtual_ in virtuals.into_iter() {
//         map.insert(virtual_.id.to_owned(), virtual_);
//     }

//     if map.is_empty() {
//         output.info("no virtuals found");
//     }

//     Virtuals {
//         _context: Some(LD_CONTEXT.to_owned()),
//         _type: ld_type!("Virtuals"),
//         _id: Some("".to_owned()),
//         base: format!("{}virtuals/", &repo.base),
//         channel: channel.unwrap_or(&repo.default_channel).to_string(),
//         virtuals: map,
//     }
// }

// fn write_repo_index_virtuals<T: ProgressOutput>(
//     cur_dir: &Path,
//     repo: &Repository,
//     index: &Virtuals,
//     channel: Option<&str>,
//     output: &T,
// ) {
//     write_index(cur_dir, repo, index, channel, output, "virtuals");
// }

// pub fn index_fn(channel: Option<&str>) -> String {
//     match channel {
//         Some(v) => format!("index.{}.json", v),
//         None => "index.json".into(),
//     }
// }
