use futures::stream::{self, TryStreamExt};
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
use structopt::StructOpt;

use crate::pb;

#[derive(Debug, StructOpt)]
struct StatusCommand {
    package_id: String,
    target: String,
}

#[derive(Debug, StructOpt)]
struct RepoIndexesCommand {
}


#[derive(Debug, StructOpt)]
struct ProcessTransactionCommand {
    // package-id::action[::target]
    actions: Vec<String>,
}

#[derive(Debug, StructOpt)]
struct StringsCommand {
    category: String,
    language: String,
}

#[derive(Debug, StructOpt)]
enum Command {
    Status(StatusCommand),
    RepoIndexes(RepoIndexesCommand),
    ProcessTransaction(ProcessTransactionCommand),
    Strings(StringsCommand),
}

#[derive(Debug, StructOpt)]
struct Args {
    #[structopt(subcommand)]
    command: Command,
}

type PahkatClient = pb::pahkat_client::PahkatClient<tonic::transport::channel::Channel>;
use once_cell::sync::Lazy;
use std::sync::{RwLock, Arc};
use std::error::Error;
use std::path::PathBuf;

async fn new_client() -> anyhow::Result<PahkatClient> {
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

    let mut client = PahkatClient::new(channel);
    Ok(client)
}

pub async fn run() -> anyhow::Result<()> {
    let args = Args::from_args();
    let mut client = new_client().await?;

    match args.command {
        Command::Status(command) => {
            let request = Request::new(pb::StatusRequest {
                package_id: command.package_id,
                target: if command.target == "user" { 1 } else { 0 }
            });

            let response = client.status(request).await?;
            println!("{:#?}", response);
        }
        Command::RepoIndexes(_) => {
            let request = Request::new(pb::RepositoryIndexesRequest {
            });

            let response = client.repository_indexes(request).await?;
            println!("{:?}", response);
        }
        Command::ProcessTransaction(command) => {
            let actions = command.actions.into_iter().map(|s| {
                let mut s = s.split("::");
                let id = s.next().unwrap().to_string();
                let action = if s.next().unwrap_or_else(|| "install") == "install" {
                    0
                } else {
                    1
                };
                let target = if s.next().unwrap_or_else(|| "system") != "user" {
                    0
                } else {
                    1
                };
                pb::PackageAction {
                    id,
                    action,
                    target,
                }
            }).collect::<Vec<_>>();
                
            let req = stream::iter(vec![pb::TransactionRequest {
                value: Some(pb::transaction_request::Value::Transaction(
                    pb::transaction_request::Transaction { actions }
                ))
            }]);

            let request = Request::new(req);
            let stream = client.process_transaction(request).await?;

            let mut stream = stream.into_inner();

            while let Ok(Some(message)) = stream.message().await {
                println!("{:?}", message);
            }
        }
        Command::Strings(StringsCommand { category, language }) => {
            let request = Request::new(pb::StringsRequest {
                category,
                language
            });

            let response = client.strings(request).await?;
            println!("{:?}", response);
        }
        // Args::SetRepos
        // Args::Refresh
    }
    Ok(())
}

use serde::Serialize;
use cursed::{FromForeign, ToForeign, InputType, ReturnType};

pub struct JsonMarshaler;

impl InputType for JsonMarshaler {
    type Foreign = <cursed::StringMarshaler as InputType>::Foreign;
}

impl ReturnType for JsonMarshaler {
    type Foreign = cursed::Slice<u8>;

    fn foreign_default() -> Self::Foreign {
        cursed::Slice::default()
    }
}

impl<T> ToForeign<Result<T, Box<dyn Error>>, cursed::Slice<u8>> for JsonMarshaler
where
    T: Serialize,
{
    type Error = Box<dyn Error>;

    fn to_foreign(result: Result<T, Self::Error>) -> Result<cursed::Slice<u8>, Self::Error> {
        result.and_then(|input| {
            let json_str = serde_json::to_string(&input)?;
            Ok(cursed::StringMarshaler::to_foreign(json_str).unwrap())
        })
    }
}

#[cthulhu::invoke(return_marshaler = "cursed::ArcMarshaler::<RwLock<PahkatClient>>")]
pub extern "C" fn pahkat_rpc_new() -> Result<Arc<RwLock<PahkatClient>>, Box<dyn Error>> {
    let client = block_on(new_client())?;
    Ok(Arc::new(RwLock::new(client)))
}

#[cthulhu::invoke(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_rpc_repo_indexes(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<PahkatClient>>)]
    client: Arc<RwLock<PahkatClient>>
) -> Result<pb::RepositoryIndexesResponse, Box<dyn Error>> {
    let request = Request::new(pb::RepositoryIndexesRequest {});

    let mut client = client.write().unwrap();
    let response = block_on(client.repository_indexes(request))?;

    let response: pb::RepositoryIndexesResponse = response.into_inner();
    Ok(response)
}

// #[cthulhu::invoke]
// pub extern "C" fn pahkat_rpc_status(
//     #[marshal(cursed::ArcRefMarshaler::<PahkatClient>)]
//     handle: &Arc<PahkatClient>
// ) {

// }

// #[cthulhu::invoke]
// pub extern "C" fn pahkat_rpc_process_transaction(
//     #[marshal(cursed::ArcRefMarshaler::<PahkatClient>)]
//     handle: &Arc<PahkatClient>
// ) {

// }

static BASIC_RUNTIME: Lazy<RwLock<tokio::runtime::Runtime>> = Lazy::new(|| {
    RwLock::new(
        tokio::runtime::Builder::new()
            .threaded_scheduler()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime"),
    )
});

#[inline(always)]
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    BASIC_RUNTIME.write().unwrap().block_on(future)
}

// #[inline(always)]
// fn spawn<F: std::future::Future>(future: F) -> tokio::task::JoinHandle<F::Output> {
//     BASIC_RUNTIME.read().unwrap().spawn(future)
// }