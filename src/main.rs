extern crate serde_json;

mod api;
mod state;

pub use api::endpoints::{get_network, get_position, query_vehicle};
pub use state::{Network, State, Tram};

use tlms::telegrams::r09::{
    R09GrpcTelegram, ReceivesTelegrams, ReceivesTelegramsServer, ReturnCode,
};

use std::env;
use std::sync::{Arc, RwLock};
use std::thread;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use actix_web_prom::{PrometheusMetrics, PrometheusMetricsBuilder};
use log::{debug, info};
use tonic::{transport::Server, Request, Response, Status};

#[derive(Clone)]
pub struct TelegramProcessor {
    pub state: Arc<RwLock<State>>,
}

impl TelegramProcessor {
    fn new(state: Arc<RwLock<State>>) -> TelegramProcessor {
        let default_stops = String::from("../stops.json");
        let stops_file = env::var("STOPS_FILE").unwrap_or(default_stops);

        debug!("Reading File: {}", &stops_file);
        TelegramProcessor { state }
    }
}

#[tonic::async_trait]
impl ReceivesTelegrams for TelegramProcessor {
    async fn receive_r09(
        &self,
        request: Request<R09GrpcTelegram>,
    ) -> Result<Response<ReturnCode>, Status> {
        let extracted = request.into_inner();
        {
            let unwrapped_state = &mut (*self.state.write().unwrap());

            if let Some(network) = unwrapped_state.regions.get_mut(&extracted.region) {
                network.update(&extracted);
            }
        }

        let reply = ReturnCode { status: 0 };
        Ok(Response::new(reply))
    }
}

pub fn get_prometheus() -> PrometheusMetrics {
    PrometheusMetricsBuilder::new("api")
        .endpoint("/metrics")
        //.const_labels(None)
        .build()
        .unwrap()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let default_worker_count = "4".to_string();
    let worker_count = (env::var("WORKER_COUNT").unwrap_or(default_worker_count))
        .parse::<usize>()
        .expect("cannot decode workers into integer");

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

    let state = Arc::new(RwLock::new(State::new()));
    let state_copy = Arc::clone(&state);

    thread::spawn(move || {
        info!("Opening Http Sever ...");
        let data = web::Data::new(state_copy);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let prometheus = get_prometheus();

        rt.block_on(
            HttpServer::new(move || {
                let cors = Cors::default()
                    .allow_any_header()
                    .allow_any_method()
                    .allow_any_origin();

                App::new()
                    .app_data(data.clone())
                    .wrap(cors)
                    .wrap(prometheus.clone())
                    .route("/vehicles/{region}/all", web::get().to(get_network))
                    .route("/vehicles/{region}/query", web::post().to(query_vehicle))
                    .route("/vehicles/{region}/position", web::post().to(get_position))
            })
            .workers(worker_count)
            .bind((http_host, http_port))
            .unwrap()
            .run(),
        )
        .unwrap();
    });
    let telegram_processor = TelegramProcessor::new(state);
    Server::builder()
        .add_service(ReceivesTelegramsServer::new(telegram_processor))
        .serve(addr)
        .await?;

    Ok(())
}
