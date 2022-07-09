extern crate serde_json;

mod graph;

use graph::Graph;

use telegrams::{R09GrpcTelegram};
use stop_names::{InterRegional, Region, RegionMetaInformation};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Tram {
    pub junction: u32, // germany wide or local ones
    pub line: u32,
    pub run_number: u32,
    pub time_stamp: u64,
    pub delayed: i32,
    pub direction: u32,
}

#[derive(Serialize, Debug, Clone)]
pub struct Network {
    pub lines: HashMap<u32, HashMap<u32, Tram>>,
    pub positions: HashMap<u32, Vec<Tram>>,
    pub edges: HashMap<(u32, u32), u32>,
    pub graph: Graph,
}

impl Network {
    pub fn new(graph: Graph) -> Network {
        Network {
            lines: HashMap::new(),
            graph: graph,
            positions: HashMap::new(),
            edges: HashMap::new(),
        }
    }

    pub fn query_tram(&self, line: &u32, run_number: &u32) -> Option<u32> {
        match self.lines.get(line) {
            Some(line) => line
                .get(run_number)
                .map_or(None, |tram| Some(tram.junction)),
            None => None,
        }
    }

    pub fn query_position(&mut self, position: &u32) -> Vec<Tram> {
        match self.positions.get(position) {
            Some(trams) => trams.to_vec(),
            None => Vec::new(),
        }
    }

    pub fn update(&mut self, telegram: &R09GrpcTelegram) {
        if telegram.line.is_none() || telegram.run_number.is_none() || telegram.delay.is_none() {
            return;
        }

        let new_tram = Tram {
            junction: telegram.junction,
            line: telegram.line.unwrap(),
            run_number: telegram.run_number.unwrap(),
            time_stamp: telegram.time,
            delayed: telegram.delay.unwrap(),
            direction: telegram.direction,
        };

        match self.positions.get_mut(&telegram.junction) {
            Some(trams) => {
                trams.push(new_tram.clone());
            }
            None => {
                self.positions
                    .insert(telegram.junction, vec![new_tram.clone()]);
            }
        }

        let mut _start_time: u64;
        let mut remove_index = 0;
        match self.lines.get(&telegram.line.unwrap()) {
            Some(_) => {
                {
                    //TODO the fucking unwrap
                    let data = self.lines.get_mut(&telegram.line.unwrap()).unwrap();
                    data.insert(telegram.run_number.unwrap(), new_tram.clone());
                }

                let mut previous = None;
                let possible_starts: Vec<u32> = self.graph.adjacent_paths(telegram.junction);
                for start in possible_starts {
                    // we now look up if there is a tram started from this position

                    let trams = self.query_position(&start);
                    for (i, found_tram) in trams.iter().enumerate() {
                        if found_tram.line == new_tram.line
                            && found_tram.run_number == new_tram.run_number
                        {
                            // maybe add destination here
                            previous = Some(found_tram.clone());
                            remove_index = i;
                            break;
                        }
                    }
                }

                if previous.is_some() {
                    let unwrapped = previous.unwrap();
                    //let new_time = self.lines.get(&telegram.line).unwrap().get(&telegram.run_number).unwrap().time_stamp;
                    let delta = telegram.time - unwrapped.time_stamp;
                    println!(
                        "Tram: Line: {} Run Number: {} followed path: {} -- {} -> {} Time: {}",
                        unwrapped.line,
                        unwrapped.run_number,
                        unwrapped.junction,
                        unwrapped.direction,
                        telegram.junction,
                        delta
                    );

                    self.positions
                        .get_mut(&unwrapped.junction)
                        .unwrap()
                        .remove(remove_index);
                    self.edges
                        .insert((unwrapped.junction, unwrapped.direction), delta as u32);
                }
            }
            None => {
                self.lines.insert(
                    telegram.line.unwrap(),
                    HashMap::from([(telegram.run_number.unwrap(), new_tram)]),
                );
            }
        }
    }
}

pub struct State {
    pub regions: HashMap<String, Network>,
}

impl State {
    pub fn new() -> State {
        let default_graph_file = String::from("graph.json");
        let graph_file = env::var("GRAPH_FILE").unwrap_or(default_graph_file);

        let data = fs::read_to_string(graph_file).expect("Unable to read file");
        let res: HashMap<String, Graph> = serde_json::from_str(&data).unwrap();
        let mut regions = HashMap::new();

        for (key, value) in res {
            regions.insert(key, Network::new(value));
        }

        State { regions: regions }
    }
}
