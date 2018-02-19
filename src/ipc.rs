
extern crate jsonrpc_tcp_server;

use std::thread;
use std::sync::{atomic, Arc, RwLock};
use std::collections::HashMap;

use jsonrpc_core::{Params, Metadata, Error, ErrorCode, Result};
use jsonrpc_core::types::Value;
use jsonrpc_core::futures::Future;
use jsonrpc_pubsub::{Session, PubSubMetadata, PubSubHandler, SubscriptionId};

use jsonrpc_macros::pubsub;
use pahkat::types::*;
use ::macos::*;
use ::{Repository, StoreConfig, PackageStatus, PackageStatusError, Download};


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

build_rpc_trait! {
	pub trait Rpc {
		type Metadata;

		#[rpc(name = "repository")]
		fn repository(&self, String) -> Result<Repository>;

		#[rpc(name = "status")]
		fn status(&self, String, u8) -> Result<PackageStatus>;

		#[rpc(name = "install")]
		fn install(&self, String, u8) -> Result<PackageStatus>;

		#[rpc(name = "uninstall")]
		fn uninstall(&self, String, u8) -> Result<PackageStatus>;

		#[pubsub(name = "download")] {
			#[rpc(name = "download_subscribe")]
			fn download_subscribe(&self, Self::Metadata, pubsub::Subscriber<[usize; 2]>, String, u8);

			#[rpc(name = "download_unsubscribe")]
			fn download_unsubscribe(&self, SubscriptionId) -> Result<bool>;
		}

		// #[pubsub(name = "install")] {
		// 	#[rpc(name = "install_subscribe")]
		// 	fn install_subscribe(&self, pubsub::Subscriber<usize>, String, String);

		// 	#[rpc(name = "install_unsubscribe")]
		// 	fn install_unsubscribe(&self, SubscriptionId) -> Result<bool>;
		// }

		// #[pubsub(name = "uninstall")] {
		// 	#[rpc(name = "uninstall_subscribe")]
		// 	fn uninstall_subscribe(&self, pubsub::Subscriber<usize>, String, String);

		// 	#[rpc(name = "uninstall_unsubscribe")]
		// 	fn uninstall_unsubscribe(&self, SubscriptionId) -> Result<bool>;
		// }
	}
}

#[derive(Default)]
struct RpcImpl {
	uid: atomic::AtomicUsize,
	active: Arc<RwLock<HashMap<SubscriptionId, pubsub::Sink<String>>>>,
	current_repo: Arc<RwLock<Option<Repository>>>
}

fn repo_check(rpc_impl: &RpcImpl) -> Result<Repository> {
	let rw_guard = rpc_impl.current_repo.read().unwrap();
	match *rw_guard {
		Some(ref v) => Ok(v.clone()),
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

	fn repository(&self, url: String) -> Result<Repository> {
		let repo = Repository::from_url(&url).unwrap();
		*self.current_repo.write().unwrap() = Some(repo.clone());
		// println!("{:?}", repo);
		Ok(repo)
	}

	fn status(&self, package_id: String, target: u8) -> Result<PackageStatus> {
		let repo = repo_check(&self)?;
		let package = parse_package(&repo, &package_id)?;
		let target = parse_target(target);
		
		// TODO make init check
		let config = StoreConfig::load_default().unwrap();
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

	fn install(&self, package_id: String, target: u8) -> Result<PackageStatus> {
		let repo = repo_check(&self)?;
		let package = parse_package(&repo, &package_id)?;
		let target = parse_target(target);
		
		// TODO make init check
		let config = StoreConfig::load_default().unwrap();
		let store = MacOSPackageStore::new(&repo, &config);
		store.install(&package, target).map_err(|e| {
			Error {
				code: ErrorCode::InvalidParams,
				message: format!("{}", "An error occurred."),//e),
				data: None
			}
		})
	}


	fn uninstall(&self, package_id: String, target: u8) -> Result<PackageStatus> {
		let repo = repo_check(&self)?;
		let package = parse_package(&repo, &package_id)?;
		let target = parse_target(target);
		
		// TODO make init check
		let config = StoreConfig::load_default().unwrap();
		let store = MacOSPackageStore::new(&repo, &config);
		store.uninstall(&package, target).map_err(|e| {
			Error {
				code: ErrorCode::InvalidParams,
				message: format!("{}", "An error occurred."),//e),
				data: None
			}
		})
	}

	fn download_subscribe(&self, _meta: Self::Metadata, subscriber: pubsub::Subscriber<[usize; 2]>, package_id: String, target: u8) {
		let repo = match repo_check(&self) {
			Ok(v) => v,
			Err(e) => {
				subscriber.reject(e);
				return;
			}
		};

		let package = match parse_package(&repo, &package_id) {
			Ok(v) => v,
			Err(e) => {
				subscriber.reject(e);
				return;
			}
		};

		let target = parse_target(target);

		let id = self.uid.fetch_add(1, atomic::Ordering::SeqCst);
		let sub_id = SubscriptionId::Number(id as u64);
		let sink = subscriber.assign_id(sub_id.clone()).unwrap();

		thread::spawn(move || {
			let sink = sink.clone();
			let config = StoreConfig::load_default().unwrap();
			let store = MacOSPackageStore::new(&repo, &config);

			let package_cache = store.download_path(&package);
			let pkg_path = package.download(&package_cache, 
				Some(|cur, max| {
					match sink.notify(Ok([cur, max])).wait() {
						Ok(_) => {},
						Err(_) => {
							println!("Subscription has ended, finishing.");
						}
					}
				})).unwrap();
		});
	}

	fn download_unsubscribe(&self, id: SubscriptionId) -> Result<bool> {
		return Ok(true)
	}

	// fn subscribe(&self, _meta: Self::Metadata, subscriber: pubsub::Subscriber<String>, param: u64) {
	// 	if param != 10 {
	// 		subscriber.reject(Error {
	// 			code: ErrorCode::InvalidParams,
	// 			message: "Rejecting subscription - invalid parameters provided.".into(),
	// 			data: None,
	// 		}).unwrap();
	// 		return;
	// 	}

	// 	let id = self.uid.fetch_add(1, atomic::Ordering::SeqCst);
	// 	let sub_id = SubscriptionId::Number(id as u64);
	// 	let sink = subscriber.assign_id(sub_id.clone()).unwrap();
	// 	self.active.write().unwrap().insert(sub_id, sink);
	// }

	// fn unsubscribe(&self, id: SubscriptionId) -> Result<bool> {
	// 	let removed = self.active.write().unwrap().remove(&id);
	// 	if removed.is_some() {
	// 		Ok(true)
	// 	} else {
	// 		Err(Error {
	// 			code: ErrorCode::InvalidParams,
	// 			message: "Invalid subscription.".into(),
	// 			data: None,
	// 		})
	// 	}
	// }
}

pub fn start() {
	let mut io = PubSubHandler::default();
	let rpc = RpcImpl::default();
	let active_subscriptions = rpc.active.clone();

	thread::spawn(move || {
		loop {
			{
				let subscribers = active_subscriptions.read().unwrap();
				println!("{:?}", subscribers.len());
			}
			thread::sleep(::std::time::Duration::from_secs(1));
		}
	});

	io.extend_with(rpc.to_delegate());

	let server = jsonrpc_tcp_server::ServerBuilder::new(io)
		.session_meta_extractor(|context: &jsonrpc_tcp_server::RequestContext| {
			Meta {
				session: Some(Arc::new(Session::new(context.sender.clone()))),
			}
		})
		.start(&"0.0.0.0:3030".parse().unwrap())
		.expect("Server must start with no issues");

	server.wait()
}
