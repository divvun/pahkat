use std::io::prelude::*;
use std::fs::File;
use std::io::BufReader;

use actix_web::{web, HttpResponse, Responder};
use log::error;

use crate::ServerState;

fn read_file(path: &str) -> std::io::Result<String> {
    let file = File::open(path)?;
    let mut buf_reader = BufReader::new(file);
    let mut contents = String::new();
    buf_reader.read_to_string(&mut contents)?;

    Ok(contents)
}

pub fn repo_index(state: web::Data<ServerState>) -> impl Responder {
    let mut repo_index_path = state.path.clone();

    repo_index_path.push("index.json");

    match read_file(
        repo_index_path
            .to_str()
            .expect("Cannot convert path to string"),
    ) {
        Ok(body) => HttpResponse::Ok()
            .content_type("application/json")
            .body(body),
        Err(e) => {
            error!("Error while reading repo index file: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub fn packages_index(state: web::Data<ServerState>) -> impl Responder {
    let mut packages_index_path = state.path.clone();

    packages_index_path.push("packages");
    packages_index_path.push("index.json");

    match read_file(
        packages_index_path
            .to_str()
            .expect("Cannot convert path to string"),
    ) {
        Ok(body) => HttpResponse::Ok()
            .content_type("application/json")
            .body(body),
        Err(e) => {
            error!("Error while reading packages index file: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub fn packages_package_index(
    state: web::Data<ServerState>,
    path: web::Path<String>,
) -> impl Responder {
    let package_id = path.clone();

    let mut packages_package_index_path = state.path.clone();

    packages_package_index_path.push("packages");
    packages_package_index_path.push(package_id);
    packages_package_index_path.push("index.json");
    let index_path_str = packages_package_index_path
        .to_str()
        .expect("Cannot convert path to string");

    match read_file(index_path_str) {
        Ok(body) => HttpResponse::Ok()
            .content_type("application/json")
            .body(body),
        Err(e) => {
            error!(
                "Error while reading packages package index {}: {:?}",
                index_path_str, e
            );
            HttpResponse::NotFound().finish()
        }
    }
}

pub fn virtuals_index(state: web::Data<ServerState>) -> impl Responder {
    let mut virtuals_index_path = state.path.clone();

    virtuals_index_path.push("virtuals");
    virtuals_index_path.push("index.json");

    match read_file(
        virtuals_index_path
            .to_str()
            .expect("Cannot convert path to string"),
    ) {
        Ok(body) => HttpResponse::Ok()
            .content_type("application/json")
            .body(body),
        Err(e) => {
            error!("Error while reading virtuals index file: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub fn virtuals_package_index(
    state: web::Data<ServerState>,
    path: web::Path<String>,
) -> impl Responder {
    let package_id = path.clone();

    let mut virtuals_package_index_path = state.path.clone();

    virtuals_package_index_path.push("virtuals");
    virtuals_package_index_path.push(package_id);
    virtuals_package_index_path.push("index.json");
    let index_path_str = virtuals_package_index_path
        .to_str()
        .expect("Cannot convert path to string");

    match read_file(index_path_str) {
        Ok(body) => HttpResponse::Ok()
            .content_type("application/json")
            .body(body),
        Err(e) => {
            error!(
                "Error while reading virtuals package index {}: {:?}",
                index_path_str, e
            );
            HttpResponse::NotFound().finish()
        }
    }
}
