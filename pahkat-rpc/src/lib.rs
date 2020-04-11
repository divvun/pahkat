#![recursion_limit = "1024"]

use futures::stream::TryStreamExt;
use std::convert::TryFrom;
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use parity_tokio_ipc::{Endpoint, SecurityAttributes};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::stream::StreamExt;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tonic::transport::server::Connected;
use tonic::{transport::Server, Request, Response, Status, Streaming};

use pahkat_client::{
    PackageAction, PackageActionType, PackageKey, PackageStore, PackageTransaction,
    PrefixPackageStore,
};
use std::convert::TryInto;
use std::sync::Arc;

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
    store: Arc<PrefixPackageStore>,
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

        let (tx, rx) = mpsc::channel(10);
        let store = Arc::clone(&self.store);

        tokio::spawn(async move {
            let mut tx = tx;

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

                    while let Some(event) = store.download_async(&action.id).next().await {
                        match event {
                            DownloadEvent::Error(e) => {
                                yield pb::TransactionResponse {
                                    value: Some(Value::DownloadError(DownloadError {
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

                let result = tokio::task::spawn_blocking(move || {
                    transaction.process(move |id, event| {
                        let mut tx = tx.clone();
                        tokio::spawn(async move {
                            let response = Ok(pb::TransactionResponse {
                                value: Some(Value::InstallStarted(InstallStarted {
                                    package_id: id.to_string(),
                                }))
                            });
                            tx.send(response).await.unwrap();
                        });
                        true
                    }).join().unwrap()
                }).await;

                match result {
                    Ok(_) => {},
                    Err(e) => {
                        yield pb::TransactionResponse {
                            value: Some(Value::InstallError(InstallError {
                                package_id: "<unknown>".to_string(), // TODO: add pkg
                                error: format!("{}", e)
                            }))
                        };
                        return;
                    }
                };

                yield pb::TransactionResponse {
                    value: Some(Value::TransactionComplete(TransactionComplete {}))
                };
            };

            futures::pin_mut!(stream);

            while let Some(value) = stream.next().await {
                tx.send(value).await.unwrap();
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

pub async fn start(
    path: String,
    config_path: &std::path::Path,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut endpoint = Endpoint::new(path);
    endpoint.set_security_attributes(SecurityAttributes::allow_everyone_create().unwrap());

    let incoming = endpoint.incoming().expect("failed to open new socket");
    let (sender, mut _rx) = broadcast::channel(5);

    let store = Arc::new(PrefixPackageStore::open_or_create(config_path)?);

    let rpc = Rpc {
        store,
        notifications: sender,
    };

    Server::builder()
        .add_service(pb::pahkat_server::PahkatServer::new(rpc))
        .serve_with_incoming(incoming.map_ok(StreamBox))
        .await?;

    Ok(())
}

#[derive(Debug)]
struct StreamBox<T: AsyncRead + AsyncWrite>(T);

impl<T: AsyncRead + AsyncWrite> Connected for StreamBox<T> {}

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncRead for StreamBox<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncWrite for StreamBox<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}
