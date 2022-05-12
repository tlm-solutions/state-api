use std::collections::HashMap;
use std::fs::{File};
use std::io::Read;
use std::env;
use serde::{Serialize, Deserialize};


#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Graph {
    pub structure: HashMap<u32, HashMap<u32, u32>>
}


impl Graph {
    pub fn from_file(path: &String) -> Graph {

        let mut file = File::open(path).unwrap();
        let mut data = String::new();
        file.read_to_string(&mut data).unwrap();

        let json: HashMap<u32, HashMap<u32, u32>> = serde_json::from_str(&data).expect("JSON was not well-formatted");

        Graph {
            structure: json
        }
    }

    pub fn adjacent_paths(&self, position: u32) -> Vec<u32> {
        match self.structure.get(&position) {
            Some(vertex) => {
                vertex.clone().into_values().collect::<Vec<u32>>()
            }
            None => {
                Vec::new()
            }
        }
    }
}
