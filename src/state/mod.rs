extern crate serde_json;

mod graph;

use dump_dvb::locations::{
    LineSegment, LocationsJson, RegionReportLocations, RequestStatus, Segments,
};
use dump_dvb::telegrams::r09::R09GrpcTelegram;

use log::info;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use chrono::{Utc, NaiveDateTime};

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
}

impl Network {
    pub fn new(model: RegionReportLocations) -> Network {
        Network {
            lines: HashMap::new(),
            positions: HashMap::new(),
            model,
        }
    }

    pub fn query_tram(&self, line: &u32, run_number: &u32) -> Option<i32> {
        match self.lines.get(line) {
            Some(line) => line
                .get(run_number)
                .map_or(None, |tram| Some(tram.reporting_point)),
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

        let request_status = match RequestStatus::from_i16(telegram.request_status as i16) {
            Some(status) => status,
            None => {
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
            request_status: request_status,
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
            Some(runs) => {
                match runs.get_mut(&new_tram.run_number) {
                    Some(tram) => {
                        *tram = new_tram;
                    }
                    None => {
                        runs.insert(new_tram.run_number, new_tram);
                    }
                }
            }
            None => {
                self.lines.insert(new_tram.line, HashMap::from([(new_tram.run_number, new_tram)]));
            }
        }
    }
}

pub struct State {
    pub regions: HashMap<i32, Network>,
}

impl State {
    pub fn new() -> State {
        let default_graph_file = String::from("all.json");
        let graph_file = env::var("STOPS_FILE").unwrap_or(default_graph_file);

        let data = fs::read_to_string(graph_file).expect("Unable to read file");
        let res: LocationsJson = serde_json::from_str(&data).unwrap();
        let mut regions = HashMap::new();

        for (key, value) in res.data {
            regions.insert(key, Network::new(value));
        }

        State { regions }
    }
}
