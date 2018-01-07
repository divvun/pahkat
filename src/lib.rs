extern crate bahkat;
extern crate rusqlite;
extern crate reqwest;
extern crate serde_json;
extern crate serde;
extern crate semver;
extern crate xz2;
extern crate tar;
extern crate tempdir;
extern crate url;

use bahkat::types::*;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use xz2::read::XzDecoder;
use std::fs::{remove_file, read_dir, remove_dir, File};
use std::cell::RefCell;

pub enum PackageStatus {
    NotInstalled,
    UpToDate,
    RequiresUpdate
}

pub enum PackageStatusError {
    NoInstaller,
    ParsingVersion
}

enum PackageAction<'a> {
    Install(&'a Package),
    Uninstall(&'a Package)
}

pub trait PackageStore<'a> {
    type InstallResult;
    type UninstallResult;
    type StatusResult;

    fn install(&self, package: &'a Package, path: &'a Path) -> Self::InstallResult;
    fn uninstall(&self, package: &'a Package) -> Self::UninstallResult;
    fn status(&self, package: &'a Package) -> Self::StatusResult;
}

#[derive(Debug)]
struct PackageRecord {
    id: String,
    version: String,
    files: Vec<PathBuf>,
    dependencies: Vec<String>
}

impl PackageRecord {
    fn sql_dependencies(conn: &rusqlite::Connection, id: &str) -> Vec<String> {
        let mut stmt = conn.prepare("SELECT dependency_id FROM packages_dependencies WHERE package_id = ?")
            .unwrap();

        let res = stmt
            .query_map(&[&id], |row| row.get(0))
            .unwrap()
            .map(|x| x.unwrap()).collect();

        res
    }

    fn sql_files(conn: &rusqlite::Connection, id: &str) -> Vec<PathBuf> {
        let mut stmt = conn.prepare("SELECT file_path FROM packages_files WHERE package_id = ?")
            .unwrap();

        let res = stmt
            .query_map(&[&id], |row| row.get(0))
            .unwrap()
            .map(|x: Result<String, _>| Path::new(&x.unwrap()).to_path_buf())
            .collect();

        res
    }

    fn sql_version(conn: &rusqlite::Connection, id: &str) -> Option<String> {
        match conn.query_row("SELECT version FROM packages WHERE id = ? LIMIT 1", &[&id], |row| row.get(0)) {
            Ok(v) => v,
            Err(_) => return None
        }
    }

    fn sql_replace_pkg(&self, tx: &rusqlite::Transaction) -> rusqlite::Result<()> {
        tx.execute("REPLACE INTO packages(id, version) VALUES (?, ?)", &[&self.id, &self.version])?;
        tx.execute("DELETE FROM packages_dependencies WHERE package_id = ?", &[&self.id])?;
        tx.execute("DELETE FROM packages_files WHERE package_id = ?", &[&self.id])?;
        
        let mut dep_stmt = tx.prepare("INSERT INTO packages_dependencies(package_id, dependency_id) VALUES (?, ?)")?;
        for id in &self.dependencies {
            dep_stmt.execute(&[&self.id, &*id])?;
        }

        let mut file_stmt = tx.prepare("INSERT INTO packages_files(package_id, file_path) VALUES (?, ?)")?;
        for file_path in &self.files {
            file_stmt.execute(&[&self.id, &file_path.to_str()])?;
        }

        Ok(())
    }

    fn sql_remove_pkg(&self, tx: &rusqlite::Transaction) -> rusqlite::Result<()> {
        tx.execute("DELETE FROM packages WHERE id = ?", &[&self.id])?;
        tx.execute("DELETE FROM packages_dependencies WHERE package_id = ?", &[&self.id])?;
        tx.execute("DELETE FROM packages_files WHERE package_id = ?", &[&self.id])?;

        Ok(())
    }

    pub fn find_by_id(conn: &rusqlite::Connection, id: &str) -> Option<PackageRecord> {
        let version = match Self::sql_version(conn, id) {
            Some(v) => v,
            None => return None
        };
        
        let files = Self::sql_files(conn, id);
        let dependencies = Self::sql_dependencies(conn, id);

        Some(PackageRecord {
            id: id.to_owned(),
            version: version,
            files: files,
            dependencies: dependencies
        })
    }

    pub fn save(&self, tx: rusqlite::Transaction) -> rusqlite::Result<()> {
        self.sql_replace_pkg(&tx)?;
        tx.commit()
    }

    pub fn delete(self, tx: rusqlite::Transaction) -> rusqlite::Result<()> {
        self.sql_remove_pkg(&tx)?;
        tx.commit()
    }
}

#[test]
fn test_create_database() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let store = TarballPackageStore::new(conn, Path::new("/"));
    store.init_sqlite_database().unwrap();
}

#[test]
fn test_create_record() {
    let mut conn = {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        let store = TarballPackageStore::new(conn, Path::new("/"));
        store.init_sqlite_database().unwrap();
        store.conn.into_inner()
    };

    let pkg = PackageRecord {
        id: "test-pkg".to_owned(),
        version: "1.0.0".to_owned(),
        files: vec![Path::new("bin/test").to_path_buf()],
        dependencies: vec!["one-thing".to_owned()]
    };

    {
        pkg.save(conn.transaction().unwrap()).unwrap();
    }

    let found = PackageRecord::find_by_id(&mut conn, "test-pkg");
    assert!(found.is_some());
}

#[test]
fn test_download_repo() {
    let repo = download_repository("http://localhost:8000").unwrap();
    assert_eq!(repo.meta.base, "localhost");
}

#[test]
fn test_extract_files() {
    let tmpdir = Path::new("/tmp");
    let conn = rusqlite::Connection::open("./testextract.sqlite").unwrap();

    let pkgstore = TarballPackageStore::new(conn, tmpdir);
    pkgstore.init_sqlite_database();
    let repo = Repository::from_url("http://localhost:8000").unwrap();

    let test_pkg = repo.package("test-pkg").unwrap();
    let inst_path = test_pkg.download(tmpdir).expect("a download");
    pkgstore.install(test_pkg, &inst_path).unwrap();
    pkgstore.uninstall(test_pkg).unwrap();
}

pub struct Repository {
    meta: RepositoryMeta,
    packages: Packages,
    virtuals: Virtuals
}

impl Repository {
    pub fn from_url(url: &str) -> Result<Repository, RepoDownloadError> {
        download_repository(url)
    }

    pub fn package(&self, key: &str) -> Option<&Package> {
        let map = &self.packages.packages;
        map.get(key)
    }

    pub fn packages(&self) -> &PackageMap {
        &self.packages.packages
    }

    pub fn virtuals(&self) -> &VirtualRefMap {
        &self.virtuals.virtuals
    }
}

trait Download {
    fn download(&self, dir_path: &Path) -> Option<PathBuf>;
}

impl Download for Package {
    fn download(&self, dir_path: &Path) -> Option<PathBuf> {
        let url_str = match self.installer() {
            Some(&Installer::Windows(ref installer)) => &installer.url,
            Some(&Installer::Tarball(ref installer)) => &installer.url,
            None => return None
        };

        let url = url::Url::parse(&url_str).unwrap();
        let mut res = reqwest::get(url_str).unwrap();
        let tmppath = dir_path.join(&url.path_segments().unwrap().last().unwrap()).to_path_buf();
        let file = File::create(&tmppath).unwrap();
        
        let mut writer = BufWriter::new(file);
        if res.copy_to(&mut writer).unwrap() == 0 {
            return None;
        }

        Some(tmppath)
    }
}

#[derive(Debug)]
pub enum RepoDownloadError {
    ReqwestError(reqwest::Error),
    JsonError(serde_json::Error)
}

fn download_repository(url: &str) -> Result<Repository, RepoDownloadError> {
    let client = reqwest::Client::new();

    let mut meta_res = client.get(&format!("{}/index.json", url)).send()
        .map_err(|e| RepoDownloadError::ReqwestError(e))?;
    let meta_text = meta_res.text()
        .map_err(|e| RepoDownloadError::ReqwestError(e))?;
    let meta: RepositoryMeta = serde_json::from_str(&meta_text)
        .map_err(|e| RepoDownloadError::JsonError(e))?;

    let mut pkg_res = client.get(&format!("{}/packages/index.json", url)).send()
        .map_err(|e| RepoDownloadError::ReqwestError(e))?;
    let pkg_text = pkg_res.text()
        .map_err(|e| RepoDownloadError::ReqwestError(e))?;
    let packages: Packages = serde_json::from_str(&pkg_text)
        .map_err(|e| RepoDownloadError::JsonError(e))?;

    let mut virt_res = client.get(&format!("{}/virtuals/index.json", url)).send()
        .map_err(|e| RepoDownloadError::ReqwestError(e))?;
    let virt_text = virt_res.text()
        .map_err(|e| RepoDownloadError::ReqwestError(e))?;
    let virtuals: Virtuals = serde_json::from_str(&virt_text)
        .map_err(|e| RepoDownloadError::JsonError(e))?;

    Ok(Repository {
        meta: meta,
        packages: packages,
        virtuals: virtuals
    })
}

struct TarballPackageStore<'a> {
    conn: RefCell<rusqlite::Connection>,
    prefix: &'a Path
}

