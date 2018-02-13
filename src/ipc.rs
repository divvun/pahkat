

use std::thread;
use std::sync::{atomic, Arc, RwLock};
use std::collections::HashMap;

use jsonrpc_core::*;
use jsonrpc_core::futures::Future;
use jsonrpc_pubsub::{PubSubHandler, PubSubMetadata, Session, Subscriber, SubscriptionId};
use jsonrpc_tcp_server::{ServerBuilder, RequestContext};

use jsonrpc_macros::pubsub;

// #[derive(Clone, Default)]
// struct Meta {
// 	session: Option<Arc<Session>>,
// }

// impl Metadata for Meta {}
// impl PubSubMetadata for Meta {
// 	fn session(&self) -> Option<Arc<Session>> {
// 		self.session.clone()
// 	}
// }

// build_rpc_trait! {
// 	pub trait Rpc {
// 		type Metadata;

//         #[pubsub(name = "set_repo")] {
//             #[rpc(name = "set_repo_subscribe")]
//             fn set_repo_subscribe(&self, Self::Metadata, pubsub::Subscriber<Vec<String>>, String);

//             #[rpc(name = "set_repo_unsubscribe")]
//             fn set_repo_unsubscribe(&self, SubscriptionId) -> Result<bool>;
//         }

// 		#[pubsub(name = "download")] {
// 			/// Hello subscription
// 			#[rpc(name = "download_subscribe")]
// 			fn download_subscribe(&self, Self::Metadata, pubsub::Subscriber<String>, String);

// 			/// Unsubscribe from hello subscription.
// 			#[rpc(name = "download_unsubscribe")]
// 			fn download_unsubscribe(&self, SubscriptionId) -> Result<bool>;
// 		}
// 	}
// }

// #[derive(Default)]
// struct RpcImpl {
// 	uid: atomic::AtomicUsize,
// 	active: Arc<RwLock<HashMap<SubscriptionId, pubsub::Sink<String>>>>,
// }
// impl Rpc for RpcImpl {
// 	type Metadata = Meta;

//     fn set_repo_subscribe(&self, _meta: Self::Metadata, subscriber: pubsub::Subscriber<Vec<String>>, repo_url: String) {
//         let id = self.uid.fetch_add(1, atomic::Ordering::SeqCst);
//         let sub_id = SubscriptionId::Number(id as u64);
// 		let sink = subscriber.assign_id(sub_id.clone()).unwrap();

//         thread::spawn(move || {
//             let res = match ::download_repository(&repo_url) {
//                 Ok(_) => Ok(Params::Array(vec![json!("OK")])),
//                 Err(e) => Err(Error {
//                     code: ErrorCode::ParseError,
//                     message: "Url was invalid.".into(),
//                     data: None   
//                 })
//             };
//             sink.notify(res).wait();
//         });
//     }

//     fn set_repo_unsubscribe(&self, id: SubscriptionId) -> Result<bool> {
//         unimplemented!()
//     }

// 	fn download_subscribe(&self, _meta: Self::Metadata, subscriber: pubsub::Subscriber<String>, package_id: String) {
// 		let id = self.uid.fetch_add(1, atomic::Ordering::SeqCst);
// 		let sub_id = SubscriptionId::Number(id as u64);
// 		let sink = subscriber.assign_id(sub_id.clone()).unwrap();
//         self.active.write().unwrap().insert(sub_id, sink);

//         // let sink = self.active.write().unwrap().get(&sub_id).unwrap();
//         // thread::spawn(move || {

//         //     // let mut res = reqwest::get(url_str).unwrap();

//         // });
// 	}

// 	fn download_unsubscribe(&self, id: SubscriptionId) -> Result<bool> {
// 		let removed = self.active.write().unwrap().remove(&id);
// 		if removed.is_some() {
// 			Ok(true)
// 		} else {
// 			Err(Error {
// 				code: ErrorCode::InvalidParams,
// 				message: "Invalid subscription.".into(),
// 				data: None,
// 			})
// 		}
// 	}
// }

pub fn start() {
    let mut io = PubSubHandler::default();
    let rpc = RpcImpl::default();
    let active_subscriptions = rpc.active.clone();

    io.add_subscription("set_repo",
        ("subscribe_set_repo", |params: Params, _, subscriber: Subscriber| {
            
        })

	// io.extend_with(rpc.to_delegate());

	let server = ServerBuilder::new(io)
		.session_meta_extractor(|context: &RequestContext| {
			Meta {
				session: Some(Arc::new(Session::new(context.sender.clone()))),
			}
		})
		.start(&"0.0.0.0:3030".parse().unwrap())
		.expect("Server must start with no issues");

    server.wait()
}