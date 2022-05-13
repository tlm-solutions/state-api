
mod r#static;

pub use r#static::{coordinates};

use super::{State, Tram, Stop};

use actix_web::{web, Responder, HttpResponse, http::header};
use std::sync::{RwLock, Arc};
use std::collections::{HashMap};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct NetworkRequest {
    region: String
}

#[derive(Serialize, Deserialize)]
pub struct Error{
    error_message: String
}

#[derive(Serialize, Deserialize)]
pub struct EntireNetworkResponse {
    network: HashMap<u32, HashMap<u32, Tram>>,
    time_stamp: u64
}

#[derive(Serialize, Deserialize)]
pub struct RequestInformationTime {
    junction: u32,
    direction: u32
}

#[derive(Serialize, Deserialize)]
pub struct RequestVehicleInformation {
    line: u32,
    run_number: u32
}

#[derive(Serialize, Deserialize)]
pub struct RequiredTime {
    historical_time: u32,
    destination: u32
}

pub async fn name_to_id(name: &String) -> Option<u32> {
    let region_lookup: HashMap<&str, u32> = HashMap::from([
        ("dresden", 0),
        ("chemnitz", 1),
        ("karlsruhe", 2),
        ("berlin", 3)
    ]);
    
    match region_lookup.get(name.as_str()){
        Some(val) => {
            Some(*val)
        }
        None => {
            None
        }
    }
}

// GET /vehicles/dresden/all
pub async fn get_network(state: web::Data<Arc<RwLock<State>>>, region: web::Path<String>)-> impl Responder {
    //let unwrapped_region = region.into_inner();
    println!("Received Request with {}", region.as_str());
    
    let region_id;
    match name_to_id(&region).await {
        Some(region ) => {
            region_id = region;
        }
        None => {
            return HttpResponse::BadRequest().finish();
        }
    }

    let data = state.read().unwrap();
    match data.regions.get(&region_id) {
        Some(region) => {
            let start = SystemTime::now();
            let since_the_epoch = start
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs();

            HttpResponse::Ok()
                .insert_header(header::ContentType::json())
                .json(EntireNetworkResponse{ 
                    network: region.lines.clone(),
                    time_stamp: since_the_epoch
                })
        }
        None => {
            HttpResponse::BadRequest().finish()
        }
    }
}

// POST /vehicles/dresden/query
pub async fn query_vehicle(state: web::Data<Arc<RwLock<State>>>, region: web::Path<String>, request: web::Json<RequestVehicleInformation>)-> impl Responder {
    //let unwrapped_region = region.into_inner();
    println!("Received Request with {}", region.as_str());
    
    let region_id;
    match name_to_id(&region).await {
        Some(region ) => {
            region_id = region;
        }
        None => {
            return HttpResponse::BadRequest().finish();
        }
    }

    let data = state.read().unwrap();
    match data.regions.get(&region_id) {
        Some(region) => {
            if region.lines.contains_key(&request.line) && region.lines.get(&request.line).unwrap().contains_key(&request.run_number) {
                HttpResponse::Ok()
                    .insert_header(header::ContentType::json())
                    .json(
                        region.lines.get(&request.line).unwrap().get(&request.run_number).unwrap()
                    )

            } else {
                HttpResponse::BadRequest().finish()
            }
                    }
        None => {
            HttpResponse::BadRequest().finish()
        }
    }
}

// POST /network/dresden/estimated_travel_time
pub async fn expected_time(state: web::Data<Arc<RwLock<State>>>, region: web::Path<String>, request: web::Json<RequestInformationTime>)-> impl Responder {
    let region_id;
    match name_to_id(&region).await {
        Some(region ) => {
            region_id = region;
        }
        None => {
            return HttpResponse::BadRequest().finish();
        }
    }

    let data = state.read().unwrap();
    match data.regions.get(&region_id) {
        Some(region) => {
            match region.edges.get(&(request.junction, request.direction)) {
                Some(time_found) => {

                    let destination = region.graph.structure.get(&request.junction).unwrap().get(&request.direction).unwrap();
                    HttpResponse::Ok()
                        .insert_header(header::ContentType::json())
                        .json( RequiredTime { 
                            required_time: *time_found,
                            destination: *destination
                        })
                }
                None => {
                    HttpResponse::BadRequest().finish()
                }
            } 
        }
        None => {
            HttpResponse::BadRequest().finish()
        }
    }
}

