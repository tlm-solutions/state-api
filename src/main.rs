extern crate serde_json;

mod api;
mod connection;
mod state;
mod telegram;

pub use api::{coordinates, expected_time, get_network, query_vehicle};
pub use connection::{connection_loop, ConnectionPool, ProtectedState, Socket };
pub use state::{Network, State, Tram};
pub use telegram::{
    ReceivesTelegrams, ReceivesTelegramsServer, ReducedTelegram, ReturnCode, WebSocketTelegram,
};

use std::collections::HashMap;
use std::env;
use std::fs;
use std::sync::{Arc, RwLock};
use std::thread;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer};

use serde::{Deserialize, Serialize};
use tonic::{transport::Server, Request, Response, Status};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Stop {
    lat: f64,
    lon: f64,
    name: String,
}

#[derive(Clone)]
pub struct TelegramProcessor {
    pub connections: ConnectionPool,
    pub state: Arc<RwLock<State>>,
    pub stops_lookup: HashMap<u32, HashMap<u32, Stop>>,
}

impl TelegramProcessor {
    fn new(list: ConnectionPool, state: Arc<RwLock<State>>) -> TelegramProcessor {
        let default_stops = String::from("../stops.json");
        let stops_file = env::var("STOPS_FILE").unwrap_or(default_stops);

        println!("Reading File: {}", &stops_file);
        let data = fs::read_to_string(stops_file).expect("Unable to read file");
        let res: HashMap<u32, HashMap<u32, Stop>> =
            serde_json::from_str(&data).expect("Unable to parse");
        TelegramProcessor {
            connections: list,
            state: state,
            stops_lookup: res,
        }
    }

    fn stop_meta_data(&self, junction: u32, region: u32) -> Stop {
        match self.stops_lookup.get(&region) {
            Some(regional_stops) => match regional_stops.get(&junction) {
                Some(found_stop) => {
                    return found_stop.clone();
                }
                _ => {}
            },
            _ => {}
        }
        Stop {
            lat: 0f64,
            lon: 0f64,
            name: String::from(""),
        }
    }
}

#[tonic::async_trait]
impl ReceivesTelegrams for TelegramProcessor {
    async fn receive_new(
        &self,
        request: Request<ReducedTelegram>,
    ) -> Result<Response<ReturnCode>, Status> {
        //let mut unlocked = self.connections.lock().unwrap();

        let extracted = request.into_inner().clone();
        let stop_meta_information =
            self.stop_meta_data(extracted.position_id, extracted.region_code);

        self.connections
            .write_all(&extracted, &stop_meta_information)
            .await;

        {
            let unwrapped_state = &mut (*self.state.write().unwrap());
            match unwrapped_state.regions.get_mut(&extracted.region_code) {
                Some(network) => {
                    network.update(&extracted);
                }
                None => {}
            }
        }

        let reply = ReturnCode { status: 0 };
        Ok(Response::new(reply))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let default_grpc_port = String::from("127.0.0.1:50051");
    let grpc_port = env::var("GRPC_HOST").unwrap_or(default_grpc_port);

    let default_host = String::from("127.0.0.1");
    let http_host = env::var("HTTP_HOST").unwrap_or(default_host);

    let default_port = String::from("9002");
    let http_port = env::var("HTTP_PORT")
        .unwrap_or(default_port)
        .parse::<u16>()
        .unwrap();

    let addr = grpc_port.parse()?;

    let list: ConnectionPool = ConnectionPool::new();
    let list_clone = list.clone();
    let state = Arc::new(RwLock::new(State::new()));
    let state_copy = Arc::clone(&state);

    tokio::spawn(async move {
        connection_loop(list_clone).await;
    });

    thread::spawn(move || {
        println!("Opening Http Sever ...");
        let data = web::Data::new(state_copy);
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(
            HttpServer::new(move || {
                let cors = Cors::default()
                    .allow_any_header()
                    .allow_any_method()
                    .allow_any_origin();

                App::new()
                    .app_data(data.clone())
                    .wrap(cors)
                    .route("/vehicles/{region}/all", web::get().to(get_network))
                    .route("/vehicles/{region}/query", web::post().to(query_vehicle))
                    .route(
                        "/network/{region}/estimated_travel_time",
                        web::post().to(expected_time),
                    )
                    .route("/static/{region}/coordinates", web::post().to(coordinates))
            })
            .bind((http_host, http_port))
            .unwrap()
            .run(),
        )
        .unwrap();
    });
    let telegram_processor = TelegramProcessor::new(list.clone(), state);
    Server::builder()
        .add_service(ReceivesTelegramsServer::new(telegram_processor))
        .serve(addr)
        .await?;

    Ok(())
}