impl<'a> TarballPackageStore<'a> {
    fn init_sqlite_database(&self) -> rusqlite::Result<()> {
        self.conn.borrow().execute_batch(include_str!("./pkgstore_init.sql"))
    }

    pub fn new(conn: rusqlite::Connection, prefix: &'a Path) -> TarballPackageStore<'a> {
        TarballPackageStore {
            conn: RefCell::new(conn),
            prefix: prefix
        }
    }
}

impl<'a> PackageStore<'a> for TarballPackageStore<'a> {
    type StatusResult = Result<PackageStatus, PackageStatusError>;
    type InstallResult = Result<PackageAction<'a>, ()>;
    type UninstallResult = Result<PackageAction<'a>, ()>;

    fn install(&self, package: &'a Package, path: &'a Path) -> Self::InstallResult {
        let installer = match package.installer() {
            None => return Err(()),
            Some(v) => v
        };

        let tarball = match installer {
            &Installer::Windows(_) => return Err(()),
            &Installer::Tarball(ref v) => v
        };

        let ext = match path.extension().and_then(OsStr::to_str) {
            None => return Err(()),
            Some(v) => v
        };
        
        let file = File::open(path).unwrap();

        let reader = match ext {
            "txz" | "xz" => XzDecoder::new(file),
            // "tgz" | "gz" => (),
            _ => return Err(())
        };
        
        let mut tar_file = tar::Archive::new(reader);
        let mut files: Vec<PathBuf> = vec![];

        for entry in tar_file.entries().unwrap() {
            let mut entry = entry.unwrap();
            let unpack_res;
            {
                unpack_res = entry.unpack_in(self.prefix);
            }

            match unpack_res {
                Ok(true) => {
                    let entry_path = entry.header().path().unwrap();
                    files.push(entry_path.to_path_buf());
                },
                Ok(false) => continue,
                Err(_) => return Err(())
            }
        }

        let deps = &package.dependencies;
        let dependencies: Vec<String> = deps.keys().map(|x| x.to_owned()).collect();
        
        {
            let record = PackageRecord {
                id: package.id.to_owned(),
                version: package.version.to_owned(),
                files: files,
                dependencies: dependencies
            };

            let mut conn = self.conn.borrow_mut();
            let tx = conn.transaction().unwrap();
            record.save(tx).unwrap();
        }

        Ok(PackageAction::Install(package))
    }

