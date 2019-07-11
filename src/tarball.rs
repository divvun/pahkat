#![cfg(feature = "prefix")]

use hashbrown::HashMap;
use pahkat_types::*;
use r2d2_sqlite::SqliteConnectionManager;
use snafu::{ensure, Backtrace, ErrorCompat, OptionExt, ResultExt, Snafu};
use std::cell::RefCell;
use std::ffi::OsStr;
use std::fs::{create_dir_all, read_dir, remove_dir, remove_file, File};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use xz2::read::XzDecoder;

use crate::transaction::PackageTransaction;
use crate::{
    cmp, download::Download, repo::Repository, AbsolutePackageKey, PackageDependency,
    PackageStatus, PackageStatusError, RepoRecord, StoreConfig,
};

use crate::transaction::{
    install::InstallError, install::ProcessError, uninstall::UninstallError, PackageActionType,
    PackageDependencyError, PackageStore,
};

pub struct TarballPackageStore {
    pool: r2d2::Pool<SqliteConnectionManager>,
    prefix: PathBuf,
}

pub struct PrefixPackageStore {
    pool: r2d2::Pool<SqliteConnectionManager>,
    prefix: PathBuf,
    repos: Arc<RwLock<HashMap<RepoRecord, Repository>>>,
    config: Arc<RwLock<StoreConfig>>,
}

impl PrefixPackageStore {
    pub fn create<P: AsRef<Path>>(
        prefix_path: P,
    ) -> Result<PrefixPackageStore, Box<dyn std::error::Error>> {
        let prefix_path: &Path = prefix_path.as_ref();

        create_dir_all(prefix_path)?;
        create_dir_all(prefix_path.join("pkg"))?;

        let config = StoreConfig::new(prefix_path);
        config.save()?;

        let db_file_path = PrefixPackageStore::package_db_path(&config);
        let manager = SqliteConnectionManager::file(&db_file_path);
        let pool = r2d2::Pool::new(manager)?;
        let conn = pool.get()?;
        conn.execute_batch(PKG_STORE_INIT)?;

        let store = PrefixPackageStore {
            pool,
            prefix: prefix_path.to_owned(),
            repos: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(config)),
        };

        store.refresh_repos();

        Ok(store)
    }

    pub fn open<P: AsRef<Path>>(
        prefix_path: P,
    ) -> Result<PrefixPackageStore, Box<dyn std::error::Error>> {
        let prefix_path = prefix_path.as_ref().canonicalize()?;
        println!("{:?}", &prefix_path);
        let config = StoreConfig::load(&prefix_path.join("config.json"), true)?;

        let db_file_path = PrefixPackageStore::package_db_path(&config);
        println!("{:?}", &db_file_path);
        let manager = SqliteConnectionManager::file(&db_file_path);
        let pool = r2d2::Pool::new(manager)?;

        let store = PrefixPackageStore {
            pool,
            prefix: prefix_path,
            repos: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(config)),
        };

        store.refresh_repos();

        Ok(store)
    }

    // TODO: unsure if we want this to exist at all
    pub fn config(&self) -> Arc<RwLock<StoreConfig>> {
        Arc::clone(&self.config)
    }

    fn package_db_path(config: &StoreConfig) -> PathBuf {
        config.config_dir().join("packages.sqlite")
    }

    pub fn into_arc(self) -> Arc<dyn PackageStore<Target = ()>> {
        Arc::new(self)
    }

    pub fn find_package_by_id(&self, package_id: &str) -> Option<(AbsolutePackageKey, Package)> {
        crate::repo::find_package_by_id(&*self.repos.read().unwrap(), package_id)
    }

    fn package_path(&self, package: &Package) -> PathBuf {
        self.prefix.join("pkg").join(&package.id)
    }

    pub fn refresh_repos(&self) {
        *self.repos.write().unwrap() = crate::repo::refresh_repos(&*self.config.read().unwrap());
    }
}

impl PackageStore for PrefixPackageStore {
    type Target = ();

    fn resolve_package(&self, key: &AbsolutePackageKey) -> Option<Package> {
        crate::repo::resolve_package(key, &self.repos)
    }

