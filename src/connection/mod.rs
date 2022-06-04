use serde::{Serialize, Deserialize};
use futures::{
    channel::mpsc::{unbounded, UnboundedSender}
};
use async_std::{
    net::{TcpListener, TcpStream},
    task,
};
use std::env;
use std::sync::{Mutex, Arc};
use futures_util::{
    StreamExt,
    stream::{SplitStream},
};

use async_tungstenite::{
    tungstenite::protocol::Message,
    async_std::ConnectStream,
    WebSocketStream
};
use futures::prelude::*;
use serde::de::{Error};

//use futures_io::{AsyncRead, AsyncWrite};

use super::{ReducedTelegram, Stop, WebSocketTelegram};

//pub type ConnectStream = ClientStream<TcpStream>;

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

pub struct UserState {
    filter: Option<Filter>,
    dead: bool
}

pub type ProtectedState = Arc<Mutex<UserState>>;

pub struct ReadSocket {
    socket: SplitStream<WebSocketStream<ConnectStream>>, //TcpStream, //SplitSink,
    //socket: SplitStream<TcpStream>, //TcpStream, //SplitSink,
    state: ProtectedState
}

pub struct WriteSocket {
    socket: UnboundedSender<Message>,//SplitSink<WebSocketStream<ConnectStream>, tungstenite::Message>, //TcpStream, //SplitStream,
    //socket: SplitSink<TcpStream, tungstenite::Message>, //TcpStream, //SplitStream,
    state: ProtectedState
}

pub type ConnectionPool = Arc<Mutex<Vec<WriteSocket>>>;

impl ReadSocket {
    pub async fn read(&mut self) -> bool {
        match (&mut self.socket).try_filter(|msg| {
            future::ready(msg.is_text())
        }).map(|data| {
            match data {
                Ok(nice_data) => {
                    match nice_data {
                        tungstenite::Message::Text(text_message) => {
                            serde_json::from_str(&text_message)
                        }
                        _ => {
                            Err(Error::custom("fuck"))
                        }
                    }
                },
                Err(_) => {Err(Error::custom("fuck"))}
            }
        }).next().await {
            Some(connection) => {
                match connection {
                    Ok(filter) => {
                        self.state.lock().unwrap().filter = Some(filter);
                        false
                    }
                    Err(_) => { true }
                }
            }
            None => { true }
        }
    }
}

impl WriteSocket {
    pub fn write(&mut self, telegram: &ReducedTelegram, stop: &Stop) -> bool {
        {
            let state = self.state.lock().unwrap();
            if state.filter.is_some() && !state.filter.as_ref().unwrap().fits(telegram) {
                return false;
            }
        }

        let sock_tele = WebSocketTelegram {
            reduced: telegram.clone(),
            meta_data: stop.clone(),
        };

        println!("Dumping Data");

        //let wstelegram = tungstenite::Message::text(
        let wstelegram = serde_json::to_string(&sock_tele).unwrap();

        match self.socket.unbounded_send(tungstenite::Message::Text(wstelegram)) {
            Err(e) => {
                println!("Err: {:?}", e);
                true
            }
            _ => {false}
        }
    }
}


impl UserState {
    pub fn new() -> UserState {
        UserState {
            filter: None,
            dead: false
        }
    } 
}

pub async fn handle_connection(mut read: ReadSocket) {
    loop {
        // connection has died
        if read.state.lock().unwrap().dead {
            return;
        }
        if read.read().await {
            return;
        };
    }
}

pub async fn connection_setup(stream: TcpStream, connections: ConnectionPool) {
    let ws_stream = async_tungstenite::accept_async(stream)
        .await
        .unwrap();

    let (tx, _) = unbounded();

    let (_write, read) = ws_stream.split();
    //println!("[Socket] new connection from {}", &addr);

    let state: ProtectedState = Arc::new(Mutex::new(UserState{ dead: false, filter: None })); 

    {
        let mut unpacked = connections.lock().unwrap();
        println!("Pushing back Writesocket");
        unpacked.push(WriteSocket {
                socket: tx,
                state: state.clone()
        });
    }
    let read_socket = ReadSocket {
        socket: read,
        state: state.clone()
    };

    println!("Spawning process");
    task::spawn(handle_connection(read_socket));
}

pub async fn accept_connections(connections: ConnectionPool) {
    let default_websock_port = String::from("127.0.0.1:9001");
    let websocket_port = env::var("DEFAULT_WEBSOCKET_HOST").unwrap_or(default_websock_port);
    

    println!("Opening Websocket Sever ...");
    let server = TcpListener::bind(websocket_port).await.unwrap();
    while let Ok((stream, addr)) = server.accept().await {
        println!("New Socket Connection {}!", addr);
        async_std::task::spawn(connection_setup(stream, connections.clone()));
    }
}


