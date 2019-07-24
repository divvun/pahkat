use std::fs::{self, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use actix_multipart::Multipart;
use actix_web::{http::header::AUTHORIZATION, web, HttpRequest, HttpResponse};
use chrono::Duration;
use form_data::{handle_multipart, Error, Value};
use futures::future::{ok, Future};
use log::{debug, error, info};
use serde::Deserialize;

use pahkat_common::database::Database;
use pahkat_common::version::Version;
use pahkat_common::{index_fn, open_package};
use pahkat_types::{Downloadable, Installer, Package};

use crate::server::ServerState;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;

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
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    debug!("HttpRequest: {:?}", request);

    let authorization_result = authorize(&request, &state.database);
    if authorization_result.is_err() {
        return authorization_result.unwrap_err();
    }

    let ref_state = state.get_ref().clone();
    let mut repo_path = ref_state.path.clone();
    let mut destination_dir = ref_state.config.artifacts_dir.clone();
    let url_prefix = ref_state.config.url_prefix.clone();
    let form = ref_state.upload_form;

    let final_result = handle_multipart(multipart, form).map(move |uploaded_content| {
        let mut map = uploaded_content.map().unwrap();
        let params = map.remove("params").unwrap().text().unwrap();

        let upload_params: Result<UploadParams, _> = serde_json::from_str(&params);
        if let Err(e) = upload_params {
            return HttpResponse::BadRequest().body(format!("Error processing params: {}", e));
        }

        let upload_params = upload_params.unwrap();

        repo_path.push("packages");
        repo_path.push(Path::new(&path.clone()));

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

        let version_validation = validate_version(&package.version, &upload_params.version);
        if version_validation.is_err() {
            return version_validation.unwrap_err();
        }
        let incoming_version = version_validation.unwrap();

        match &upload_params.installer {
            Installer::Tarball(_) => {
                return HttpResponse::BadRequest().body("Tarball installers not supported")
            }
            installer => {
                let mut url = installer.url();

                info!("installer url: {}", url);

                if url == "pahkat:payload" {
                    let filename = get_filename(&package, &installer, &incoming_version);
                    if filename.is_err() {
                        return filename.unwrap_err();
                    }
                    let filename = filename.unwrap();

                    let payload_result =
                        copy_payload_and_get_url(&mut map, &mut destination_dir, &filename);

                    if payload_result.is_err() {
                        return payload_result.unwrap_err();
                    }

                    url = format!("{}/{}", &url_prefix, filename);
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
                        return HttpResponse::Conflict()
                            .body("Previous package had Tarball installer");
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
    });

    Box::new(final_result)
}

fn authorize(
    request: &HttpRequest,
    database: &Database,
) -> Result<(), Box<dyn Future<Item = HttpResponse, Error = Error>>> {
    let auth_header = request.headers().get(AUTHORIZATION);
    if auth_header == None {
        return Err(Box::new(ok(
            HttpResponse::Unauthorized().body("No Authorization header found")
        )));
    }

    let auth_header = auth_header
        .unwrap()
        .to_str()
        .expect("header to be readable as string");

    let split_vec: Vec<_> = auth_header.split(' ').collect();
    if split_vec.len() != 2 || split_vec.get(0).unwrap() != &"Bearer" {
        return Err(Box::new(ok(
            HttpResponse::Unauthorized().body("No bearer token provided")
        )));
    }

    let result = database.validate_token(split_vec.get(1).unwrap());

    match result {
        Err(e) => {
            error!("Error when processing token: {:?}", e);
            return Err(Box::new(ok(
                HttpResponse::Unauthorized().body("Failed to validate token")
            )));
        }
        Ok(valid) => {
            if valid {
                Ok(())
            } else {
                return Err(Box::new(ok(
                    HttpResponse::Unauthorized().body("User not authorized for this action")
                )));
            }
        }
    }
}

fn validate_version(
    current_version: &str,
    incoming_version: &str,
) -> Result<Version, HttpResponse> {
    let current_version = Version::new(&current_version);
    let incoming_version = Version::new(&incoming_version);

    match (&current_version, &incoming_version) {
        (Ok(_), Err(e)) => {
            return Err(HttpResponse::BadRequest()
                .body(format!("Invalid version: {:?}: {:?}", &current_version, e)));
        }
        (Ok(current_version), Ok(incoming_version)) => {
            if current_version > incoming_version {
                return Err(HttpResponse::Conflict().body(format!(
                    "Incoming version less than current version: {:?} < {:?}",
                    &incoming_version, &current_version
                )));
            } else {
                match incoming_version {
                    Version::UtcDate(incoming_date) => {
                        let now = chrono::offset::Utc::now();
                        let now = now.checked_add_signed(Duration::minutes(2)).unwrap();

                        if incoming_date > &now {
                            return Err(HttpResponse::Conflict().body(format!(
                                "Incoming date version is too far into the future: {:?}",
                                &incoming_version
                            )));
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    Ok(incoming_version.unwrap())
}

fn copy_payload_and_get_url(
    map: &mut HashMap<String, Value, RandomState>,
    destination_dir: &mut PathBuf,
    filename: &str,
) -> Result<(), HttpResponse> {
    if !map.contains_key("payload") {
        return Err(HttpResponse::BadRequest().body("payload required if `pahkat:payload` in uri"));
    }

    let (_, filepath) = map.remove("payload").unwrap().file().unwrap();

    let copy_error = format!(
        "failed to copy temp file {:?} to artifacts dir {:?}",
        &filepath, &destination_dir
    );

    destination_dir.push(&filename);
    fs::copy(&filepath, destination_dir).expect(&copy_error);

    Ok(())
}

fn get_filename(
    package: &Package,
    installer: &Installer,
    version: &Version,
) -> Result<String, HttpResponse> {
    match installer {
        Installer::Windows(installer) => {
            let mut ext = "exe";
            if let Some(installer_type) = &installer.installer_type {
                if installer_type == "msi" {
                    ext = &installer_type;
                }
            }

            Ok(format!("{}-{}.{}", package.id, version.to_string(), ext))
        }
        Installer::MacOS(_) => Ok(format!("{}-{}.pkg", package.id, version.to_string())),
        Installer::Tarball(_) => {
            Err(HttpResponse::Conflict().body("Previous package had Tarball installer"))
        }
    }
}