    fn download(
        &self,
        key: &AbsolutePackageKey,
        progress: Box<dyn Fn(u64, u64) -> () + Send + 'static>,
    ) -> Result<PathBuf, crate::download::DownloadError> {
        let package = match self.resolve_package(key) {
            Some(v) => v,
            None => {
                return Err(crate::download::DownloadError::NoUrl);
            }
        };

        let installer = match package.installer() {
            None => return Err(crate::download::DownloadError::NoUrl),
            Some(v) => v,
        };

        let disposable = package.download(
            &crate::repo::download_path(&*self.config.read().unwrap(), &installer.url()),
            Some(progress),
        )?;
        disposable.wait()
    }

    fn install(
        &self,
        key: &AbsolutePackageKey,
        target: &Self::Target,
    ) -> Result<PackageStatus, InstallError> {
        let package = self
            .resolve_package(key)
            .with_context(|| crate::transaction::install::NoPackage)?;
        let installer = package
            .installer()
            .with_context(|| crate::transaction::install::NoInstaller)?;
        let installer = match *installer {
            Installer::Tarball(ref v) => v,
            _ => return Err(crate::transaction::install::InstallError::WrongInstallerType),
        };
        let url = url::Url::parse(&installer.url).with_context(|| {
            crate::transaction::install::InvalidUrl {
                url: installer.url.to_owned(),
            }
        })?;
        let filename = url.path_segments().unwrap().last().unwrap();
        let pkg_path =
            crate::repo::download_path(&*self.config.read().unwrap(), &url.as_str()).join(filename);

        if !pkg_path.exists() {
            eprintln!("Package path doesn't exist: {:?}", &pkg_path);
            return Err(crate::transaction::install::InstallError::PackageNotInCache);
        }

        let ext = pkg_path.extension().and_then(OsStr::to_str).unwrap();
        // .context(InvalidExtension)?;

        let file = File::open(&pkg_path).unwrap();

        let reader = match ext {
            "txz" | "xz" => XzDecoder::new(file),
            _ => panic!(), //return Err(crate::transaction::install::InstallError::InvalidExtension {}),
        };

        let mut tar_file = tar::Archive::new(reader);
        let mut files: Vec<PathBuf> = vec![];

        let pkg_path = self.package_path(&package);
        create_dir_all(&pkg_path).unwrap(); //.context(CreateDirFailed)?;

        for entry in tar_file.entries().unwrap() {
            let mut entry = entry.unwrap();
            let unpack_res;
            {
                unpack_res = entry.unpack_in(&pkg_path).unwrap(); //.context(UnpackFailed)?;
            }

            if unpack_res {
                let entry_path = entry.header().path().unwrap();
                files.push(entry_path.to_path_buf());
            } else {
                continue;
            }
        }

        let deps = &package.dependencies;
        let dependencies: Vec<String> = deps.keys().map(|x| x.to_owned()).collect();

        {
            let record = PackageRecord {
                id: package.id.to_owned(),
                version: package.version.to_owned(),
                files,
                dependencies,
            };

            let mut conn = self.pool.get().unwrap();
            record.save(&mut conn).unwrap();
        };

        Ok(PackageStatus::UpToDate)
    }
    
    fn uninstall(
        &self,
        key: &AbsolutePackageKey,
        target: &Self::Target,
    ) -> Result<PackageStatus, UninstallError> {
        let package = self
            .resolve_package(key)
            .ok_or(UninstallError::NoPackage)?;

        let mut conn = self.pool.get().unwrap();
        let record = match PackageRecord::find_by_id(&mut conn, &package.id) {
            None => return Err(UninstallError::NoPackage),
            Some(v) => v,
        };

        let pkg_path = self.package_path(&package);
        for file in &record.files {
            let file = match pkg_path.join(file).canonicalize() {
                Ok(v) => v,
                Err(_) => continue,
            };

            if file.is_dir() {
                continue;
            }

            if file.exists() {
                remove_file(file).unwrap();
            }
        }

        for file in &record.files {
            let file = match pkg_path.join(file).canonicalize() {
                Ok(v) => v,
                Err(_) => continue,
            };

            if !file.is_dir() {
                continue;
            }

            let dir = read_dir(&file).unwrap();
            if dir.count() == 0 {
                remove_dir(&file).unwrap();
            }
        }

        record.delete(&mut conn).unwrap();

        Ok(PackageStatus::NotInstalled)
    }
    
    fn find_package_dependencies(
        &self,
        key: &AbsolutePackageKey,
        package: &Package,
        target: &Self::Target,
    ) -> Result<Vec<crate::PackageDependency>, PackageDependencyError> {
        // TODO!
        Ok(vec![])
    }
    
