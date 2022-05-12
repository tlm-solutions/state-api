
use super::{State, Tram};

use actix_web::{web, Responder};
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

pub async fn get_network(state: web::Data<Arc<RwLock<State>>>, region: web::Path<String>)-> impl Responder {
    let data = state.read().unwrap();
    //let unwrapped_region = region.into_inner();

    println!("Received Request with {}", region.as_str());

    let region_lookup: HashMap<&str, u32> = HashMap::from([
        ("dresden", 0),
        ("chemnitz", 1),
        ("karlsruhe", 2),
        ("berlin", 3)
    ]);

    let region_id;
    match region_lookup.get(&region.as_str()) {
        Some(id) => {
            region_id = id;
        }
        None => {
            return web::Json(Err(Error {
                error_message: String::from("Invalid Region ID")
            }))
        }
    }

    match data.regions.get(region_id) {
        Some(region) => {
            let start = SystemTime::now();
            let since_the_epoch = start
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs();

            web::Json(Ok(EntireNetworkResponse{ 
                network: region.lines.clone(),
                time_stamp: since_the_epoch
            }))
        }
        None => {
            web::Json(Err(Error {
                error_message: String::from("Network of region was not initialized!")
            }))
        }
    }
}

