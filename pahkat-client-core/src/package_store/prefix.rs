#![cfg(feature = "prefix")]

use std::collections::BTreeMap;
use std::fs::{create_dir_all, read_dir, remove_dir, remove_file, File};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use hashbrown::HashMap;
use indexmap::IndexMap;
use pahkat_types::package::{Descriptor, Package};
use r2d2_sqlite::SqliteConnectionManager;
use url::Url;
use xz2::read::XzDecoder;

use crate::transaction::{
    install::InstallError, uninstall::UninstallError, PackageDependencyError,
};
use crate::{
    cmp, download::Download, download::DownloadManager, package_store::ImportError,
    repo::LoadedRepository, transaction::PackageStatus, transaction::PackageStatusError, Config,
    PackageKey, PackageStore,
};

pub struct PrefixPackageStore {
    pool: r2d2::Pool<SqliteConnectionManager>,
    prefix: PathBuf,
    repos: Arc<RwLock<HashMap<Url, LoadedRepository>>>,
    config: Arc<RwLock<Config>>,
}

impl PrefixPackageStore {
    pub fn create<P: AsRef<Path>>(
        prefix_path: P,
    ) -> Result<PrefixPackageStore, Box<dyn std::error::Error>> {
        // let prefix_path: &Path = prefix_path.as_ref();

        // create_dir_all(prefix_path)?;
        // create_dir_all(prefix_path.join("pkg"))?;

        // let config = StoreConfig::new(prefix_path);
        // config.save()?;

        // let db_file_path = PrefixPackageStore::package_db_path(&config);
        // let manager = SqliteConnectionManager::file(&db_file_path);
        // let pool = Self::make_pool(manager)?;
        // let conn = pool.get()?;
        // conn.execute_batch(PKG_STORE_INIT)?;

        // let store = PrefixPackageStore {
        //     pool,
        //     prefix: prefix_path.to_owned(),
        //     repos: Arc::new(RwLock::new(HashMap::new())),
        //     config: Arc::new(RwLock::new(config)),
        // };

        // store.refresh_repos();

        // Ok(store)
        todo!()
    }

    pub fn open<P: AsRef<Path>>(
        prefix_path: P,
    ) -> Result<PrefixPackageStore, Box<dyn std::error::Error>> {
        // let prefix_path = prefix_path.as_ref().canonicalize()?;
        // log::debug!("{:?}", &prefix_path);
        // let config = StoreConfig::load(&prefix_path.join("config.json"), true)?;

        // let db_file_path = PrefixPackageStore::package_db_path(&config);
        // log::debug!("{:?}", &db_file_path);
        // let manager = SqliteConnectionManager::file(&db_file_path);
        // let pool = Self::make_pool(manager)?;

        // let store = PrefixPackageStore {
        //     pool,
        //     prefix: prefix_path,
        //     repos: Arc::new(RwLock::new(HashMap::new())),
        //     config: Arc::new(RwLock::new(config)),
        // };

        // store.refresh_repos();

        // Ok(store)
        todo!()
    }

    #[inline(always)]
    fn make_pool(
        manager: SqliteConnectionManager,
    ) -> Result<r2d2::Pool<SqliteConnectionManager>, r2d2::Error> {
        r2d2::Pool::builder()
            .max_size(4)
            .min_idle(Some(0))
            .idle_timeout(Some(std::time::Duration::new(10, 0)))
            .build(manager)
    }

    fn package_db_path(config: &Config) -> PathBuf {
        config.settings().config_dir().join("packages.sqlite")
    }

    pub(crate) fn into_arc(self) -> Arc<dyn PackageStore<Target = ()>> {
        Arc::new(self)
    }

    fn package_path(&self, package: &Descriptor) -> PathBuf {
        self.prefix.join("pkg").join(&package.id)
    }
}

impl PackageStore for PrefixPackageStore {
    type Target = ();

    fn find_package_by_key(&self, key: &PackageKey) -> Option<Package> {
        let repos = self.repos.read().unwrap();
        crate::repo::find_package_by_key(key, &*repos)
    }

    fn repos(&self) -> super::SharedRepos {
        Arc::clone(&self.repos)
    }

    fn config(&self) -> super::SharedStoreConfig {
        Arc::clone(&self.config)
    }

