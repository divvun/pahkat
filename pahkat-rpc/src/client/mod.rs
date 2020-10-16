use futures::stream::{self, TryStreamExt};
use futures::Stream;
use std::convert::TryFrom;
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use structopt::StructOpt;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tonic::transport::server::Connected;
use tonic::transport::{Endpoint, Uri};
use tonic::{transport::Server, Request, Response, Status, Streaming};
use tower::service_fn;

use crate::pb;

#[derive(Debug, StructOpt)]
struct StatusCommand {
    package_id: String,
    target: String,
}

#[derive(Debug, StructOpt)]
struct RepoIndexesCommand {}

#[derive(Debug, StructOpt)]
struct ProcessTransactionCommand {
    // package-id::action[::target]
    actions: Vec<String>,
}

#[derive(Debug, StructOpt)]
struct StringsCommand {
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
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

async fn new_client() -> anyhow::Result<PahkatClient> {
    let channel = Endpoint::try_from("file://tmp/pahkat")?
        .connect_with_connector(service_fn(|_: Uri| {
            let path = if cfg!(windows) {
                format!("//./pipe/pahkat")
            } else {
                format!("/tmp/pahkat")
            };

            parity_tokio_ipc::Endpoint::connect(path)
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
                target: if command.target == "user" { 1 } else { 0 },
            });

            let response = client.status(request).await?;
            println!("{:#?}", response);
        }
        Command::RepoIndexes(_) => {
            let request = Request::new(pb::RepositoryIndexesRequest {});

            let response = client.repository_indexes(request).await?;
            println!("{:?}", response);
        }
        Command::ProcessTransaction(command) => {
            let actions = command
                .actions
                .into_iter()
                .map(|s| {
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
                    pb::PackageAction { id, action, target }
                })
                .collect::<Vec<_>>();

            let req = stream::iter(vec![pb::TransactionRequest {
                value: Some(pb::transaction_request::Value::Transaction(
                    pb::transaction_request::Transaction { actions },
                )),
            }]);

            let request = Request::new(req);
            let stream = client.process_transaction(request).await?;

            let mut stream = stream.into_inner();

            while let Ok(Some(message)) = stream.message().await {
                println!("{:?}", message);
            }
        }
        Command::Strings(StringsCommand { language }) => {
            let request = Request::new(pb::StringsRequest { language });

            let response = client.strings(request).await?;
            println!("{:?}", response);
        }
    }
    Ok(())
}

use cffi::{FromForeign, InputType, ReturnType, ToForeign};
use serde::Serialize;

pub struct JsonMarshaler;

impl InputType for JsonMarshaler {
    type Foreign = <cffi::StringMarshaler as InputType>::Foreign;
}

impl ReturnType for JsonMarshaler {
    type Foreign = cffi::Slice<u8>;

    fn foreign_default() -> Self::Foreign {
        cffi::Slice::default()
    }
}

impl<T> ToForeign<Result<T, Box<dyn Error>>, cffi::Slice<u8>> for JsonMarshaler
where
    T: Serialize,
{
    type Error = Box<dyn Error>;

    fn to_foreign(result: Result<T, Self::Error>) -> Result<cffi::Slice<u8>, Self::Error> {
        result.and_then(|input| {
            let json_str = serde_json::to_string(&input)?;
            Ok(cffi::StringMarshaler::to_foreign(json_str).unwrap())
        })
    }
}

pub struct JsonRefMarshaler<'a>(&'a std::marker::PhantomData<()>);

impl<'a> InputType for JsonRefMarshaler<'a> {
    type Foreign = <cffi::StrMarshaler<'a> as InputType>::Foreign;
}

impl<'a, T> FromForeign<cffi::Slice<u8>, T> for JsonRefMarshaler<'a>
where
    T: serde::de::DeserializeOwned,
{
    type Error = Box<dyn Error>;

    unsafe fn from_foreign(ptr: cffi::Slice<u8>) -> Result<T, Self::Error> {
        let json_str =
            <cffi::StrMarshaler<'a> as FromForeign<cffi::Slice<u8>, &'a str>>::from_foreign(
                ptr,
            )?;
        log::debug!("JSON: {}, type: {}", &json_str, std::any::type_name::<T>());

        let v: Result<T, _> = serde_json::from_str(&json_str);
        v.map_err(|e| {
            log::error!("Json error: {}", &e);
            log::debug!("{:?}", &e);
            Box::new(e) as _
        })
    }
}

