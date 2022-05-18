extern crate serde_json;

mod api;
mod state;
mod telegram;

pub use api::{coordinates, expected_time, get_network, query_vehicle};
pub use state::{Network, State, Tram};
pub use telegram::{
    ReceivesTelegrams, ReceivesTelegramsServer, ReducedTelegram, ReturnCode, WebSocketTelegram,
};

use std::collections::HashMap;
use std::env;
use std::fs;
use std::net::TcpListener;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer};

use serde::{Deserialize, Serialize};
use tonic::{transport::Server, Request, Response, Status};
use tungstenite::accept;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Stop {
    lat: f64,
    lon: f64,
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Filter {
    #[serde(default)]
    regions: Vec<u32>,
    #[serde(default)]
    junctions: Vec<u32>,
    #[serde(default)]
    lines: Vec<u32>,
}

impl Filter {
    pub fn fits(&self, telegram: &ReducedTelegram) -> bool {
        (self.regions.is_empty() || self.regions.contains(&telegram.region_code))
            && (self.junctions.is_empty() || self.junctions.contains(&telegram.position_id))
            && (self.lines.is_empty() || self.lines.contains(&telegram.line))
    }
}

pub struct UserConnection {
    socket: tungstenite::protocol::WebSocket<std::net::TcpStream>,
    filter: Option<Filter>,
}

impl UserConnection {
    pub fn update(&mut self) {
        match self.socket.read_message() {
            Ok(message) => {
                if !message.is_text() {
                    return;
                }

                match message {
                    tungstenite::protocol::Message::Text(raw_message) => {
                        self.filter = serde_json::from_str(&raw_message).ok();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    pub fn write(&mut self, telegram: &ReducedTelegram, stop: Stop) -> bool {
        if self.filter.is_some() && !self.filter.as_ref().unwrap().fits(telegram) {
            return false;
        }

        let sock_tele = WebSocketTelegram {
            reduced: telegram.clone(),
            meta_data: stop,
        };

        let wstelegram = serde_json::to_string(&sock_tele).unwrap();
        self.socket
            .write_message(tungstenite::Message::text(wstelegram))
            .is_err()
    }
}

#[derive(Clone)]
pub struct TelegramProcessor {
    pub connections: Arc<Mutex<Vec<UserConnection>>>,
    pub state: Arc<RwLock<State>>,
    pub stops_lookup: HashMap<u32, HashMap<u32, Stop>>,
}

impl TelegramProcessor {
    fn new(list: Arc<Mutex<Vec<UserConnection>>>, state: Arc<RwLock<State>>) -> TelegramProcessor {
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
        let mut unlocked = self.connections.lock().unwrap();

        let extracted = request.into_inner().clone();
        let region = &extracted.region_code;
        let mut dead_socket_indices: Vec<usize> = Vec::new();
        for (i, socket) in (&mut *unlocked).iter_mut().enumerate() {
            let stop_meta_information =
                self.stop_meta_data(extracted.position_id, extracted.region_code);
            socket.update();
            if socket.write(&extracted, stop_meta_information) {
                dead_socket_indices.push(i);
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
        // removing dead sockets
        let mut remove_count = 0;
        for index in dead_socket_indices {
            (&mut *unlocked).remove(index - remove_count);
            remove_count += 1;
        }

        let reply = ReturnCode { status: 0 };

        Ok(Response::new(reply))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let default_grpc_port = String::from("127.0.0.1:50051");
    let grpc_port = env::var("GRPC_HOST").unwrap_or(default_grpc_port);

    let default_websock_port = String::from("127.0.0.1:9001");
    let websocket_port = env::var("DEFAULT_WEBSOCKET_HOST").unwrap_or(default_websock_port);

    let default_host = String::from("127.0.0.1");
    let http_host = env::var("HTTP_HOST").unwrap_or(default_host);

    let default_port = String::from("9002");
    let http_port = env::var("HTTP_PORT")
        .unwrap_or(default_port)
        .parse::<u16>()
        .unwrap();

    //let addr = "127.0.0.1:50051".parse()?;
    let addr = grpc_port.parse()?;

    let list: Arc<Mutex<Vec<UserConnection>>> = Arc::new(Mutex::new(vec![]));
    let list_ref = Arc::clone(&list);
    let state = Arc::new(RwLock::new(State::new())); //Arc::new(Mutex::new(Network::new()));
    let state_copy = Arc::clone(&state);

    thread::spawn(move || {
        println!("Opening Websocket Sever ...");
        let server = TcpListener::bind(websocket_port).unwrap();
        for stream in server.incoming() {
            match accept(stream.unwrap()) {
                Ok(websocket) => {
                    let mut unpacked = list_ref.lock().unwrap();
                    unpacked.push(UserConnection {
                        socket: websocket,
                        filter: None,
                    });
                }
                Err(_) => {}
            };
        }
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
                    //.service(
                    //    web::scope("/")
                    //        .service(web::resource("/state/{region}/all").route(web::get().to(get_network))),
                    //)
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