    fn import(&self, key: &PackageKey, installer_path: &Path) -> Result<PathBuf, ImportError> {
        // let package = match self.find_package_by_key(key) {
        //     Some(v) => v,
        //     None => {
        //         return Err(Box::new(crate::download::DownloadError::NoUrl) as _);
        //     }
        // };

        // let installer = match package.installer() {
        //     None => return Err(Box::new(crate::download::DownloadError::NoUrl) as _),
        //     Some(v) => v,
        // };

        // let config = &self.config.read().unwrap();
        // let installer_url = installer.url();
        // let output_path = crate::repo::download_dir(config, &installer_url);
        // println!("{:?}", output_path);

        // std::fs::create_dir_all(&output_path).unwrap();
        // let url = url::Url::parse(&installer_url).map_error(|| {
        //     crate::transaction::install::InvalidUrl {
        //         url: installer_url.to_owned(),
        //     }
        // })?;
        // let filename = url.path_segments().unwrap().last().unwrap();
        // let output_file = output_path.join(filename);

        // std::fs::copy(installer_path, &output_file)?;
        // Ok(output_file)
        todo!()
    }

    fn download(
        &self,
        key: &PackageKey,
        progress: Box<dyn Fn(u64, u64) -> bool + Send + 'static>,
    ) -> Result<PathBuf, crate::download::DownloadError> {
        // let package = match self.find_package_by_key(key) {
        //     Some(v) => v,
        //     None => {
        //         return Err(crate::download::DownloadError::NoUrl);
        //     }
        // };

        // let installer = match package.installer() {
        //     None => return Err(crate::download::DownloadError::NoUrl),
        //     Some(v) => v,
        // };

        // let url = match Url::parse(&*installer.url()) {
        //     Ok(v) => v,
        //     Err(e) => return Err(crate::download::DownloadError::InvalidUrl),
        // };

        // let config = &self.config.read().unwrap();
        // let dm = DownloadManager::new(
        //     config.download_cache_dir(),
        //     config.max_concurrent_downloads(),
        // );

        // let output_path = crate::repo::download_dir(config, &installer.url());
        // crate::block_on(dm.download(&url, output_path, Some(progress)))
        todo!()
    }

    fn install(
        &self,
        key: &PackageKey,
        target: &Self::Target,
    ) -> Result<PackageStatus, InstallError> {
        // let package = self
        //     .find_package_by_key(key)
        //     .with_context(|| crate::transaction::install::NoPackage)?;
        // let installer = package
        //     .installer()
        //     .with_context(|| crate::transaction::install::NoInstaller)?;
        // let installer = match *installer {
        //     Installer::Tarball(ref v) => v,
        //     _ => return Err(crate::transaction::install::InstallError::WrongInstallerType),
        // };
        // let url = url::Url::parse(&installer.url).with_context(|| {
        //     crate::transaction::install::InvalidUrl {
        //         url: installer.url.to_owned(),
        //     }
        // })?;
        // let filename = url.path_segments().unwrap().last().unwrap();
        // let pkg_path =
        //     crate::repo::download_dir(&*self.config.read().unwrap(), &url.as_str()).join(filename);

        // if !pkg_path.exists() {
        //     log::error!("Package path doesn't exist: {:?}", &pkg_path);
        //     return Err(crate::transaction::install::InstallError::PackageNotInCache);
        // }

        // let ext = pkg_path
        //     .extension()
        //     .and_then(OsStr::to_str)
        //     .ok_or(InstallError::InvalidFileType)?;

        // let file = File::open(&pkg_path).unwrap();

        // let reader = match ext {
        //     "txz" | "xz" => XzDecoder::new(file),
        //     _ => return Err(InstallError::InvalidFileType),
        // };

        // let mut tar_file = tar::Archive::new(reader);
        // let mut files: Vec<PathBuf> = vec![];

        // let pkg_path = self.package_path(&package);
        // create_dir_all(&pkg_path).unwrap(); //.context(CreateDirFailed)?;

        // for entry in tar_file.entries().unwrap() {
        //     let mut entry = entry.unwrap();
        //     let unpack_res;
        //     {
        //         unpack_res = entry.unpack_in(&pkg_path).unwrap(); //.context(UnpackFailed)?;
        //     }

        //     if unpack_res {
        //         let entry_path = entry.header().path().unwrap();
        //         files.push(entry_path.to_path_buf());
        //     } else {
        //         continue;
        //     }
        // }

        // let deps = &package.dependencies;
        // let dependencies: Vec<String> = deps.keys().map(|x| x.to_owned()).collect();

        // {
        //     let record = PackageDbRecord {
        //         id: 0,
        //         url: key.to_string(),
        //         version: package.version.to_owned(),
        //         files,
        //         dependencies,
        //     };

        //     let mut conn = self.pool.get().unwrap();
        //     record.save(&mut conn).unwrap();
        // };

        // Ok(PackageStatus::UpToDate)
        todo!()
    }

