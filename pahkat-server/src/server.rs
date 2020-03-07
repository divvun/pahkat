use actix_web::{middleware, web, App, HttpServer};
use form_data::{Field, FilenameGenerator, Form};
use log::info;
use pahkat_common::{database::Database, db_path};
use std::path::{Path, PathBuf};

use crate::config::TomlConfig;
use crate::handlers::{
    download_package, package_stats, packages_index, packages_index_stable, packages_package_index,
    packages_package_index_stable, repo_index, repo_stats, upload::upload_package, virtuals_index,
    virtuals_index_stable, virtuals_package_index, virtuals_package_index_stable,
};

struct UploadFilenameGenerator {
    directory: PathBuf,
}

impl FilenameGenerator for UploadFilenameGenerator {
    fn next_filename(&self, _: &mime::Mime) -> Option<PathBuf> {
        let random_fn = format!("{}.tmp", uuid::Uuid::new_v4().to_simple());
        Some(self.directory.join(random_fn))
    }
}

#[derive(Clone)]
pub struct ServerState {
    pub path: PathBuf,
    pub bind: String,
    pub port: String,
    pub config: TomlConfig,
    pub database: Database,
    pub upload_form: Form,
}

fn config_db_path(config: &TomlConfig) -> String {
    config
        .db_path
        .clone()
        .unwrap_or(db_path().as_path().to_str().unwrap().to_string())
}

embed_migrations!("../pahkat-common/migrations");

pub fn run_server(config: TomlConfig, path: &Path, bind: &str, port: &str) {
    let system = actix::System::new("páhkat-server");

    let database = match Database::new(&config_db_path(&config)) {
        Ok(database) => database,
        Err(e) => {
            panic!("Failed to create database: {}", e);
        }
    };

    info!("Running migrations");
    embedded_migrations::run(&database.get_connection().expect("connection to succeed"))
        .expect("migrations to run");

    let upload_tmp_path = path.join("upload-tmp");

    std::fs::create_dir_all(&upload_tmp_path).expect(&format!(
        "could not create upload temp directory {}",
        upload_tmp_path.as_path().display()
    ));

    std::fs::create_dir_all(&config.artifacts_dir).expect(&format!(
        "could not create artifacts directory {}",
        &config.artifacts_dir.as_path().display()
    ));

    // TODO(bbqsrc): Delete everything inside temp dir to ensure clean state
    // TODO(bbqsrc): Check the user access for the temp dir for security

    let form = Form::new().field("params", Field::text()).field(
        "payload",
        Field::file(UploadFilenameGenerator {
            directory: upload_tmp_path,
        }),
    );

    let state = ServerState {
        path: path.to_path_buf(),
        bind: bind.to_string(),
        port: port.to_string(),
        config,
        database,
        upload_form: form,
    };

    HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .wrap(middleware::Logger::default())
            .service(web::resource("/index.json").route(web::get().to(repo_index)))
            .service(web::resource("/repo/stats").route(web::get().to(repo_stats)))
            .service(
                web::scope("/packages")
                    .service(
                        web::resource("/index.json").route(web::get().to(packages_index_stable)),
                    )
                    .service(
                        web::resource("/index.{channel}.json").route(web::get().to(packages_index)),
                    )
                    .service(
                        web::resource("/{packageId}/index.json")
                            .route(web::get().to(packages_package_index_stable)),
                    )
                    .service(
                        web::resource("/{packageId}/index.{channel}.json")
                            .route(web::get().to(packages_package_index)),
                    )
                    .service(web::resource("/{packageId}").route(web::patch().to(upload_package)))
                    // TODO: add channel to download/stats endpoints
                    .service(
                        web::resource("/{packageId}/download")
                            .route(web::get().to(download_package)),
                    )
                    .service(
                        web::resource("/{packageId}/stats").route(web::get().to(package_stats)),
                    ),
            )
            .service(
                web::scope("/virtuals")
                    .service(
                        web::resource("/index.json").route(web::get().to(virtuals_index_stable)),
                    )
                    .service(
                        web::resource("/index.{channel}.json").route(web::get().to(virtuals_index)),
                    )
                    .service(
                        web::resource("/{packageId}/index.json")
                            .route(web::get().to(virtuals_package_index_stable)),
                    )
                    .service(
                        web::resource("/{packageId}/index.{channel}.json")
                            .route(web::get().to(virtuals_package_index)),
                    ),
            )
    })
    .bind(&format!("{}:{}", bind, port))
    .expect(&format!("Can not bind to {}:{}", bind, port))
    .start();

    let _ = system.run();
}