#[cffi::marshal(return_marshaler = "cffi::ArcMarshaler::<RwLock<PahkatClient>>")]
pub extern "C" fn pahkat_rpc_new() -> Result<Arc<RwLock<PahkatClient>>, Box<dyn Error>> {
    let client = block_on(new_client())?;
    Ok(Arc::new(RwLock::new(client)))
}

#[no_mangle]
pub extern "C" fn pahkat_rpc_free(ptr: *const RwLock<PahkatClient>) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        Arc::from_raw(ptr);
    }
}

#[no_mangle]
pub extern "C" fn pahkat_rpc_slice_free(slice: cffi::Slice<u8>) {
    unsafe {
        let _ = cffi::VecMarshaler::from_foreign(slice);
    }
}

#[cffi::marshal(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_rpc_repo_indexes(
    #[marshal(cffi::ArcRefMarshaler::<RwLock<PahkatClient>>)] client: Arc<RwLock<PahkatClient>>,
) -> Result<pb::RepositoryIndexesResponse, Box<dyn Error>> {
    let request = Request::new(pb::RepositoryIndexesRequest {});

    let response = block_on(async move {
        let mut client = client.write().await;
        let hold_on = client.repository_indexes(request).await;

        hold_on
    })?;

    let response = response.into_inner();
    Ok(response)
}

#[cffi::marshal]
pub extern "C" fn pahkat_rpc_status(
    #[marshal(cffi::ArcRefMarshaler::<RwLock<PahkatClient>>)] client: Arc<RwLock<PahkatClient>>,
    #[marshal(cffi::StrMarshaler::<'_>)] raw_package_key: &str,
    target: u8,
) -> i32 {
    let request = Request::new(pb::StatusRequest {
        package_id: raw_package_key.to_string(),
        target: target as u32,
    });

    block_on(async move {
        let mut client = client.write().await;
        let response = match client.status(request).await {
            Ok(v) => v,
            Err(_) => return 0,
        };
        response.into_inner().value
    })
}

#[no_mangle]
extern "C" fn pahkat_rpc_cancel_callback() {
    let mut tx = CURRENT_CANCEL_TX.lock().unwrap();
    let cb = tx.borrow_mut().take();
    match cb {
        Some(tx) => tx
            .send(pb::TransactionRequest {
                value: Some(pb::transaction_request::Value::Cancel(
                    pb::transaction_request::Cancel {},
                )),
            })
            .unwrap(),
        None => {
            // No problem.
        }
    }
}

#[cffi::marshal(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_rpc_get_repo_records(
    #[marshal(cffi::ArcRefMarshaler::<RwLock<PahkatClient>>)] client: Arc<RwLock<PahkatClient>>,
) -> Result<pb::GetRepoRecordsResponse, Box<dyn Error>> {
    let request = Request::new(pb::GetRepoRecordsRequest {});

    block_on(async move {
        let mut client = client.write().await;
        let response = client.get_repo_records(request).await.box_err()?;
        Ok(response.into_inner())
    })
}

#[cffi::marshal(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_rpc_set_repo(
    #[marshal(cffi::ArcRefMarshaler::<RwLock<PahkatClient>>)] client: Arc<RwLock<PahkatClient>>,
    #[marshal(cffi::StrMarshaler::<'_>)] repo_url: &str,
    #[marshal(JsonRefMarshaler)] settings: pb::RepoRecord,
) -> Result<pb::SetRepoResponse, Box<dyn Error>> {
    let request = Request::new(pb::SetRepoRequest {
        url: repo_url.to_string(),
        settings: Some(settings),
    });

    let result = block_on(async move {
        let mut client = client.write().await;
        let response = client.set_repo(request).await.box_err()?;
        Ok(response.into_inner())
    });

    eprintln!("RESULT: {:?}", &result);

    result
}

#[cffi::marshal(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_rpc_remove_repo(
    #[marshal(cffi::ArcRefMarshaler::<RwLock<PahkatClient>>)] client: Arc<RwLock<PahkatClient>>,
    #[marshal(cffi::StrMarshaler::<'_>)] repo_url: &str,
) -> Result<pb::RemoveRepoResponse, Box<dyn Error>> {
    let request = Request::new(pb::RemoveRepoRequest {
        url: repo_url.to_string(),
    });

    block_on(async move {
        let mut client = client.write().await;
        let response = client.remove_repo(request).await.box_err()?;
        Ok(response.into_inner())
    })
}

