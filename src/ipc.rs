
extern crate jsonrpc_tcp_server;

use std::thread;
use std::sync::{atomic, Arc, RwLock};
use std::collections::{BTreeMap, HashMap};

use jsonrpc_core::{Metadata, Error, ErrorCode, Result};
use jsonrpc_core::futures::{Future, Stream, future};
use jsonrpc_core::futures::sync::mpsc;
use jsonrpc_pubsub::{Session, PubSubMetadata, PubSubHandler, SubscriptionId};

use jsonrpc_macros::pubsub;
use pahkat::types::*;
use ::macos::*;
use ::{Repository, StoreConfig, PackageStatus, Download};
use std::fs::create_dir_all;
use std::io::{BufRead};
use std;

#[derive(Clone, Default)]
struct Meta {
	session: Option<Arc<Session>>,
}

impl Metadata for Meta {}
impl PubSubMetadata for Meta {
	fn session(&self) -> Option<Arc<Session>> {
		self.session.clone()
	}
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RepoConfig {
	pub url: String,
	pub channel: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageStatusResponse {
	pub status: PackageStatus,
    pub target: MacOSInstallTarget
}

build_rpc_trait! {
	pub trait Rpc {
		type Metadata;

		#[rpc(name = "repository")]
		fn repository(&self, String, String) -> Result<Repository>;

		#[rpc(name = "repository_statuses")]
		fn repository_statuses(&self, String) -> Result<BTreeMap<String, PackageStatusResponse>>;

		#[rpc(name = "status")]
		fn status(&self, String, String, u8) -> Result<PackageStatus>;

		#[rpc(name = "install")]
		fn install(&self, String, String, u8) -> Result<PackageStatus>;

		#[rpc(name = "uninstall")]
		fn uninstall(&self, String, String, u8) -> Result<PackageStatus>;

		#[pubsub(name = "download")] {
			#[rpc(name = "download_subscribe")]
			fn download_subscribe(&self, Self::Metadata, pubsub::Subscriber<[usize; 2]>, String, String, u8);

			#[rpc(name = "download_unsubscribe")]
			fn download_unsubscribe(&self, SubscriptionId) -> Result<bool>;
		}
	}
}

#[derive(Default)]
struct RpcImpl {
	uid: atomic::AtomicUsize,
	active: Arc<RwLock<HashMap<SubscriptionId, pubsub::Sink<String>>>>,
	repo_configs: Arc<RwLock<Vec<RepoConfig>>>,
	repo: Arc<RwLock<HashMap<String, Repository>>>
}

fn repo_check(rpc_impl: &RpcImpl, repo_id: String) -> Result<Repository> {
	let rw_guard = rpc_impl.repo.read().unwrap();
	match rw_guard.get(&repo_id) {
		Some(v) => Ok(v.clone()),
		None => {
			Err(Error {
				code: ErrorCode::InvalidParams,
				message: "No repository set; use `repository` method first.".to_owned(),
				data: None
			})
		}
	}
}

fn parse_target(number: u8) -> MacOSInstallTarget {
	if number == 0 {
		MacOSInstallTarget::System
	} else {
		MacOSInstallTarget::User
	}
}

fn parse_package(repo: &Repository, package_id: &str) -> Result<Package> {
	match repo.packages().get(package_id) {
		Some(v) => Ok(v.clone()),
		None => {
			Err(Error {
				code: ErrorCode::InvalidParams,
				message: "No package found with identifier.".to_owned(),
				data: None
			})
		}
	}
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

	fn status(&self, repo_id: String, package_id: String, target: u8) -> Result<PackageStatus> {
		let repo = repo_check(&self, repo_id)?;
		let package = parse_package(&repo, &package_id)?;
		let target = parse_target(target);
		
		let config = StoreConfig::load_or_default();
		let store = MacOSPackageStore::new(&repo, &config);
		let status = store.status(&package, target);
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
		let store = MacOSPackageStore::new(&repo, &config);

		let mut map = BTreeMap::new();

		for package in repo.packages().values() {
			let status = match store.status(&package, MacOSInstallTarget::System) {
				Ok(v) => v,
				Err(e) => {
					eprintln!("{:?}", e);
					PackageStatus::NotInstalled
				}
 			};

			match status {
				PackageStatus::NotInstalled => {},
				_ => {
					map.insert(package.id.clone(), PackageStatusResponse {
						status: status,
						target: MacOSInstallTarget::System
					});
					continue;
				}
			};

			let status = match store.status(&package, MacOSInstallTarget::User) {
				Ok(v) => v,
				Err(e) => {
					eprintln!("{:?}", e);
					PackageStatus::NotInstalled
				}
 			};

			map.insert(package.id.clone(), PackageStatusResponse {
				status: status,
				target: MacOSInstallTarget::User
			});
		}
		
		Ok(map)
	}

	fn install(&self, repo_id: String, package_id: String, target: u8) -> Result<PackageStatus> {
		let repo = repo_check(&self, repo_id)?;
		let package = parse_package(&repo, &package_id)?;
		let target = parse_target(target);
		
		let config = StoreConfig::load_or_default();
		let store = MacOSPackageStore::new(&repo, &config);
		store.install(&package, target).map_err(|e| {
			let msg = match e {
				MacOSInstallError::InstallerFailure(error) => {
					match error {
						ProcessError::Unknown(output) => String::from_utf8_lossy(&output.stderr).to_string(),
						_ => format!("{:?}", &error)
					}
				}
				_ => format!("{:?}", &e)
			};
			Error {
				code: ErrorCode::InvalidParams,
				message: msg,
				data: None
			}
		})
	}

	fn uninstall(&self, repo_id: String, package_id: String, target: u8) -> Result<PackageStatus> {
		let repo = repo_check(&self, repo_id)?;
		let package = parse_package(&repo, &package_id)?;
		let target = parse_target(target);
		
		let config = StoreConfig::load_or_default();
		let store = MacOSPackageStore::new(&repo, &config);
		
		store.uninstall(&package, target).map_err(|e| {
			let msg = match e {
				MacOSUninstallError::PkgutilFailure(error) => {
					match error {
						ProcessError::Unknown(output) => String::from_utf8_lossy(&output.stderr).to_string(),
						_ => format!("{:?}", &error)
					}
				}
				_ => format!("{:?}", &e)
			};

			Error {
				code: ErrorCode::InvalidParams,
				message: msg,
				data: None
			}
		})
	}

	fn download_subscribe(&self, _meta: Self::Metadata, subscriber: pubsub::Subscriber<[usize; 2]>, repo_id: String, package_id: String, target: u8) {
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

		let _target = parse_target(target);

		let id = self.uid.fetch_add(1, atomic::Ordering::SeqCst);
		let sub_id = SubscriptionId::Number(id as u64);
		let sink = subscriber.assign_id(sub_id.clone()).unwrap();

		thread::spawn(move || {
			let sink = sink.clone();
			let config = StoreConfig::load_or_default();
			let store = MacOSPackageStore::new(&repo, &config);

			let package_cache = store.download_path(&package);
			// println!("{:?}", &package_cache);
			if !package_cache.exists() {
				create_dir_all(&package_cache).unwrap();
			}
			let _pkg_path = package.download(&package_cache, 
				Some(|cur, max| {
					match sink.notify(Ok([cur, max])).wait() {
						Ok(_) => {},
						Err(_) => {}
					}
				})).unwrap();
		});
	}

	fn download_unsubscribe(&self, _id: SubscriptionId) -> Result<bool> {
		// TODO: handle cancel request
		return Ok(true)
	}
}

pub fn start() {
	let mut io = PubSubHandler::default();
	let rpc = RpcImpl::default();

	io.extend_with(rpc.to_delegate());
	
	let (sender, receiver) = mpsc::channel::<String>(0);
	thread::spawn(move || {
		receiver.for_each(|item| {
			println!("{}", item);
			future::ok(())
		}).wait();
	});
	
	let stdin = std::io::stdin();
	let mut stdin = stdin.lock();

	loop {
		let mut buf = vec![];
		match stdin.read_until('\n' as u8, &mut buf) {
			Err(_) | Ok(0) => break,
			Ok(_) => {}
		};
		
		let req = String::from_utf8_lossy(&buf);
		let meta = Meta {
			session: Some(Arc::new(Session::new(sender.clone())))
		};
		
		match io.handle_request_sync(&req, meta) {
			Some(v) => println!("{}", v),
			None => {}
		};
    }
}
