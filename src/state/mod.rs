extern crate serde_json;

use tlms::locations::{
    graph::{PositionGraph, RegionGraph},
    LocationsJson, RegionReportLocations, RequestStatus,
};
use tlms::telegrams::r09::R09GrpcTelegram;

use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use log::error;
use std::collections::HashMap;
use std::env;
use std::fs;

/// All the relevat information about a tram is contained in this model
#[derive(Deserialize, Serialize, ToSchema, Debug, Clone)]
pub struct Tram {
    pub last_update: NaiveDateTime,
    pub reporting_point: i32, // germany wide or local ones
    pub line: u32,
    pub run_number: u32,
    pub time_stamp: u64,
    pub delayed: i32,
    pub direction: u32,
    pub request_status: RequestStatus,
}

#[derive(Serialize, Debug, Clone)]
pub struct Network {
    pub lines: HashMap<u32, HashMap<u32, Tram>>,
    pub positions: HashMap<i32, Vec<Tram>>,
    pub model: RegionReportLocations,
    pub graph: RegionGraph,
}

impl Network {
    pub fn new(model: RegionReportLocations, graph: RegionGraph) -> Network {
        Network {
            lines: HashMap::new(),
            positions: HashMap::new(),
            model,
            graph,
        }
    }

    pub fn query_tram(&self, line: &u32, run_number: &u32) -> Option<i32> {
        match self.lines.get(line) {
            Some(line) => line.get(run_number).map(|tram| tram.reporting_point),
            None => None,
        }
    }

    pub fn query_position(&mut self, reporting_point: &i32) -> Vec<Tram> {
        match self.positions.get(reporting_point) {
            Some(trams) => trams.to_vec(),
            None => Vec::new(),
        }
    }

    pub fn update(&mut self, telegram: &R09GrpcTelegram) {
        if telegram.line.is_none() || telegram.run_number.is_none() || telegram.delay.is_none() {
            return;
        }

        let request_status: RequestStatus = match (telegram.request_status as i16).try_into() {
            Ok(status) => status,
            Err(_) => {
                error!("request status decodation failed");
                return;
            }
        };

        let new_tram = Tram {
            last_update: Utc::now().naive_utc(),
            reporting_point: telegram.reporting_point,
            line: telegram.line.unwrap() as u32,
            run_number: telegram.run_number.unwrap() as u32,
            time_stamp: telegram.time,
            delayed: telegram.delay.unwrap(),
            direction: telegram.direction as u32,
            request_status,
        };

        match self.positions.get_mut(&new_tram.reporting_point) {
            Some(trams) => {
                trams.push(new_tram.clone());
            }
            None => {
                self.positions
                    .insert(new_tram.reporting_point, vec![new_tram.clone()]);
            }
        }

        match self.lines.get_mut(&new_tram.line) {
            Some(runs) => match runs.get_mut(&new_tram.run_number) {
                Some(tram) => {
                    *tram = new_tram;
                }
                None => {
                    runs.insert(new_tram.run_number, new_tram);
                }
            },
            None => {
                self.lines.insert(
                    new_tram.line,
                    HashMap::from([(new_tram.run_number, new_tram)]),
                );
            }
        }
    }
}

pub struct State {
    pub regions: HashMap<i64, Network>,
}

impl State {
    pub fn new() -> State {
        let default_stop_file = String::from("all.json");
        let stop_file = env::var("STOPS_FILE").unwrap_or(default_stop_file);

        let default_graph_file = String::from("all.json");
        let graph_file = env::var("GRAPH_FILE").unwrap_or(default_graph_file);

        let stop_data = fs::read_to_string(stop_file).expect("Unable to read file");
        let stop_json: LocationsJson = serde_json::from_str(&stop_data).unwrap();

        let graph_data = fs::read_to_string(graph_file).expect("Unable to read file");
        let graph_json: PositionGraph = serde_json::from_str(&graph_data).unwrap();

        let mut regions = HashMap::new();

        for (key, value) in stop_json.data {
            regions.insert(
                key,
                Network::new(value, graph_json.get(&key).unwrap().clone()),
            );
        }

        State { regions }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}