    fn uninstall(&self, package: &'a Package) -> Self::UninstallResult {
        let record = {
            let conn = self.conn.borrow();

            match PackageRecord::find_by_id(&conn, &package.id) {
                None => return Err(()),
                Some(v) => v
            }
        };

        for file in &record.files {
            if file.is_dir() {
                continue;
            }

            if file.exists() {
                remove_file(file).unwrap();
            }
        }

        for file in &record.files {
            if !file.is_dir() {
                continue
            }

            if read_dir(file).iter().next().is_none() {
                remove_dir(file).unwrap();
            }
        }

        let mut conn = self.conn.borrow_mut();
        let tx = conn.transaction().unwrap();
        record.delete(tx).unwrap();

        Ok(PackageAction::Uninstall(package))
    }

    fn status(&self, package: &'a Package) -> Self::StatusResult {
        let installed_pkg = match PackageRecord::find_by_id(&self.conn.borrow(), &package.id) {
            None => return Ok(PackageStatus::NotInstalled),
            Some(v) => v
        };

        let installed_version = match semver::Version::parse(&installed_pkg.version) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v
        };
        
        let candidate_version = match semver::Version::parse(&package.version) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v
        };

        if candidate_version > installed_version {
            Ok(PackageStatus::RequiresUpdate)
        } else {
            Ok(PackageStatus::UpToDate)
        }
    }
}
