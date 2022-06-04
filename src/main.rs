extern crate serde_json;

mod api;
mod state;
mod telegram;
mod connection;

pub use api::{coordinates, expected_time, get_network, query_vehicle};
pub use state::{Network, State, Tram};
pub use telegram::{
    ReceivesTelegrams, ReceivesTelegramsServer, ReducedTelegram, ReturnCode, WebSocketTelegram,
};
pub use connection::{
    ProtectedState,
    ReadSocket,
    WriteSocket,
    ConnectionPool,
    accept_connections
};

use std::collections::HashMap;
use std::env;
use std::fs;
use std::sync::{Arc, Mutex, RwLock};
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
    async fn receive_new(&self, request: Request<ReducedTelegram>) -> Result<Response<ReturnCode>, Status> {
        //let mut unlocked = self.connections.lock().unwrap();

        let extracted = request.into_inner().clone();
        let region = &extracted.region_code;
        let mut dead_socket_indices: Vec<usize> = Vec::new();
        let stop_meta_information = self.stop_meta_data(extracted.position_id, extracted.region_code);
        {
            let mut unwrapped = self.connections.lock().unwrap();
            for (i, socket) in unwrapped.iter_mut().enumerate() {
                println!("Trying to send to {}", i);
                if socket.write(&extracted, &stop_meta_information) {
                    dead_socket_indices.push(i);
                }
            }

            // removing dead sockets
            let mut remove_count = 0;
            for index in dead_socket_indices {
                unwrapped.remove(index - remove_count);
                remove_count += 1;
            }

        }

        // update internal state
        {
            let unwrapped_state = &mut (*self.state.write().unwrap());
            match unwrapped_state.regions.get_mut(&region) {
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

    //let addr = "127.0.0.1:50051".parse()?;
    let addr = grpc_port.parse()?;

    let list: ConnectionPool = Arc::new(Mutex::new(vec![]));
    let list_ref = Arc::clone(&list);
    let state = Arc::new(RwLock::new(State::new())); //Arc::new(Mutex::new(Network::new()));
    let state_copy = Arc::clone(&state);

    tokio::spawn(async move {
        accept_connections(list_ref.clone()).await;
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
        );
    });
    let telegram_processor = TelegramProcessor::new(list, state);
    Server::builder()
        .add_service(ReceivesTelegramsServer::new(telegram_processor))
        .serve(addr)
        .await?;

    Ok(())
}
