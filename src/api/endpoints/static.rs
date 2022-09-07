use dump_dvb::locations::TransmissionPosition;

use actix_web::{web, Responder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;

#[derive(Serialize, Deserialize)]
pub struct CoordinatesStation {
    pub station_id: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Error {
    error_message: String,
}

// /static/{region}/coordinates
pub async fn coordinates(
    region: web::Path<i32>,
    request: web::Json<CoordinatesStation>,
) -> impl Responder {
    let default_stops = String::from("../stops.json");
    let stops_file = env::var("STOPS_FILE").unwrap_or(default_stops);

    println!("Reading File: {}", &stops_file);
    let data = fs::read_to_string(stops_file).expect("Unable to read file");
    let stops: HashMap<u32, HashMap<u32, TransmissionPosition>> =
        serde_json::from_str(&data).expect("Unable to parse");

    match stops.get(&(*region as u32)) {
        Some(station_look_up) => match station_look_up.get(&request.station_id) {
            Some(stop) => web::Json(Ok(stop.clone())),
            None => {
                return web::Json(Err(Error {
                    error_message: String::from("Station ID not found for region"),
                }))
            }
        },
        None => {
            return web::Json(Err(Error {
                error_message: String::from(
                    "This Server doesn't contain the config for this region",
                ),
            }))
        }
    }
}
