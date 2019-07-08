#![cfg(feature = "prefix")]

use crate::{PackageStatus, PackageStatusError, StoreConfig};
use pahkat_types::*;
use snafu::{ensure, Backtrace, ErrorCompat, OptionExt, ResultExt, Snafu};
use std::cell::RefCell;
use std::ffi::OsStr;
use std::fs::{create_dir_all, read_dir, remove_dir, remove_file, File};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use xz2::read::XzDecoder;

pub struct TarballPackageStore {
    conn: RefCell<rusqlite::Connection>,
    prefix: PathBuf,
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

impl TarballPackageStore {
    fn init_sqlite_database(&self) -> rusqlite::Result<()> {
        self.conn.borrow().execute_batch(PKG_STORE_INIT)
    }

    fn new(conn: rusqlite::Connection, prefix: &Path) -> TarballPackageStore {
        TarballPackageStore {
            conn: RefCell::new(conn),
            prefix: prefix.to_owned(),
        }
    }

    fn package_path(&self, package: &Package) -> PathBuf {
        self.prefix.join(&package.id).join(&package.version)
    }

    pub fn install(&self, package: &Package, path: &Path) -> Result<()> {
        let installer = package.installer().with_context(|| NoInstaller {
            id: package.id.clone(),
        })?;

        let _tarball = match installer {
            &Installer::Tarball(ref v) => v,
            _ => return Err(Error::WrongInstallerType),
        };

        let ext = path
            .extension()
            .and_then(OsStr::to_str)
            .context(InvalidExtension)?;

        let file = File::open(path).unwrap();

        let reader = match ext {
            "txz" | "xz" => XzDecoder::new(file),
            _ => return Err(Error::InvalidExtension {}),
        };

        let mut tar_file = tar::Archive::new(reader);
        let mut files: Vec<PathBuf> = vec![];

        let pkg_path = self.package_path(package);
        create_dir_all(&pkg_path).context(CreateDirFailed)?;

        for entry in tar_file.entries().unwrap() {
            let mut entry = entry.unwrap();
            let unpack_res;
            {
                unpack_res = entry.unpack_in(&pkg_path).context(UnpackFailed)?;
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
                files: files,
                dependencies: dependencies,
            };

            record.save(&mut self.conn.borrow_mut()).unwrap();
        };

        Ok(())
    }

    pub fn uninstall(&self, package: &Package) -> Result<(), ()> {
        let record = {
            let mut conn = self.conn.borrow_mut();

            match PackageRecord::find_by_id(&mut conn, &package.id) {
                None => return Err(()),
                Some(v) => v,
            }
        };

        let pkg_path = self.package_path(package);
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

        record.delete(&mut self.conn.borrow_mut()).unwrap();

        Ok(())
    }

    pub fn status(&self, package: &Package) -> Result<PackageStatus, PackageStatusError> {
        let installed_pkg =
            match PackageRecord::find_by_id(&mut self.conn.borrow_mut(), &package.id) {
                None => return Ok(PackageStatus::NotInstalled),
                Some(v) => v,
            };

        let installed_version = match semver::Version::parse(&installed_pkg.version) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v,
        };

        let candidate_version = match semver::Version::parse(&package.version) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v,
        };

        if candidate_version > installed_version {
            Ok(PackageStatus::RequiresUpdate)
        } else {
            Ok(PackageStatus::UpToDate)
        }
    }
}

#[allow(dead_code)]
pub struct Prefix {
    prefix: PathBuf,
    store: TarballPackageStore,
    config: StoreConfig,
}

impl Prefix {
    pub fn store(&self) -> &TarballPackageStore {
        &self.store
    }

    pub fn config(&self) -> &StoreConfig {
        &self.config
    }

    fn package_db_path(config: &StoreConfig) -> PathBuf {
        config.config_dir().join("packages.sqlite")
    }

    pub fn create(prefix_path: &Path) -> Result<Prefix, Box<dyn std::error::Error>> {
        create_dir_all(&prefix_path)?;
        create_dir_all(&prefix_path.join("prefix"))?;
        create_dir_all(&prefix_path.join("packages"))?;
        let config = StoreConfig::new(prefix_path);
        config.save()?;

        let conn = rusqlite::Connection::open(Prefix::package_db_path(&config)).unwrap();
        let store = TarballPackageStore::new(conn, &prefix_path);
        store.init_sqlite_database().unwrap();

        Ok(Prefix {
            prefix: prefix_path.to_owned(),
            store: store,
            config: config,
        })
    }

    pub fn open(prefix: &Path) -> Result<Prefix, Box<dyn std::error::Error>> {
        let prefix = prefix.canonicalize().unwrap();
        println!("{:?}", &prefix);
        let config = StoreConfig::load(&prefix.join("config.json"), true).unwrap();

        let db_path = Prefix::package_db_path(&config);
        println!("{:?}", &db_path);
        let conn = rusqlite::Connection::open(&db_path)?;
        let store = TarballPackageStore::new(conn, &prefix);

        Ok(Prefix {
            prefix: prefix,
            store: store,
            config: config,
        })
    }
}

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
        )?;
        tx.execute(
            "DELETE FROM packages_dependencies WHERE package_id = ?",
            &[&pkg.id],
        )?;
        tx.execute(
            "DELETE FROM packages_files WHERE package_id = ?",
            &[&pkg.id],
        )?;

        {
            let mut dep_stmt = tx.prepare(
                "INSERT INTO packages_dependencies(package_id, dependency_id) VALUES (?, ?)",
            )?;
            for dep_id in &pkg.dependencies {
                dep_stmt.execute(&[&pkg.id, &*dep_id])?;
            }

            let mut file_stmt = tx
                .prepare("INSERT INTO packages_files(package_id, file_path) VALUES (:id, :path)")?;

            for file_path in &pkg.files {
                file_stmt.execute_named(&[
                    ("id", &pkg.id),
                    ("path", &file_path.as_os_str().as_bytes()),
                ])?;
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