    fn status(
        &self,
        key: &AbsolutePackageKey,
        target: &InstallTarget,
    ) -> Result<PackageStatus, PackageStatusError> {
        unimplemented!()
    }
    
    fn find_package_by_id(&self, package_id: &str) -> Option<(AbsolutePackageKey, Package)> {
        unimplemented!()
    }
    
    fn refresh_repos(&self) {
        unimplemented!()
    }
    
    fn clear_cache(&self) {
        unimplemented!()
    }
    
    fn add_repo(&self, url: String, channel: String) -> Result<bool, Box<dyn std::error::Error>> {
        unimplemented!()
    }
    
    fn remove_repo(
        &self,
        url: String,
        channel: String,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        unimplemented!()
    }

    fn update_repo(
        &self,
        index: usize,
        url: String,
        channel: String,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        unimplemented!()
    }
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("No installer found for package: {}", id))]
    NoInstaller {
        id: String,
        // source: std::io::Error
    },
    #[snafu(display("Wrong installer type"))]
    WrongInstallerType,
    #[snafu(display("Invalid extension"))]
    InvalidExtension {
        // source: std::io::Error
    },
    CreateDirFailed {
        source: std::io::Error,
    },
    UnpackFailed {
        source: std::io::Error,
    },
}

type Result<T, E = Error> = std::result::Result<T, E>;

static PKG_STORE_INIT: &'static str = include_str!("./pkgstore_init.sql");

// impl TarballPackageStore {

//     pub fn uninstall(&self, package: &Package) -> Result<(), ()> {
//         let record = {
//             let mut conn = self.conn.borrow_mut();

//             match PackageRecord::find_by_id(&mut conn, &package.id) {
//                 None => return Err(()),
//                 Some(v) => v,
//             }
//         };

//         let pkg_path = self.package_path(package);
//         for file in &record.files {
//             let file = match pkg_path.join(file).canonicalize() {
//                 Ok(v) => v,
//                 Err(_) => continue,
//             };

//             if file.is_dir() {
//                 continue;
//             }

//             if file.exists() {
//                 remove_file(file).unwrap();
//             }
//         }

//         for file in &record.files {
//             let file = match pkg_path.join(file).canonicalize() {
//                 Ok(v) => v,
//                 Err(_) => continue,
//             };

//             if !file.is_dir() {
//                 continue;
//             }

//             let dir = read_dir(&file).unwrap();
//             if dir.count() == 0 {
//                 remove_dir(&file).unwrap();
//             }
//         }

//         record.delete(&mut self.conn.borrow_mut()).unwrap();

//         Ok(())
//     }

//     pub fn status(&self, package: &Package) -> Result<PackageStatus, PackageStatusError> {
//         let installed_pkg =
//             match PackageRecord::find_by_id(&mut self.conn.borrow_mut(), &package.id) {
//                 None => return Ok(PackageStatus::NotInstalled),
//                 Some(v) => v,
//             };

//         let installed_version = match semver::Version::parse(&installed_pkg.version) {
//             Err(_) => return Err(PackageStatusError::ParsingVersion),
//             Ok(v) => v,
//         };

//         let candidate_version = match semver::Version::parse(&package.version) {
//             Err(_) => return Err(PackageStatusError::ParsingVersion),
//             Ok(v) => v,
//         };

//         if candidate_version > installed_version {
//             Ok(PackageStatus::RequiresUpdate)
//         } else {
//             Ok(PackageStatus::UpToDate)
//         }
//     }
// }

// #[allow(dead_code)]
// pub struct Prefix {
//     prefix: PathBuf,
//     store: TarballPackageStore,
//     config: StoreConfig,
// }

// impl Prefix {
//     pub fn store(&self) -> &TarballPackageStore {
//         &self.store
//     }

//     pub fn config(&self) -> &StoreConfig {
//         &self.config
//     }

//     fn package_db_path(config: &StoreConfig) -> PathBuf {
//         config.config_dir().join("packages.sqlite")
//     }

//     pub fn create(prefix_path: &Path) -> Result<Prefix, Box<dyn std::error::Error>> {
//         create_dir_all(&prefix_path)?;
//         create_dir_all(&prefix_path.join("prefix"))?;
//         create_dir_all(&prefix_path.join("packages"))?;