    fn uninstall(
        &self,
        key: &PackageKey,
        target: &Self::Target,
    ) -> Result<PackageStatus, UninstallError> {
        // let package = self
        //     .find_package_by_key(key)
        //     .ok_or(UninstallError::NoPackage)?;

        // let mut conn = self.pool.get().unwrap();
        // let record = match PackageDbRecord::find_by_id(&mut conn, &key) {
        //     None => return Err(UninstallError::NoPackage),
        //     Some(v) => v,
        // };

        // let pkg_path = self.package_path(&package);
        // for file in &record.files {
        //     let file = match pkg_path.join(file).canonicalize() {
        //         Ok(v) => v,
        //         Err(_) => continue,
        //     };

        //     if file.is_dir() {
        //         continue;
        //     }

        //     if file.exists() {
        //         remove_file(file).unwrap();
        //     }
        // }

        // for file in &record.files {
        //     let file = match pkg_path.join(file).canonicalize() {
        //         Ok(v) => v,
        //         Err(_) => continue,
        //     };

        //     if !file.is_dir() {
        //         continue;
        //     }

        //     let dir = read_dir(&file).unwrap();
        //     if dir.count() == 0 {
        //         remove_dir(&file).unwrap();
        //     }
        // }

        // record.delete(&mut conn).unwrap();

        // Ok(PackageStatus::NotInstalled)
        todo!()
    }

    fn status(&self, key: &PackageKey, _target: &()) -> Result<PackageStatus, PackageStatusError> {
        // log::debug!("status: {}", &key.to_string());

        // let package = match self.find_package_by_key(key) {
        //     Some(v) => v,
        //     None => {
        //         return Err(PackageStatusError::NoPackage);
        //     }
        // };

        // let installer = match package.installer() {
        //     None => return Err(PackageStatusError::NoInstaller),
        //     Some(v) => v,
        // };

        // let installer = match installer {
        //     &Installer::Tarball(ref v) => v,
        //     _ => return Err(PackageStatusError::WrongInstallerType),
        // };

        // let mut conn = self.pool.get().unwrap();
        // let record = match PackageDbRecord::find_by_id(&mut conn, &key) {
        //     None => return Ok(PackageStatus::NotInstalled),
        //     Some(v) => v,
        // };

        // let config = self.config.read().unwrap();

        // let skipped_package = config.skipped_package(key);
        // let skipped_package = skipped_package.as_ref().map(String::as_ref);

        // let status = self::cmp::cmp(&record.version, &package.version, skipped_package);

        // log::debug!("Status: {:?}", &status);
        // status
        todo!()
    }

    fn all_statuses(
        &self,
        repo_url: &Url,
        target: &(),
    ) -> BTreeMap<String, Result<PackageStatus, PackageStatusError>> {
        // crate::repo::all_statuses(self, repo_record, target)
        todo!()
    }

    fn find_package_by_id(&self, package_id: &str) -> Option<(PackageKey, Package)> {
        // crate::repo::find_package_by_id(self, package_id, &self.repos)
        todo!()
    }

    fn refresh_repos(&self) {
        // let config = self.config.read().unwrap();
        // let repos = crate::repo::refresh_repos(&config);
        // *self.repos.write().unwrap() = repos;
        todo!()
    }

    fn clear_cache(&self) {
        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("No installer found for package: {id}")]
    NoInstaller { id: String },

    #[error("Wrong installer type")]
    WrongInstallerType,

    #[error("Invalid extension")]
    InvalidExtension,

    #[error("Create directory failed")]
    CreateDirFailed(#[source] std::io::Error),

    #[error("Unpack failed")]
    UnpackFailed(#[source] std::io::Error),
}

type Result<T, E = Error> = std::result::Result<T, E>;

static PKG_STORE_INIT: &'static str = include_str!("../pkgstore_init.sql");

#[derive(Debug)]
struct PackageDbRecord {
    id: i64,
    url: String,
    version: String,
    files: Vec<Url>,
    dependencies: Vec<String>,
}

struct PackageDbConnection<'a>(&'a mut rusqlite::Connection);

impl<'a> PackageDbConnection<'a> {
    fn dependencies(&self, url: &str) -> Vec<String> {
        let mut stmt = self
            .0
            .prepare("SELECT dependency_id FROM packages_dependencies WHERE package_id = (SELECT id FROM packages WHERE url = ?)")
            .unwrap();

        let res = stmt
            .query_map(&[&url], |row| row.get(0))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();

        res
    }

    fn files(&self, url: &str) -> Vec<Url> {
        let mut stmt = self
            .0
            .prepare("SELECT file_url FROM packages_files WHERE package_id = (SELECT id FROM packages WHERE url = ?)")
            .expect("prepared statement");

        let res = stmt
            .query_map(&[&url], |row| row.get(0))
            .expect("query_map succeeds")
            .map(|x: Result<String, _>| Url::parse(&x.unwrap()).unwrap())
            .collect();

        res
    }

    fn version(&self, url: &str) -> Option<String> {
        match self.0.query_row(
            "SELECT version FROM packages WHERE url = ? LIMIT 1",
            &[&url],
            |row| row.get(0),
        ) {
            Ok(v) => v,
            Err(_) => return None,
        }
    }

