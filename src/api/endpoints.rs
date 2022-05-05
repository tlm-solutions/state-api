
use super::{Network};

use actix_web::{web, Responder};
use std::sync::{RwLock, Arc};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct NetworkRequest {
    region: String
}

pub async fn get_network(state: web::Data<Arc<RwLock<Network>>>,  _network: web::Json<NetworkRequest>) -> impl Responder {
    let data = state.read().unwrap();
    web::Json(data.clone())
}