//         let config = StoreConfig::new(prefix_path);
//         config.save()?;

//         let conn = rusqlite::Connection::open(Prefix::package_db_path(&config)).unwrap();
//         conn.execute_batch(PKG_STORE_INIT)?;

//         let store = TarballPackageStore::new(conn, &prefix_path);

//         Ok(Prefix {
//             prefix: prefix_path.to_owned(),
//             store,
//             config,
//         })
//     }

//     pub fn open(prefix: &Path) -> Result<Prefix, Box<dyn std::error::Error>> {
//         let prefix = prefix.canonicalize().unwrap();
//         println!("{:?}", &prefix);
//         let config = StoreConfig::load(&prefix.join("config.json"), true).unwrap();

//         let db_path = Prefix::package_db_path(&config);
//         println!("{:?}", &db_path);
//         let conn = rusqlite::Connection::open(&db_path)?;
//         let store = TarballPackageStore::new(conn, &prefix);

//         Ok(Prefix {
//             prefix,
//             store,
//             config,
//         })
//     }
// }

#[derive(Debug)]
struct PackageRecord {
    id: String,
    version: String,
    files: Vec<PathBuf>,
    dependencies: Vec<String>,
}

struct PackageDbConnection<'a>(&'a mut rusqlite::Connection);

impl<'a> PackageDbConnection<'a> {
    fn dependencies(&self, id: &str) -> Vec<String> {
        let mut stmt = self
            .0
            .prepare("SELECT dependency_id FROM packages_dependencies WHERE package_id = ?")
            .unwrap();

        let res = stmt
            .query_map(&[&id], |row| row.get(0))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();

        res
    }

    fn files(&self, id: &str) -> Vec<PathBuf> {
        let mut stmt = self
            .0
            .prepare("SELECT file_path FROM packages_files WHERE package_id = ?")
            .unwrap();

        let res = stmt
            .query_map(&[&id], |row| row.get(0))
            .unwrap()
            .map(|x: Result<String, _>| Path::new(&x.unwrap()).to_path_buf())
            .collect();

        res
    }

    fn version(&self, id: &str) -> Option<String> {
        match self.0.query_row(
            "SELECT version FROM packages WHERE id = ? LIMIT 1",
            &[&id],
            |row| row.get(0),
        ) {
            Ok(v) => v,
            Err(_) => return None,
        }
    }

    fn replace_pkg(&mut self, pkg: &PackageRecord) -> rusqlite::Result<()> {
        let tx = self.0.transaction().unwrap();

        tx.execute(
            "REPLACE INTO packages(id, version) VALUES (?, ?)",
            &[&pkg.id, &pkg.version],
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
                    "INSERT INTO packages_dependencies(package_id, dependency_id) VALUES (?, ?)",
                )
                .unwrap();
            for dep_id in &pkg.dependencies {
                dep_stmt.execute(&[&pkg.id, &*dep_id])?;
            }

            let mut file_stmt = tx
                .prepare("INSERT INTO packages_files(package_id, file_path) VALUES (:id, :path)")?;

            for file_path in &pkg.files {
                file_stmt
                    .execute_named(&[
                        (":id", &pkg.id),
                        (":path", &file_path.as_os_str().as_bytes()),
                    ])
                    .unwrap();
            }
        }

        tx.commit()
    }

    fn remove_pkg(&mut self, pkg: &PackageRecord) -> rusqlite::Result<()> {
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

impl PackageRecord {
    pub fn find_by_id(conn: &mut rusqlite::Connection, id: &str) -> Option<PackageRecord> {
        let conn = PackageDbConnection(conn);

        let version = match conn.version(id) {
            Some(v) => v,
            None => return None,
        };

        let files = conn.files(id);
        let dependencies = conn.dependencies(id);

        Some(PackageRecord {
            id: id.to_owned(),
            version: version,
            files: files,
            dependencies: dependencies,
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

//     let pkg = PackageRecord {
//         id: "test-pkg".to_owned(),
//         version: "1.0.0".to_owned(),
//         files: vec![Path::new("bin/test").to_path_buf()],
//         dependencies: vec!["one-thing".to_owned()]
//     };

//     {
//         pkg.save(conn.transaction().unwrap()).unwrap();
//     }

//     let found = PackageRecord::find_by_id(&mut conn, "test-pkg");
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
