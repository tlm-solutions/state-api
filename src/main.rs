extern crate serde_json;

mod state;
mod api;

pub use state::{Network};
pub use api::{get_network};

use std::thread;
use std::env;
use std::net::TcpListener;
use std::sync::{Arc, Mutex, RwLock};

use serde::ser::{Serialize, SerializeStruct, Serializer};
use tungstenite::accept;
use tonic::{transport::Server, Request, Response, Status};
use actix_web::{web, App, HttpServer };

use dvb_dump::receives_telegrams_server::{ReceivesTelegrams, ReceivesTelegramsServer};
use dvb_dump::{ReturnCode, ReducedTelegram};

pub mod dvb_dump {
    tonic::include_proto!("dvbdump");
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
        s.serialize_field("lat", &self.lat)?;
        s.serialize_field("lon", &self.lon)?;
        s.serialize_field("station_name", &self.station_name)?;
        s.serialize_field("train_length", &self.train_length)?;
        s.serialize_field("run_number", &self.run_number)?;
        s.end()
    }
}

#[derive(Clone)]
pub struct TelegramProcessor {
    pub connections: Arc<Mutex<Vec<Mutex<tungstenite::protocol::WebSocket<std::net::TcpStream>>>>>,
    pub state: Arc<RwLock<Network>>
}

impl TelegramProcessor {
    fn new(list: Arc<Mutex<Vec<Mutex<tungstenite::protocol::WebSocket<std::net::TcpStream>>>>>,
           state: Arc<RwLock<Network>>
) -> TelegramProcessor {
        TelegramProcessor {
            connections: list,
            state: state
        }
    }
}

#[tonic::async_trait]
impl ReceivesTelegrams for TelegramProcessor {
    async fn receive_new (&self, request: Request<ReducedTelegram>) -> Result<Response<ReturnCode>, Status> {
        let mut unlocked = self.connections.lock().unwrap();

        let extracted = request.into_inner().clone();
        let mut dead_socket_indices = Vec::new();
        for (i, socket) in (&*unlocked).iter().enumerate() {
            let mut client = socket.lock().unwrap();

			match client.write_message(tungstenite::Message::text(serde_json::to_string(&extracted).unwrap())) {
                Ok(_) => {}
                Err(_) => {
                    dead_socket_indices.push(i);
                }
            };
        }

        // update internal state
        {
            let mut data = (*self.state).write().unwrap();
            data.update(&extracted);
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
    let state = Arc::new(RwLock::new(Network::new())); //Arc::new(Mutex::new(Network::new()));
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
        rt.block_on(HttpServer::new(move || App::new()
                    .app_data(data.clone())
                    .route("/state_all", web::post().to(get_network))
                    )
            .bind((http_host, http_port))
            .unwrap()
            .run()
        );
    });
    let telegram_processor = TelegramProcessor::new(list, state);
    Server::builder()
        .add_service(ReceivesTelegramsServer::new(telegram_processor))
        .serve(addr)
        .await?;

    Ok(())
}
