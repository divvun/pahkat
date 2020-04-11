use futures::stream::TryStreamExt;
use futures::Stream;
use parity_tokio_ipc::Endpoint as IpcEndpoint;
use std::convert::TryFrom;
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tonic::transport::server::Connected;
use tonic::transport::{Endpoint, Uri};
use tonic::{transport::Server, Request, Response, Status, Streaming};
use tower::service_fn;

use pahkat_client::{PackageAction, PackageKey};

mod pb {
    tonic::include_proto!("/pahkat");
}

impl From<PackageAction<()>> for pb::PackageAction {
    fn from(action: PackageAction<()>) -> pb::PackageAction {
        pb::PackageAction {
            id: action.id.to_string(),
            action: action.action.to_u8() as u32,
            target: 0,
        }
    }
}


#[cthulhu::invoke]
pub extern "C" fn pahkat_rpc_new() {

}

#[cthulhu::invoke]
pub extern "C" fn pahkat_rpc_notifications(handle: TODO) {

}

#[cthulhu::invoke]
pub extern "C" fn pahkat_rpc_repo_indexes(handle: TODO) {

}

#[cthulhu::invoke]
pub extern "C" fn pahkat_rpc_status(handle: TODO) {

}

#[cthulhu::invoke]
pub extern "C" fn pahkat_rpc_process_transaction(handle: TODO) {

}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let channel = Endpoint::try_from("file://tmp/pahkat")?
        .connect_with_connector(service_fn(|_: Uri| {
            let path = if cfg!(windows) {
                format!("//./pipe/pahkat")
            } else {
                format!("/tmp/pahkat")
            };

            IpcEndpoint::connect(path)
        }))
        .await?;

    let mut client = pb::pahkat_client::PahkatClient::new(channel);

    let stream = client
        .notifications(tonic::Request::new(pb::NotificationsRequest {}))
        .await?;
    let mut stream = stream.into_inner();

    tokio::spawn(async move {
        while let Ok(Some(response)) = stream.message().await {
            println!("RESPONSE={:?}", response);
        }
    });

    let request = tonic::Request::new(pb::TransactionRequest {
        actions: vec![PackageAction::install(
            PackageKey::try_from("https://test.com/packages/woo?platform=nope").unwrap(),
            (),
        )
        .into()],
    });
    let stream = client.process_transaction(request).await?;
    let mut stream = stream.into_inner();

    // let req = tonic::Request::new(pb::RefreshRequest {});
    // let response = client.refresh(req).await?;
    // let req = tonic::Request::new(pb::RefreshRequest {});
    // let response = client.refresh(req).await?;
    // let req = tonic::Request::new(pb::RefreshRequest {});
    // let response = client.refresh(req).await?;

    Ok(())
}