    fn replace_pkg(&mut self, pkg: &PackageDbRecord) -> rusqlite::Result<()> {
        let tx = self.0.transaction().unwrap();

        tx.execute_named(
            "REPLACE INTO packages(id, url, version) VALUES (:id, :url, :version)",
            &[
                (":id", &pkg.id),
                (":url", &pkg.url),
                (":version", &pkg.version),
            ],
        )
        .unwrap();
        tx.execute(
            "DELETE FROM packages_dependencies WHERE package_id = ?",
            &[&pkg.id],
        )
        .unwrap();
        tx.execute(
            "DELETE FROM packages_files WHERE package_id = ?",
            &[&pkg.id],
        )
        .unwrap();

        {
            let mut dep_stmt = tx
                .prepare(
                    "INSERT INTO packages_dependencies(package_id, dependency_id) VALUES (:id, (SELECT id FROM packages WHERE url = :dep_url))",
                )
                .unwrap();
            for dep_url in &pkg.dependencies {
                dep_stmt.execute_named(&[(":id", &pkg.id), (":dep_url", &*dep_url)])?;
            }

            let mut file_stmt =
                tx.prepare("INSERT INTO packages_files(package_id, file_url) VALUES (:id, :path)")?;

            for file_path in &pkg.files {
                file_stmt
                    .execute_named(&[(":id", &pkg.id), (":path", &file_path.as_str())])
                    .unwrap();
            }
        }

        tx.commit()
    }

    fn remove_pkg(&mut self, pkg: &PackageDbRecord) -> rusqlite::Result<()> {
        let tx = self.0.transaction().unwrap();

        tx.execute("DELETE FROM packages WHERE id = ?", &[&pkg.id])?;
        tx.execute(
            "DELETE FROM packages_dependencies WHERE package_id = ?",
            &[&pkg.id],
        )?;
        tx.execute(
            "DELETE FROM packages_files WHERE package_id = ?",
            &[&pkg.id],
        )?;

        tx.commit()
    }
}

impl PackageDbRecord {
    pub fn find_by_id(
        conn: &mut rusqlite::Connection,
        key: &PackageKey,
    ) -> Option<PackageDbRecord> {
        let conn = PackageDbConnection(conn);
        let url = key.to_string();

        let version = match conn.version(&url) {
            Some(v) => v,
            None => return None,
        };

        let files = conn.files(&url);
        let dependencies = conn.dependencies(&url);

        Some(PackageDbRecord {
            id: 0,
            url,
            version,
            files,
            dependencies,
        })
    }

    pub fn save(&self, conn: &mut rusqlite::Connection) -> rusqlite::Result<()> {
        PackageDbConnection(conn).replace_pkg(self)
    }

    pub fn delete(self, conn: &mut rusqlite::Connection) -> rusqlite::Result<()> {
        PackageDbConnection(conn).remove_pkg(&self)
    }
}

// #[test]
// fn test_create_database() {
//     let conn = rusqlite::Connection::open_in_memory().unwrap();
//     let store = TarballPackageStore::new(conn, Path::new("/"));
//     store.init_sqlite_database().unwrap();
// }

// #[test]
// fn test_create_record() {
//     let mut conn = {
//         let mut conn = rusqlite::Connection::open_in_memory().unwrap();
//         let store = TarballPackageStore::new(conn, Path::new("/"));
//         store.init_sqlite_database().unwrap();
//         store.conn.into_inner()
//     };

//     let pkg = PackageDbRecord {
//         id: "test-pkg".to_owned(),
//         version: "1.0.0".to_owned(),
//         files: vec![Path::new("bin/test").to_path_buf()],
//         dependencies: vec!["one-thing".to_owned()]
//     };

//     {
//         pkg.save(conn.transaction().unwrap()).unwrap();
//     }

//     let found = PackageDbRecord::find_by_id(&mut conn, "test-pkg");
//     assert!(found.is_some());
// }

// #[test]
// fn test_download_repo() {
//     let repo = download_repository("http://localhost:8000").unwrap();
//     assert_eq!(repo.meta.base, "localhost");
// }

// #[test]
// fn test_extract_files() {
//     let tmpdir = Path::new("/tmp");
//     let conn = rusqlite::Connection::open_in_memory().unwrap();

//     let pkgstore = TarballPackageStore::new(conn, tmpdir);
//     pkgstore.init_sqlite_database();
//     let repo = Repository::from_url("http://localhost:8000").unwrap();

//     let test_pkg = repo.package("test-pkg").unwrap();
//     let inst_path = test_pkg.download(&tmpdir).expect("a download");
//     pkgstore.install(test_pkg, &inst_path).unwrap();
//     pkgstore.uninstall(test_pkg).unwrap();
// }
