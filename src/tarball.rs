use pahkat::types::*;
use pahkat::types::{Repository as RepositoryMeta};
use pahkat::types::Downloadable;
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use xz2::read::XzDecoder;
use std::fs::{remove_file, read_dir, remove_dir, create_dir_all, File};
use std::cell::RefCell;
use rhai::RegisterFn;
use ::{Repository, PackageStatusError, PackageStatus, StoreConfig};
use rusqlite;
use serde_json;
use semver;
use tar;
use std;
use rhai;

pub struct TarballPackageStore {
    conn: RefCell<rusqlite::Connection>,
    prefix: PathBuf
}

impl TarballPackageStore {
    fn init_sqlite_database(&self) -> rusqlite::Result<()> {
        self.conn.borrow().execute_batch(include_str!("./pkgstore_init.sql"))
    }

    pub fn create_cache(&self) -> PathBuf {
        let path = self.prefix.join("var/pahkatc/cache");
        create_dir_all(&path).unwrap();
        path
    }

    pub fn create_receipts(&self) -> PathBuf {
        let path = self.prefix.join("var/pahkatc/receipts");
        create_dir_all(&path).unwrap();
        path
    }

    fn new(conn: rusqlite::Connection, prefix: &Path) -> TarballPackageStore {
        TarballPackageStore {
            conn: RefCell::new(conn),
            prefix: prefix.to_owned()
        }
    }

    pub fn install(&self, package: &Package, path: &Path) -> Result<(), ()> {
        let installer = match package.installer() {
            None => return Err(()),
            Some(v) => v
        };

        let tarball = match installer {
            &Installer::Tarball(ref v) => v,
            _ => return Err(())
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
                unpack_res = entry.unpack_in(&self.prefix);
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
        };

        let receipt = self.create_receipts().join(format!("{}.rhai", &package.id));

        if receipt.exists() {
            fn rhai_println<T: std::fmt::Display>(x: &mut T) -> () {
                println!("{}", x)
            }
            
            let mut engine = rhai::Engine::new();
            let mut scope = rhai::Scope::new();
            let mut chai_str = String::new();
            File::open(receipt).unwrap().read_to_string(&mut chai_str).unwrap();

            engine.register_fn("println", rhai_println as fn(x: &mut String)->());

            if let Err(err) = engine.consume_with_scope(&mut scope, &chai_str) {
                println!("An error occurred initialising receipt: {:?}", err);
            }

            if let Err(err) = engine.eval_with_scope::<bool>(&mut scope, "install()") {
                println!("An error occurred running install() in receipt: {:?}", err);
            }
        };

        Ok(())
    }

    pub fn uninstall(&self, package: &Package) -> Result<(), ()> {
        let record = {
            let conn = self.conn.borrow();

            match PackageRecord::find_by_id(&conn, &package.id) {
                None => return Err(()),
                Some(v) => v
            }
        };

        let receipt = self.create_receipts().join(format!("{}.rhai", &package.id));

        if receipt.exists() {
            fn rhai_println<T: std::fmt::Display>(x: &mut T) -> () {
                println!("{}", x)
            }
            
            let mut engine = rhai::Engine::new();
            let mut scope = rhai::Scope::new();
            let mut chai_str = String::new();
            File::open(receipt).unwrap().read_to_string(&mut chai_str).unwrap();

            engine.register_fn("println", rhai_println as fn(x: &mut String)->());

            if let Err(err) = engine.consume_with_scope(&mut scope, &chai_str) {
                println!("An error occurred initialising receipt: {:?}", err);
            }

            if let Err(err) = engine.eval_with_scope::<bool>(&mut scope, "uninstall()") {
                println!("An error occurred running uninstall() in receipt: {:?}", err);
            }
        };

        for file in &record.files {
            let file = match self.prefix.join(file).canonicalize() {
                Ok(v) => v,
                Err(_) => continue
            };

            if file.is_dir() {
                continue;
            }

            if file.exists() {
                remove_file(file).unwrap();
            }
        }

        for file in &record.files {
            let file = match self.prefix.join(file).canonicalize() {
                Ok(v) => v,
                Err(_) => continue
            };
            
            if !file.is_dir() {
                continue
            }

            let dir = read_dir(&file).unwrap();
            if dir.count() == 0 {
                remove_dir(&file).unwrap();
            }
        }

        let mut conn = self.conn.borrow_mut();
        let tx = conn.transaction().unwrap();
        record.delete(tx).unwrap();

        Ok(())
    }

    pub fn status(&self, package: &Package) -> Result<PackageStatus, PackageStatusError> {
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

pub struct Prefix {
    prefix: PathBuf,
    store: TarballPackageStore,
    config: StoreConfig
}

impl Prefix {
    pub fn store(&self) -> &TarballPackageStore {
        &self.store
    }

    pub fn config(&self) -> &StoreConfig {
        &self.config
    }

    pub fn create(prefix: &Path, url: &str) -> Result<Prefix, ()> {
        let cache_dir = prefix.join("var/pahkatc/cache");
        let config = StoreConfig {
            url: url.to_owned(),
            cache_dir: cache_dir.to_str().unwrap().to_owned()
        };

        if !cache_dir.exists() {
            create_dir_all(cache_dir).unwrap()
        }

        let config_path = prefix.join("etc/pahkatc/config.json");
        if config_path.exists() {
            return Err(())
        }

        let db_path = prefix.join("var/pahkatc/packages.sqlite");
        if db_path.exists() {
            return Err(())
        }

        let cfg_str = serde_json::to_string_pretty(&config).unwrap();
        {
            create_dir_all(config_path.parent().unwrap()).unwrap();
            let mut file = File::create(config_path).unwrap();
            file.write_all(cfg_str.as_bytes()).unwrap();
        }

        create_dir_all(db_path.parent().unwrap()).unwrap();
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let store = TarballPackageStore::new(conn, &prefix);
        store.init_sqlite_database().unwrap();

        Ok(Prefix {
            prefix: prefix.to_owned(),
            store: store,
            config: config
        })
    }

    pub fn open(prefix: &Path) -> Result<Prefix, ()> {
        let prefix = prefix.canonicalize().unwrap().to_owned();

        let config_path = prefix.join("etc/pahkatc/config.json");
        if !config_path.exists() {
            return Err(())
        }

        let db_path = prefix.join("var/pahkatc/packages.sqlite");
        if !db_path.exists() {
            return Err(())
        }

        let conn = rusqlite::Connection::open(&db_path).unwrap();

        let file = File::open(config_path).unwrap();
        let config: StoreConfig = serde_json::from_reader(file).unwrap();

        let store = TarballPackageStore::new(conn, &prefix);

        Ok(Prefix {
            prefix: prefix,
            store: store,
            config: config
        })
    }
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
