// This example explores how to properly close a connection.
//
use
{
	ws_stream_tungstenite  :: { *                                            } ,
	futures                :: { TryFutureExt, StreamExt, SinkExt, join, executor::block_on } ,
	asynchronous_codec     :: { LinesCodec, Framed, FramedRead, FramedWrite                         } ,
	tokio                  :: { net::{ TcpListener }                         } ,
	futures                :: { FutureExt, select, future::{ ok, ready }     } ,
	async_tungstenite      :: { accept_async, tokio::{ TokioAdapter, connect_async } } ,
	std                    :: { time::Duration, sync::{Arc, Mutex}, env                               } ,
    serde::{Serialize, Deserialize}
};
use futures_util::stream::{SplitSink, SplitStream};
use tokio::net::TcpStream;
use super::{ReducedTelegram, Stop, WebSocketTelegram};

/*
block_on( async
{
    join!( server(), client() );

});
*/

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
    socket: SplitStream<Framed<WsStream<TokioAdapter<TcpStream>>, LinesCodec>>,
    state: ProtectedState
}

pub struct WriteSocket {
    socket: SplitSink<Framed<WsStream<TokioAdapter<TcpStream>>, LinesCodec>, String>,
    state: ProtectedState
}


fn print_type_of<T>(_: &T) {
    println!("{}", std::any::type_name::<T>())
}

pub async fn connection_loop(mut connections: ConnectionPool) {
    let default_websock_port = String::from("127.0.0.1:9001");
    let websocket_port = env::var("DEFAULT_WEBSOCKET_HOST").unwrap_or(default_websock_port);
 
	let server = TcpListener::bind( websocket_port ).await.unwrap();

    while let Ok((tcp, addr)) = server.accept().await {
        println!("New Socket Connection {}!", addr);

        let s   = accept_async( TokioAdapter::new(tcp) ).await.expect( "ws handshake" );
	    let ws  = WsStream::new( s );

        // spliting the socket into read and write component
	    let (mut writer, mut reader) = Framed::new( ws, LinesCodec {} ).split();

        print_type_of(&writer);
        print_type_of(&reader);

        let state: ProtectedState = Arc::new(Mutex::new(UserState{ dead: false, filter: None })); 

        {
            connections.push(WriteSocket {
                    socket: writer,
                    state: state.clone()
            });
        }

        let read_socket = ReadSocket {
            socket: reader,
            state: state.clone()
        };

        //task::spawn(handle_connection(read_socket));
    }
}

impl ReadSocket {
    pub async fn read(&mut self) -> bool {
        //let mut framed = FramedRead::new( self.socket, LinesCodec::new() );
        //let read = framed.next().await.transpose().expect( "close connection" );
        false
    }
}

impl WriteSocket {
    pub async fn write(&mut self, telegram: &ReducedTelegram, stop: &Stop) -> bool {
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
        let serialized = serde_json::to_string(&sock_tele).unwrap();

        match self.socket.send( serialized ).await
		{
			Ok(_) => {false}
			Err(_) => {true}
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

#[derive(Clone)]
pub struct ConnectionPool {
    connections: Arc<Mutex<Vec<WriteSocket>>>
}

impl ConnectionPool {
    pub fn new() -> ConnectionPool {
        ConnectionPool {
            connections: Arc::new(Mutex::new(Vec::new()))
        }
    }

    pub fn clone(&mut self) -> ConnectionPool {
        ConnectionPool {
            connections:  Arc::clone(&self.connections)
        }
    }

    pub async fn write_all(&self, extracted: &ReducedTelegram, stop_meta_information: &Stop) {
        let mut results = Vec::new();

        let mut unlocked_self = self.connections.lock().unwrap();

        for (i, socket) in unlocked_self.iter_mut().enumerate() {
            println!("Trying to send to {}", i);
            results.push(block_on(socket.write(&extracted, &stop_meta_information)));
        }

        // removing dead sockets
        let mut remove_count = 0;
        for (index, dead) in results.iter().enumerate() {
            if *dead {
                unlocked_self.remove(index - remove_count);
                remove_count += 1;
            }
        }
    }

    pub async fn push(&mut self, sock: WriteSocket) {
        let mut unlocked_self = self.connections.lock().unwrap();
        unlocked_self.push(sock);
    }
}

