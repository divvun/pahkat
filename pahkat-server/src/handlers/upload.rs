use std::fs::{self, File};
use std::io::prelude::*;
use std::path::Path;

use actix_multipart::Multipart;
use actix_web::{http::header::AUTHORIZATION, web, HttpRequest, HttpResponse};
use chrono::Duration;
use form_data::{handle_multipart, Error};
use futures::future::{ok, Future};
use log::{error, info};
use serde::Deserialize;

use pahkat_common::{open_package, index_fn};
use pahkat_common::version::Version;
use pahkat_types::{Downloadable, Installer};

use crate::server::ServerState;

#[derive(Deserialize)]
struct UploadParams {
    pub channel: String,
    pub version: String,
    pub installer: Installer,
}

pub fn upload_package(
    request: HttpRequest,
    state: web::Data<ServerState>,
    path: web::Path<String>,
    multipart: Multipart,
) -> Box<dyn Future<Item=HttpResponse, Error=Error>> {
    let auth_header = request.headers().get(AUTHORIZATION);
    if auth_header == None {
        // TODO: Potentially use Forbidden, otherwise need to add auth header response
        return Box::new(ok(HttpResponse::Unauthorized().finish()));
    }

    let auth_header = auth_header.unwrap();

    let database = &state.database;

    let str_header = auth_header.to_str().unwrap();

    info!("Str header: {}", str_header);

    let split_vec: Vec<_> = str_header.split(' ').collect();
    if split_vec.len() != 2 || split_vec.get(0).unwrap() != &"Bearer" {
        info!("split vec: {:?}", split_vec);
        return Box::new(ok(HttpResponse::Unauthorized().finish()));
    }

    let result = database.validate_token(split_vec.get(1).unwrap());
    info!("db result: {:?}", &result);
    if !result.is_ok() && result.unwrap() == false {
        return Box::new(ok(HttpResponse::Unauthorized().finish()));
    }

    let ref_state = state.get_ref().clone();
    let mut repo_path = ref_state.path.clone();
    let mut destination_dir = ref_state.config.artifacts_dir.clone();
    let url_prefix = ref_state.config.url_prefix.clone();
    let form = ref_state.upload_form;

    info!("HttpRequest: {:?}", request);

    Box::new(
        handle_multipart(multipart, form).map(move |uploaded_content| {
            println!("execute");
            let mut map = uploaded_content.map().unwrap();
            let params = map.remove("params").unwrap().text().unwrap();

            let upload_params: Result<UploadParams, _> = serde_json::from_str(&params);
            match upload_params {
                Err(e) => {
                    return HttpResponse::BadRequest()
                        .body(format!("Error processing params: {}", e));
                }
                Ok(upload_params) => {
                    repo_path.push("packages");
                    repo_path.push(Path::new(&path.clone()));

                    info!(
                        "Repo path: {:?}, channel: {}",
                        &repo_path, &upload_params.channel
                    );
                    let mut channel: Option<&str> = None;
                    if upload_params.channel != "stable" {
                        channel = Some(&upload_params.channel);
                    }

                    let package_option = open_package(&repo_path, channel);
                    if let Err(err) = package_option {
                        error!("Error when opening {:?}: {:?}", &repo_path, err);
                        return HttpResponse::NotFound().finish();
                    }
                    let mut package = package_option.unwrap();

                    let current_version = Version::new(&package.version);
                    let incoming_version = Version::new(&upload_params.version);
                    info!(
                        "curr_ver: {:?}, incoming_ver: {:?}",
                        &current_version, &incoming_version
                    );

                    match (&current_version, &incoming_version) {
                        (Ok(_), Err(e)) => {
                            return HttpResponse::BadRequest()
                                .body(format!("Invalid version: {:?}: {:?}", &upload_params.version, e));
                        }
                        (Ok(current_version), Ok(incoming_version)) => {
                            if current_version > incoming_version {
                                return HttpResponse::Conflict().body(format!(
                                    "Incoming version less than current version: {:?} < {:?}",
                                    &incoming_version,
                                    &current_version
                                ));
                            } else {
                                match incoming_version {
                                    Version::UtcDate(incoming_date) => {
                                        let now = chrono::offset::Utc::now();
                                        let now = now.checked_add_signed(Duration::minutes(2)).unwrap();

                                        if incoming_date > &now {
                                            return HttpResponse::Conflict().body(format!(
                                                "Incoming date version is too far into the future: {:?}",
                                                &incoming_version
                                            ));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    };

                    let incoming_version = incoming_version.unwrap();

                    match &upload_params.installer {
                        Installer::Tarball(_) => return HttpResponse::BadRequest().body("Tarball installers not supported"),
                        installer => {
                            let mut url = installer.url();

                            info!("installer url: {}", url);

                            if url == "pahkat:payload" {
                                if !map.contains_key("payload") {
                                    return HttpResponse::BadRequest().body("payload required if `pahkat:payload` in uri");
                                }

                                let (filename, filepath) = map.remove("payload").unwrap().file().unwrap();

                                info!("text: {}", params);
                                info!(
                                    "filename: {}, path: {:?}",
                                    filename,
                                    filepath.as_path().display()
                                );

                                let copy_error = format!(
                                    "failed to copy temp file {:?} to artifacts dir {:?}",
                                    &filepath, &destination_dir
                                );

                                let final_filename;

                                match installer {
                                    Installer::Windows(installer) => {
                                        let mut ext = "exe";
                                        if let Some(installer_type) = &installer.installer_type {
                                            if installer_type == "msi" {
                                                ext = &installer_type;
                                            }
                                        }

                                        final_filename = format!("{}-{}.{}", package.id, incoming_version.to_string(), ext);
                                    }
                                    Installer::MacOS(_) => {
                                        final_filename = format!("{}-{}.pkg", package.id, incoming_version.to_string());
                                    }
                                    Installer::Tarball(_) => {
                                        return HttpResponse::Conflict().body("Previous package had Tarball installer");
                                    }
                                }

                                destination_dir.push(&final_filename);
                                url = format!("{}/{}", &url_prefix, final_filename);
                                fs::copy(&filepath, destination_dir).expect(&copy_error);
                            }

                            // Update the final package info
                            package.version = incoming_version.to_string();

                            // Should we use params installer?
                            let mut package_installer = package.installer.clone().unwrap();

                            match &mut package_installer {
                                Installer::Windows(installer) => {
                                    installer.url = url;
                                }
                                Installer::MacOS(installer) => {
                                    installer.url = url;
                                }
                                Installer::Tarball(_) => {
                                    return HttpResponse::Conflict().body("Previous package had Tarball installer");
                                }
                            }

                            package.installer = Some(package_installer);

                            let mut package_path = repo_path.clone();
                            package_path.push(index_fn(channel));

                            let json_package = serde_json::to_string_pretty(&package).unwrap();
                            let mut file = File::create(package_path).unwrap();
                            file.write_all(json_package.as_bytes()).unwrap();
                        }
                    };

                    HttpResponse::Created().finish()
                }
            }
        }),
    )
}