#[cffi::marshal]
pub extern "C" fn pahkat_rpc_notifications(
    #[marshal(cffi::ArcRefMarshaler::<RwLock<PahkatClient>>)] client: Arc<RwLock<PahkatClient>>,
    callback: extern "C" fn(i32),
) {
    let request = Request::new(pb::NotificationsRequest {});

    spawn(async move {
        let mut stream = {
            let mut client = client.write().await;
            let stream = client.notifications(request).await.unwrap();
            stream.into_inner()
        };

        while let Ok(Some(message)) = stream.message().await {
            unsafe {
                (callback)(message.value as i32);
            };
        }
    });
}

#[cffi::marshal(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_rpc_strings(
    #[marshal(cffi::ArcRefMarshaler::<RwLock<PahkatClient>>)] client: Arc<RwLock<PahkatClient>>,
    #[marshal(cffi::StrMarshaler::<'_>)] language_tag: &str,
) -> Result<pb::StringsResponse, Box<dyn Error>> {
    let request = Request::new(pb::StringsRequest {
        language: language_tag.to_string(),
    });

    let response = block_on(async move {
        let mut client = client.write().await;
        let response = client.strings(request).await.box_err()?;
        Ok(response.into_inner())
    });

    response
}

#[cffi::marshal(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_rpc_resolve_package_query(
    #[marshal(cffi::ArcRefMarshaler::<RwLock<PahkatClient>>)] client: Arc<RwLock<PahkatClient>>,
    #[marshal(cffi::StrMarshaler::<'_>)] package_query: &str,
) -> Result<pahkat_client::transaction::ResolvedPackageQuery, Box<dyn Error>> {
    let request = Request::new(pb::JsonRequest {
        json: package_query.to_string(),
    });

    let response: Result<pb::JsonResponse, Box<dyn Error>> = block_on(async move {
        let mut client = client.write().await;
        let response = client.resolve_package_query(request).await.box_err()?;
        Ok(response.into_inner())
    });

    serde_json::from_str(&response?.json).box_err()
}

#[cffi::marshal(return_marshaler = "cffi::UnitMarshaler")]
pub extern "C" fn pahkat_rpc_process_transaction(
    #[marshal(cffi::ArcRefMarshaler::<RwLock<PahkatClient>>)] client: Arc<RwLock<PahkatClient>>,

    #[marshal(JsonRefMarshaler)] actions: Vec<pb::PackageAction>,

    callback: extern "C" fn(cffi::Slice<u8>),
) -> Result<(), Box<dyn Error>> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    let mut global_tx = CURRENT_CANCEL_TX.lock().unwrap();
    *global_tx.borrow_mut() = Some(tx.clone());

    let request = Request::new(rx);

    spawn(async move {
        let mut stream = {
            let mut client = client.write().await;
            let stream = client.process_transaction(request).await.unwrap();
            stream.into_inner()
        };

        while let Ok(Some(message)) = stream.message().await {
            let cb_response = message.value.unwrap();
            let s = serde_json::to_string(&cb_response).unwrap();
            let bytes = s.as_bytes();

            unsafe {
                (callback)(cffi::Slice {
                    data: bytes.as_ptr() as *mut _,
                    len: bytes.len(),
                });
            };
        }
    });

    tx.send(pb::TransactionRequest {
        value: Some(pb::transaction_request::Value::Transaction(
            pb::transaction_request::Transaction { actions },
        )),
    })?;

    // Ok(pahkat_rpc_cancel_callback)

    Ok(())
}

static CURRENT_CANCEL_TX: Lazy<
    std::sync::Mutex<
        std::cell::RefCell<Option<tokio::sync::mpsc::UnboundedSender<pb::TransactionRequest>>>,
    >,
> = Lazy::new(|| std::sync::Mutex::new(std::cell::RefCell::new(None)));

static BASIC_RUNTIME: Lazy<std::sync::RwLock<tokio::runtime::Runtime>> = Lazy::new(|| {
    std::sync::RwLock::new(
        tokio::runtime::Builder::new()
            .threaded_scheduler()
            // .basic_scheduler()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime"),
    )
});

#[inline(always)]
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    let rt = BASIC_RUNTIME.read().unwrap();
    let handle = rt.handle();
    handle.block_on(future)
}

#[inline(always)]
fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send,
{
    let rt = BASIC_RUNTIME.read().unwrap();
    let handle = rt.handle();
    handle.spawn(future)
}

trait BoxError {
    type Item;

    fn box_err(self) -> Result<Self::Item, Box<dyn Error>>;
}

impl<T, E: std::error::Error + 'static> BoxError for Result<T, E> {
    type Item = T;

    #[inline(always)]
    fn box_err(self) -> Result<Self::Item, Box<dyn Error>> {
        self.map_err(|e| Box::new(e) as _)
    }
}
