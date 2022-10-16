//mod r#static;
//pub use r#static::coordinates;

use super::{State, Tram};

use actix_web::{http::header, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use log::{info, debug};

use utoipa::ToSchema;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, ToSchema)]
pub struct EntireNetworkResponse {
    network: HashMap<u32, HashMap<u32, Tram>>,
    time_stamp: u64,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct RequestVehicleInformation {
    line: u32,
    run: u32,
}


/// this endpoint returnes last seen position 
#[utoipa::path(
    get,
    path = "/vehicles/{region}/all",
    responses(
        (status = 200, description = "return all the vehicles in the requested region", body = EntireNetworkResponse),
        (status = 500, description = "postgres pool error")
    ),
)]
pub async fn get_network(
    state: web::Data<Arc<RwLock<State>>>,
    region: web::Path<i32>,
) -> impl Responder {
    //let unwrapped_region = region.into_inner();
    info!("Received Request with {}", &region);

    let data = state.read().unwrap();
    match data.regions.get(&region) {
        Some(region) => {
            let start = SystemTime::now();
            let since_the_epoch = start
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs();

            let mut region_copy = region.lines.clone();

            for (_, runs) in region_copy.iter_mut() {
                runs.retain(|_, v| since_the_epoch - v.time_stamp < 300);
            }

            HttpResponse::Ok()
                .insert_header(header::ContentType::json())
                .json(EntireNetworkResponse {
                    network: region_copy,
                    time_stamp: since_the_epoch,
                })
        }
        None => HttpResponse::BadRequest().finish(),
    }
}

/// this endpoint is for finding vehicles
#[utoipa::path(
    post,
    path = "/vehicles/{region}/all",
    responses(
        (status = 200, description = "inforamtion about the requested tram or bus", body = RequestVehicleInformation),
        (status = 500, description = "postgres pool error")
    ),
)]
pub async fn query_vehicle(
    state: web::Data<Arc<RwLock<State>>>,
    region: web::Path<i32>,
    request: web::Json<RequestVehicleInformation>,
) -> impl Responder {
    //let unwrapped_region = region.into_inner();
    info!("Received Request with {}", &region);

    let data = state.read().unwrap();
    match data.regions.get(&region) {
        Some(region) => {
            if region.lines.contains_key(&request.line)
                && region
                    .lines
                    .get(&request.line)
                    .unwrap()
                    .contains_key(&request.run)
            {
                HttpResponse::Ok()
                    .insert_header(header::ContentType::json())
                    .json(
                        region
                            .lines
                            .get(&request.line)
                            .unwrap()
                            .get(&request.run)
                            .unwrap(),
                    )
            } else {
                HttpResponse::BadRequest().finish()
            }
        }
        None => HttpResponse::BadRequest().finish(),
    }
}

/// this endpoint returnes a list of interpolated gps positions and the average
/// time that is needed to traverse them.
#[utoipa::path(
    post,
    path = "/vehicles/{region}/position",
    responses(
        (status = 200, description = "information about the tram/bus the time and list of gps postions", body = RequestVehicleInformation),
        (status = 500, description = "postgres pool error")
    ),
)]
pub async fn get_position(
    state: web::Data<Arc<RwLock<State>>>,
    region: web::Path<i32>,
    request: web::Json<RequestVehicleInformation>,
) -> impl Responder {

    let data = state.read().unwrap();

    match data.regions.get(&region) {
        Some(region) => {
            // found network for requested city
            //
            let tram = match region.lines.get(&request.line) {
                Some(runs) => {
                    match runs.get(&request.run) {
                        Some(vehicle) => vehicle,
                        None => {
                            debug!("line {} found but not the run {}", request.line, request.run);
                            return HttpResponse::BadRequest().finish();
                        }
                    }
                },
                None => {
                    debug!("line not found {}", request.run);
                    return HttpResponse::BadRequest().finish();
                }
            };

            match region.graph.get(&tram.reporting_point) {
                Some(value) => {
                    if value.len() > 0 {
                        HttpResponse::Ok()
                            .insert_header(header::ContentType::json())
                            .json(value[0].clone())
                    } else {
                        return HttpResponse::BadRequest().finish();
                    }
                }
                None => {
                    return HttpResponse::BadRequest().finish();
                }
            }
        },
        None => HttpResponse::BadRequest().finish(),
    }
}
