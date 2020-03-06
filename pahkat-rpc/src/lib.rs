use futures::stream::TryStreamExt;
use futures::Stream;
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use parity_tokio_ipc::{Endpoint, SecurityAttributes};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tonic::transport::server::Connected;
use tonic::{transport::Server, Request, Response, Status, Streaming};

mod pb {
    tonic::include_proto!("/pahkat");
}

struct Rpc;

#[tonic::async_trait]
impl pb::pahkat_server::Pahkat for Rpc {
    type NotificationsStream = mpsc::Receiver<Result<pb::NotificationResponse, Status>>;
    type SelfUpdateStream = mpsc::Receiver<Result<pb::SelfUpdateResponse, Status>>;
    type ProcessTransactionStream = mpsc::Receiver<Result<pb::TransactionResponse, Status>>;

    async fn notifications(
        &self,
        _request: Request<pb::NotificationsRequest>,
    ) -> Result<Response<Self::NotificationsStream>, Status> {
        Err(Status::unimplemented(""))
    }

    async fn self_update(
        &self,
        _request: Request<pb::SelfUpdateRequest>,
    ) -> Result<Response<Self::SelfUpdateStream>, Status> {
        Err(Status::unimplemented(""))
    }

    async fn status(
        &self,
        _request: Request<pb::StatusRequest>,
    ) -> Result<Response<pb::StatusResponse>, Status> {
        Err(Status::unimplemented(""))
    }

    async fn repository_indexes(
        &self,
        _request: Request<pb::RepositoryIndexesRequest>,
    ) -> Result<Response<pb::RepositoryIndexesResponse>, Status> {
        Err(Status::unimplemented(""))
    }

    async fn process_transaction(
        &self,
        _request: Request<pb::TransactionRequest>,
    ) -> Result<Response<Self::ProcessTransactionStream>, Status> {
        Err(Status::unimplemented(""))
    }

    async fn refresh(
        &self,
        _request: Request<pb::RefreshRequest>,
    ) -> Result<Response<pb::RefreshResponse>, Status> {
        Err(Status::unimplemented(""))
    }

    async fn clear_cache(
        &self,
        _request: Request<pb::ClearCacheRequest>,
    ) -> Result<Response<pb::ClearCacheResponse>, Status> {
        Err(Status::unimplemented(""))
    }
}

pub async fn start(path: String) -> Result<(), Box<dyn std::error::Error>> {
    let mut endpoint = Endpoint::new(path);
    endpoint.set_security_attributes(SecurityAttributes::allow_everyone_create().unwrap());

    let incoming = endpoint.incoming().expect("failed to open new socket");

    Server::builder()
        .add_service(pb::pahkat_server::PahkatServer::new(Rpc))
        .serve_with_incoming(incoming.map_ok(StreamBox))
        .await?;

    Ok(())
}

#[derive(Debug)]
pub struct StreamBox<T: AsyncRead + AsyncWrite>(pub T);

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
