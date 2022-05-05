extern crate serde_json;

use std::collections::HashMap;
use serde::{Serialize, Deserialize, Serializer};
use super::{ReducedTelegram};

#[derive(Serialize, Debug, Clone)]
pub struct Tram {
    pub position_id: u32, // germany wide or local ones
    pub time_stamp: u64,
    pub delayed: i32,
    pub direction: u32
}


#[derive(Serialize, Debug, Clone)]
pub struct Line {
    pub trams: HashMap<u32, Tram>
}

#[derive(Serialize, Debug, Clone)]
pub struct Network {
    pub lines: HashMap<u32, Line>
}



impl Network {
    pub fn new() -> Network {
        Network {
            lines: HashMap::new()
        }
    }

    pub fn update(&mut self, telegram: &ReducedTelegram) {
         let tram = Tram {
            position_id: telegram.position_id,
            time_stamp: telegram.time_stamp,
            delayed: telegram.delay,
            direction: telegram.direction
        };
        match self.lines.get(&telegram.line) {
            Some(_)=> {
                let data = self.lines.get_mut(&telegram.line).unwrap();
                data.trams.insert(telegram.run_number, tram);
            }
            None => {
                self.lines.insert(telegram.line, Line {trams: HashMap::from([(telegram.run_number, tram)])});
            }
        }
    }
}


