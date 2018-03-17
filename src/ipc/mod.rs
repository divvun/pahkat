use std::thread;
use std::sync::Arc;
use std::collections::BTreeMap;
use std::io::BufRead;

use jsonrpc_core::{Metadata, Error, ErrorCode, Result};
use jsonrpc_core::futures::{Future, Stream, future};
use jsonrpc_core::futures::sync::mpsc;
use jsonrpc_pubsub::{Session, PubSubMetadata, PubSubHandler, SubscriptionId};
use jsonrpc_macros::pubsub;

use pahkat::types::*;
use ::{PackageStatus, Repository, StoreConfig};

#[cfg(windows)]
mod windows;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
use ipc::macos::*;
#[cfg(windows)]
use ipc::windows::*;

#[derive(Clone, Default)]
pub struct Meta {
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

pub fn start() {
	use std;

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
