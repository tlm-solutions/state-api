
use super::{State};

use actix_web::{web, Responder};
use std::sync::{RwLock, Arc};
use std::collections::{HashMap};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct NetworkRequest {
    region: String
}

#[derive(Serialize, Deserialize)]
pub struct Error{
    error_message: String
}


pub async fn get_network(state: web::Data<Arc<RwLock<State>>>, region: web::Path<String>)-> impl Responder {
    let data = state.read().unwrap();

    let region_lookup: HashMap<&str, u32> = HashMap::from([
        ("dresden", 1),
        ("chemnitz", 2),
        ("karlsruhe", 3),
        ("berlin", 4)
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
            web::Json(Ok(region.lines.clone()))
        }
        None => {
            web::Json(Err(Error {
                error_message: String::from("Network of region was not initialized!")
            }))
        }
    }
}
