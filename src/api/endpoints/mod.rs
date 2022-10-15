mod r#static;

pub use r#static::coordinates;

use super::{State, Tram};

use dump_dvb::locations::{LineSegment, Segments};

use actix_web::{http::header, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use log::{info, debug};
use chrono::NaiveDateTime;

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

// GET /vehicles/dresden/all
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

// POST /vehicles/dresden/query
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

// POST /network/dresden/get
pub async fn get_vehicle(
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
                            return HttpResponse::BadRequest().finish();
                        }
                    }
                },
                None => {
                    return HttpResponse::BadRequest().finish();
                }
            };

            match region.model.get(&tram.reporting_point) {
                Some(report_location) => {
                    match serde_json::value::from_value::<Segments>(report_location.properties.clone()) {
                        Ok(value) => {
                            match value.segments.get(&tram.direction) {
                                Some(segment) => {
                                    HttpResponse::Ok()
                                        .insert_header(header::ContentType::json())
                                        .json(segment)
                                }
                                None => {
                                    return HttpResponse::BadRequest().finish();
                                }
                            }
                        }
                        Err(_) => {
                            debug!("couldn't find segment in extra properties");
                            return HttpResponse::BadRequest().finish();
                        }
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
