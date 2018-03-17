use std::sync::{atomic, Arc, RwLock};
use std::collections::HashMap;
// use std::default::Default;
// use jsonrpc_core::{Metadata, Error, ErrorCode, Result};
// use jsonrpc_core::futures::{Future, Stream, future};
// use jsonrpc_core::futures::sync::mpsc;
// use jsonrpc_pubsub::{Session, PubSubMetadata, PubSubHandler, SubscriptionId};
// use jsonrpc_macros::pubsub;
use std::fs::create_dir_all;

use super::*;
use pahkat::types::*;
use ::windows::WindowsPackageStore;
use ::{PackageStatus, Repository, StoreConfig, Download};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageStatusResponse {
	pub status: PackageStatus
}

#[derive(Default)]
pub struct RpcImpl {
	pub uid: atomic::AtomicUsize,
	pub active: Arc<RwLock<HashMap<SubscriptionId, pubsub::Sink<String>>>>,
	pub repo_configs: Arc<RwLock<Vec<RepoConfig>>>,
	pub repo: Arc<RwLock<HashMap<String, Repository>>>
}

impl Rpc for RpcImpl {
	type Metadata = Meta;

	fn repository(&self, url: String, _channel: String) -> Result<Repository> {
		let repo = Repository::from_url(&url).unwrap();
		let mut repo_map = self.repo.write().unwrap();
		repo_map.insert(url, repo.clone());
		// println!("{:?}", repo);
		Ok(repo)
	}

	fn status(&self, repo_id: String, package_id: String, _target: u8) -> Result<PackageStatus> {
		let repo = repo_check(&self, repo_id)?;
		let package = parse_package(&repo, &package_id)?;
		
		let config = StoreConfig::load_or_default();
		let store = WindowsPackageStore::new(&repo, &config);
		let status = store.status(&package);
		status.map_err(|e| {
			Error {
				code: ErrorCode::InvalidParams,
				message: format!("{}", e),
				data: None
			}
		})
	}

	fn repository_statuses(&self, repo_id: String) -> Result<BTreeMap<String, PackageStatusResponse>> {
		let repo = repo_check(&self, repo_id)?;
		
		let config = StoreConfig::load_or_default();
		let store = WindowsPackageStore::new(&repo, &config);

		let mut map = BTreeMap::new();

		for package in repo.packages().values() {
			let status = match store.status(&package) {
				Ok(v) => v,
				Err(e) => {
					eprintln!("{:?}", e);
					PackageStatus::NotInstalled
				}
 			};

			map.insert(package.id.clone(), PackageStatusResponse {
				status: status
			});
		}
		
		Ok(map)
	}

	fn install(&self, repo_id: String, package_id: String, _target: u8) -> Result<PackageStatus> {
		let repo = repo_check(&self, repo_id)?;
		let package = parse_package(&repo, &package_id)?;
		
		let config = StoreConfig::load_or_default();
		let store = WindowsPackageStore::new(&repo, &config);
		store.install(&package).map_err(|e| {
			// let msg = match e {
			// 	MacOSInstallError::InstallerFailure(error) => {
			// 		match error {
			// 			ProcessError::Unknown(output) => String::from_utf8_lossy(&output.stdout).to_string(),
			// 			_ => format!("{:?}", &error)
			// 		}
			// 	}
			// 	_ => format!("{:?}", &e)
			// };
			Error {
				code: ErrorCode::InvalidParams,
				message: "stub".to_owned(), // msg,
				data: None
			}
		})
	}

	fn uninstall(&self, repo_id: String, package_id: String, _target: u8) -> Result<PackageStatus> {
		let repo = repo_check(&self, repo_id)?;
		let package = parse_package(&repo, &package_id)?;
		
		let config = StoreConfig::load_or_default();
		let store = WindowsPackageStore::new(&repo, &config);
		
		store.uninstall(&package).map_err(|e| {
			// let msg = match e {
			// 	MacOSUninstallError::PkgutilFailure(error) => {
			// 		match error {
			// 			ProcessError::Unknown(output) => String::from_utf8_lossy(&output.stderr).to_string(),
			// 			_ => format!("{:?}", &error)
			// 		}
			// 	}
			// 	_ => format!("{:?}", &e)
			// };

			Error {
				code: ErrorCode::InvalidParams,
				message: "stub".to_owned(), // msg,
				data: None
			}
		})
	}

	fn download_subscribe(&self, _meta: Self::Metadata, subscriber: pubsub::Subscriber<[usize; 2]>, repo_id: String, package_id: String, _target: u8) {
		let repo = match repo_check(&self, repo_id) {
			Ok(v) => v,
			Err(e) => {
				subscriber.reject(e).unwrap();
				return;
			}
		};

		let package = match parse_package(&repo, &package_id) {
			Ok(v) => v,
			Err(e) => {
				subscriber.reject(e).unwrap();
				return;
			}
		};

		let id = self.uid.fetch_add(1, atomic::Ordering::SeqCst);
		let sub_id = SubscriptionId::Number(id as u64);
		let sink = subscriber.assign_id(sub_id.clone()).unwrap();

		thread::spawn(move || {
			let sink = sink.clone();
			let config = StoreConfig::load_or_default();
			let store = WindowsPackageStore::new(&repo, &config);

			let package_cache = store.download_path(&package);
			// println!("{:?}", &package_cache);
			if !package_cache.exists() {
				create_dir_all(&package_cache).unwrap();
			}

			let pkg_path = package.download(&package_cache, 
				Some(|cur, max| {
					match sink.notify(Ok([cur, max])).wait() {
						Ok(_) => {},
						Err(_) => {}
					}
				}));
			
			match pkg_path {
				Ok(_) => {},
				Err(e) => {
					eprintln!("{:?}", &e);
					let error = Error {
						code: ErrorCode::InvalidParams,
						message: format!("{:?}", &e),
						data: None
					};
					sink.notify(Err(error)).wait();
				}
			};
		});
	}

	fn download_unsubscribe(&self, _id: SubscriptionId) -> Result<bool> {
		// TODO: handle cancel request
		return Ok(true)
	}
}