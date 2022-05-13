extern crate serde_json;

mod state;
mod api;

pub use state::{State, Network, Tram};
pub use api::{get_network, coordinates, query_vehicle, expected_time};

use std::thread;
use std::env;
use std::net::TcpListener;
use std::sync::{Arc, Mutex, RwLock};
use std::fs;
use std::collections::HashMap;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer };

use tungstenite::accept;
use tonic::{transport::Server, Request, Response, Status};
use serde::ser::{SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};

use dvb_dump::receives_telegrams_server::{ReceivesTelegrams, ReceivesTelegramsServer};
use dvb_dump::{ReturnCode, ReducedTelegram};

pub mod dvb_dump {
    tonic::include_proto!("dvbdump");
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Stop {
    lat: f64,
    lon: f64,
    name: String
}

#[derive(Debug, Serialize)]
pub struct WebSocketTelegram {
    #[serde(flatten)]
    reduced: ReducedTelegram,

    #[serde(flatten)]
    meta_data: Stop
}


impl Serialize for ReducedTelegram {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("ReducedTelegram", 7)?;
        s.serialize_field("time_stamp", &self.time_stamp)?;
        s.serialize_field("position_id", &self.position_id)?;
        s.serialize_field("direction", &self.direction)?;
        s.serialize_field("status", &self.status)?;
        s.serialize_field("line", &self.line)?;
        s.serialize_field("delay", &self.delay)?;
        s.serialize_field("destination_number", &self.destination_number)?;
        s.serialize_field("train_length", &self.train_length)?;
        s.serialize_field("run_number", &self.run_number)?;
        s.serialize_field("region_code", &self.region_code)?;
        s.end()
    }
}

#[derive(Clone)]
pub struct TelegramProcessor {
    pub connections: Arc<Mutex<Vec<Mutex<tungstenite::protocol::WebSocket<std::net::TcpStream>>>>>,
    pub state: Arc<RwLock<State>>,
    pub stops_lookup: HashMap<u32, HashMap<u32, Stop>>
}

impl TelegramProcessor {
    fn new(list: Arc<Mutex<Vec<Mutex<tungstenite::protocol::WebSocket<std::net::TcpStream>>>>>,
           state: Arc<RwLock<State>>
) -> TelegramProcessor {

        let default_stops = String::from("../stops.json");
        let stops_file = env::var("STOPS_FILE").unwrap_or(default_stops);

        println!("Reading File: {}", &stops_file);
        let data = fs::read_to_string(stops_file).expect("Unable to read file");
        let res: HashMap<u32, HashMap<u32, Stop>> = serde_json::from_str(&data).expect("Unable to parse");
        TelegramProcessor {
            connections: list,
            state: state,
            stops_lookup: res
        }
    }
}

#[tonic::async_trait]
impl ReceivesTelegrams for TelegramProcessor {
    async fn receive_new (&self, request: Request<ReducedTelegram>) -> Result<Response<ReturnCode>, Status> {
        let mut unlocked = self.connections.lock().unwrap();

        let extracted = request.into_inner().clone();
        let region = &extracted.region_code;
        let mut dead_socket_indices = Vec::new();
        for (i, socket) in (&*unlocked).iter().enumerate() {
            let mut client = socket.lock().unwrap();

            let stop;
            match self.stops_lookup.get(&region){
                Some(regional_stops) => {
                    match regional_stops.get(&extracted.position_id){
                        Some(found_stop) => {
                            stop = found_stop.clone();
                        }
                        None => {
                            stop = Stop {
                                lat: 0f64,
                                lon: 0f64,
                                name: String::from("")
                            }
                        }
                    }
                }
                None => {
                    stop = Stop {
                        lat: 0f64,
                        lon: 0f64,
                        name: String::from("")
                    }
                }
            }

            let sock_tele = WebSocketTelegram {
                reduced: extracted.clone(),
                meta_data: stop
            };

            let wstelegram = serde_json::to_string(&sock_tele).unwrap();
			match client.write_message(tungstenite::Message::text(wstelegram)) {
                Ok(_) => {}
                Err(_) => {
                    dead_socket_indices.push(i);
                }
            };
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
            (&mut*unlocked).remove(index - remove_count);
            remove_count += 1;
        }

        let reply = dvb_dump::ReturnCode {
            status: 0
        };

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
    let http_port = env::var("HTTP_PORT").unwrap_or(default_port).parse::<u16>().unwrap();

    //let addr = "127.0.0.1:50051".parse()?;
    let addr = grpc_port.parse()?;

    let list: Arc<Mutex<Vec<Mutex<tungstenite::protocol::WebSocket<std::net::TcpStream>>>>> = Arc::new(Mutex::new(vec![]));
    let list_ref = Arc::clone(&list);
    let state = Arc::new(RwLock::new(State::new())); //Arc::new(Mutex::new(Network::new()));
    let state_copy = Arc::clone(&state);

    thread::spawn( move || {
        println!("Opening Websocket Sever ...");
        let server = TcpListener::bind(websocket_port).unwrap();
        for stream in server.incoming() {
            match accept(stream.unwrap()) {
                Ok(websocket) => {
                    let mut unpacked = list_ref.lock().unwrap();
                    unpacked.push(Mutex::new(websocket));
                }
                Err(_) => {}
            };
        }
    });
    thread::spawn( move || {
        println!("Opening Http Sever ...");
        let data = web::Data::new(state_copy);
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(HttpServer::new(move || {
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
                    .route("/network/{region}/estimated_travel_time", web::post().to(expected_time))
                    .route("/static/{region}/coordinates", web::post().to(coordinates))
            })
            .bind((http_host, http_port))
            .unwrap()
            .run());
    });
    let telegram_processor = TelegramProcessor::new(list, state);
    Server::builder()
        .add_service(ReceivesTelegramsServer::new(telegram_processor))
        .serve(addr)
        .await?;

    Ok(())
}
