extern crate serde_json;
use std::thread;
use serde::ser::{Serialize, SerializeStruct, Serializer};

use std::env;
use std::net::TcpListener;
use tungstenite::accept;

use tonic::{transport::Server, Request, Response, Status};

use dvb_dump::receives_telegrams_server::{ReceivesTelegrams, ReceivesTelegramsServer};
use dvb_dump::{ReducedTelegram, ReturnCode};

use std::sync::{Arc, Mutex};

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
        s.end()
    }
}

//#[derive(Default)]
pub struct TelegramProcessor {
    pub connections: Arc<Mutex<Vec<Mutex<tungstenite::protocol::WebSocket<std::net::TcpStream>>>>>
}

impl TelegramProcessor {
    fn new(list: Arc<Mutex<Vec<Mutex<tungstenite::protocol::WebSocket<std::net::TcpStream>>>>>
) -> TelegramProcessor {
        TelegramProcessor {
            connections: list
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
                    println!("Found dead socket");
                    dead_socket_indices.push(i);
                }
            };
        }
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

    //let addr = "127.0.0.1:50051".parse()?;
    let addr = grpc_port.parse()?;

    let list: Arc<Mutex<Vec<Mutex<tungstenite::protocol::WebSocket<std::net::TcpStream>>>>> = Arc::new(Mutex::new(vec![]));
    let list_ref = Arc::clone(&list);
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

    let telegram_processor = TelegramProcessor::new(list);
    Server::builder()
        .add_service(ReceivesTelegramsServer::new(telegram_processor))
        .serve(addr)
        .await?;

    Ok(())
}
