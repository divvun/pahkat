#![recursion_limit = "1024"]

use std::convert::{TryFrom, TryInto};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::stream::{StreamExt, TryStreamExt};
use parity_tokio_ipc::{Endpoint, SecurityAttributes};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tonic::transport::server::Connected;
use tonic::{transport::Server, Request, Response, Status, Streaming};

use pahkat_client::{
    PackageAction, PackageActionType, PackageKey, PackageStore, PackageTransaction,
};

mod pb {
    tonic::include_proto!("/pahkat");
}

impl From<pb::PackageAction> for PackageAction {
    fn from(input: pb::PackageAction) -> PackageAction {
        PackageAction {
            id: PackageKey::try_from(&*input.id).unwrap(),
            action: PackageActionType::from_u8(input.action as u8),
            target: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
enum Notification {
    Test,
}

type Result<T> = std::result::Result<Response<T>, Status>;
type Stream<T> =
    Pin<Box<dyn futures::Stream<Item = std::result::Result<T, Status>> + Send + Sync + 'static>>;

struct Rpc {
    store: Arc<dyn PackageStore>,
    notifications: broadcast::Sender<Notification>,
}

#[tonic::async_trait]
impl pb::pahkat_server::Pahkat for Rpc {
    type NotificationsStream = Stream<pb::NotificationResponse>;
    // type SelfUpdateStream = Stream<pb::SelfUpdateResponse>;

    async fn notifications(
        &self,
        _request: Request<pb::NotificationsRequest>,
    ) -> Result<Self::NotificationsStream> {
        let mut rx = self.notifications.subscribe();

        log::info!("Peer: {:?}", _request.peer_cred());

        let stream = async_stream::try_stream! {
            while let response = rx.recv().await {
                match response {
                    Ok(response) => {
                        match response {
                            Notification::Test => {
                                yield pb::NotificationResponse { value: Some(pb::notification_response::Value::Message("hello".into())) };
                            },
                        }
                    },
                    Err(err) => {
                        break;
                    }
                }
            }
        };

        Ok(Response::new(Box::pin(stream) as Self::NotificationsStream))
    }

    // async fn self_update(
    //     &self,
    //     _request: Request<pb::SelfUpdateRequest>,
    // ) -> Result<Self::SelfUpdateStream> {
    //     Err(Status::unimplemented(""))
    // }

    async fn status(&self, request: Request<pb::StatusRequest>) -> Result<pb::StatusResponse> {
        let request = request.into_inner();
        let package_id = PackageKey::try_from(&*request.package_id)
            .map_err(|e| Status::failed_precondition(format!("{}", e)))?;

        let result = pahkat_client::transaction::status_to_i8(
            self.store.status(&package_id, Default::default()),
        );
        Ok(Response::new(pb::StatusResponse {
            value: result.try_into().unwrap(),
        }))
    }

    // async fn repository_indexes(
    //     &self,
    //     _request: Request<pb::RepositoryIndexesRequest>,
    // ) -> Result<pb::RepositoryIndexesResponse> {
    //     Err(Status::unimplemented(""))
    // }

    type ProcessTransactionStream = Stream<pb::TransactionResponse>;

    async fn process_transaction(
        &self,
        request: Request<pb::TransactionRequest>,
    ) -> Result<Self::ProcessTransactionStream> {
        let request = request.into_inner();
        let actions = request
            .actions
            .into_iter()
            .map(|x| PackageAction::from(x))
            .collect::<Vec<_>>();
        println!("{:?}", &actions);

        let transaction = PackageTransaction::new(Arc::clone(&self.store as _) as _, actions)
            .map_err(|e| Status::failed_precondition(format!("{}", e)))?;

        let (tx, rx) = mpsc::unbounded_channel();
        let store = Arc::clone(&self.store);

        tokio::spawn(async move {
            let tx = tx;

            let tx1 = tx.clone();
            let stream = async_stream::try_stream! {
                let tx = tx1;
                use pahkat_client::package_store::DownloadEvent;
                use pb::transaction_response::*;

                yield pb::TransactionResponse {
                    value: Some(Value::TransactionStarted(TransactionStarted {}))
                };

                for action in transaction.actions().iter() {
                    let tx = tx.clone();
                    let id = action.id.clone();
                    let mut download = store.download(&action.id);

                    while let Some(event) = download.next().await {
                        match event {
                            DownloadEvent::Error(e) => {
                                yield pb::TransactionResponse {
                                    value: Some(Value::DownloadError(TransactionError {
                                        package_id: id.to_string(),
                                        error: format!("{}", e)
                                    }))
                                };
                                return;
                            }
                            DownloadEvent::Progress((current, total)) => {
                                yield pb::TransactionResponse {
                                    value: Some(Value::DownloadProgress(DownloadProgress {
                                        package_id: id.to_string(),
                                        current,
                                        total,
                                    }))
                                };
                            }
                            DownloadEvent::Complete(_) => {
                                yield pb::TransactionResponse {
                                    value: Some(Value::DownloadComplete(DownloadComplete {
                                        package_id: id.to_string(),
                                    }))
                                };
                            }
                        }
                    }
                }

                let (canceler, mut tx_stream) = transaction.process();
                let mut is_completed = false;

                while let Some(event) = tx_stream.next().await {
                    use pahkat_client::transaction::TransactionEvent;

                    match event {
                        TransactionEvent::Installing(id) => {
                            yield pb::TransactionResponse {
                                value: Some(Value::InstallStarted(InstallStarted {
                                    package_id: id.to_string(),
                                }))
                            };
                        }
                        TransactionEvent::Uninstalling(id) => {
                            yield pb::TransactionResponse {
                                value: Some(Value::UninstallStarted(UninstallStarted {
                                    package_id: id.to_string(),
                                }))
                            };
                        }
                        TransactionEvent::Progress(id, msg) => {
                            yield pb::TransactionResponse {
                                value: Some(Value::TransactionProgress(TransactionProgress {
                                    package_id: id.to_string(),
                                    message: msg,
                                    current: 0,
                                    total: 0,
                                }))
                            };
                        }
                        TransactionEvent::Error(id, err) => {
                            yield pb::TransactionResponse {
                                value: Some(Value::TransactionError(TransactionError {
                                    package_id: id.to_string(),
                                    error: format!("{}", err),
                                }))
                            };
                            return;
                        }
                        TransactionEvent::Complete => {
                            yield pb::TransactionResponse {
                                value: Some(Value::TransactionComplete(TransactionComplete {}))
                            };
                            is_completed = true;
                        }
                    }
                }

                if !is_completed {
                    yield pb::TransactionResponse {
                        value: Some(Value::TransactionError(TransactionError {
                            package_id: "".to_string(),
                            error: "user cancelled".to_string(),
                        }))
                    };
                }

                // let result = tokio::task::spawn_blocking(move || {
                //     transaction.process(move |id, event| {
                //         let mut tx = tx.clone();
                //         tokio::spawn(async move {
                //             let response = Ok(pb::TransactionResponse {
                //                 value: Some(Value::InstallStarted(InstallStarted {
                //                     package_id: id.to_string(),
                //                 }))
                //             });
                //             tx.send(response).unwrap();
                //         });
                //         true
                //     }).join().unwrap()
                // }).await;

                // match result {
                //     Ok(_) => {},
                //     Err(e) => {
                //         yield pb::TransactionResponse {
                //             value: Some(Value::InstallError(InstallError {
                //                 package_id: "<unknown>".to_string(), // TODO: add pkg
                //                 error: format!("{}", e)
                //             }))
                //         };
                //         return;
                //     }
                // };

                // yield pb::TransactionResponse {
                //     value: Some(Value::TransactionComplete(TransactionComplete {}))
                // };
            };

            futures::pin_mut!(stream);

            while let Some(value) = stream.next().await {
                match tx.send(value) {
                    Ok(_) => {},
                    Err(err) => {
                        log::error!("{:?}", err);
                    }
                }
            }
        });

        Ok(Response::new(Box::pin(rx) as Self::ProcessTransactionStream))
    }

    // async fn refresh(
    //     &self,
    //     request: Request<pb::RefreshRequest>,
    // ) -> Result<pb::RefreshResponse> {
    //     log::debug!("refresh: {:?}", &request);
    //     self.notifications.send(Notification::Test).unwrap();

    //     Ok(tonic::Response::new(pb::RefreshResponse {}))
    // }

    // async fn clear_cache(
    //     &self,
    //     _request: Request<pb::ClearCacheRequest>,
    // ) -> Result<pb::ClearCacheResponse> {
    //     Err(Status::unimplemented(""))
    // }
}

use std::path::Path;

#[inline(always)]
#[cfg(feature = "prefix")]
async fn store(config_path: Option<&Path>) -> anyhow::Result<Arc<dyn PackageStore>> {
    let config_path = config_path.ok_or_else(|| anyhow::anyhow!("No prefix path specified"))?;
    let store = pahkat_client::PrefixPackageStore::open(config_path)?;
    let store = Arc::new(store);

    if store.config().read().unwrap().repos().len() == 0 {
        println!("WARNING: There are no repositories in the given config.");
    }

    Ok(store)
}

#[inline(always)]
#[cfg(feature = "macos")]
async fn store(config_path: Option<&Path>) -> anyhow::Result<Arc<dyn PackageStore>> {
    let config = match config_path {
        Some(v) => pahkat_client::Config::load(&v, pahkat_client::Permission::ReadWrite)?,
        None => pahkat_client::Config::load_default()?,
    };
    let store = pahkat_client::MacOSPackageStore::new(config).await;
    let store = Arc::new(store);

    if store.config().read().unwrap().repos().len() == 0 {
        println!("WARNING: There are no repositories in the given config.");
    }

    Ok(store)
}


pub async fn start(
    path: String,
    config_path: Option<&Path>,
) -> std::result::Result<(), anyhow::Error> {
    let mut endpoint = tokio::net::UnixListener::bind(path).unwrap();
    // let mut endpoint = Endpoint::new(path);
    // endpoint.set_security_attributes(SecurityAttributes::empty().allow_everyone_connect().unwrap());

    let incoming = endpoint.incoming()
        .map(|x| {
            match x {
                Ok(v) => {
                    log::debug!("PEER: {:?}", &v.peer_cred());
                    Ok(v)
                },
                Err(e) => Err(e)
            }
        });//.expect("failed to open new socket");
    let (sender, mut _rx) = broadcast::channel(5);

    let store = store(config_path).await?;
    log::debug!("Created store.");

    let rpc = Rpc {
        store,
        notifications: sender,
    };

    let sigint_listener = signal(SignalKind::interrupt())?.into_future();
    let sigterm_listener = signal(SignalKind::terminate())?.into_future();
    let sigquit_listener = signal(SignalKind::quit())?.into_future();

    // SIGUSR1 and SIGUSR2 do nothing, just swallow events.
    let _sigusr1_listener = signal(SignalKind::user_defined1())?.into_future();
    let _sigusr2_listener = signal(SignalKind::user_defined2())?.into_future();

    log::debug!("Created signal listeners for: SIGINT, SIGTERM, SIGQUIT, SIGUSR1, SIGUSR2.");

    Server::builder()
        .add_service(pb::pahkat_server::PahkatServer::new(rpc))
        .serve_with_incoming_shutdown(incoming, async move {
            tokio::select! {
                _ = sigint_listener => {
                    log::info!("SIGINT received; gracefully shutting down.");
                },
                _ = sigterm_listener => {
                    log::info!("SIGTERM received; gracefully shutting down.");
                }
                _ = sigquit_listener => {
                    log::info!("SIGQUIT received; gracefully shutting down.");
                }
            };
            ()
        })
        .await?;

    Ok(())
}

// #[derive(Debug)]
// struct StreamBox<T: AsyncRead + AsyncWrite>(T);

// impl<T: AsyncRead + AsyncWrite> Connected for StreamBox<T> {}

// impl<T: AsyncRead + AsyncWrite + Unpin> AsyncRead for StreamBox<T> {
//     fn poll_read(
//         mut self: Pin<&mut Self>,
//         cx: &mut Context<'_>,
//         buf: &mut [u8],
//     ) -> Poll<std::io::Result<usize>> {
//         Pin::new(&mut self.0).poll_read(cx, buf)
//     }
// }

// impl<T: AsyncRead + AsyncWrite + Unpin> AsyncWrite for StreamBox<T> {
//     fn poll_write(
//         mut self: Pin<&mut Self>,
//         cx: &mut Context<'_>,
//         buf: &[u8],
//     ) -> Poll<std::io::Result<usize>> {
//         Pin::new(&mut self.0).poll_write(cx, buf)
//     }

//     fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
//         Pin::new(&mut self.0).poll_flush(cx)
//     }

//     fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
//         Pin::new(&mut self.0).poll_shutdown(cx)
//     }
// }
