use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;

pub use dvb_dump::receives_telegrams_server::{ReceivesTelegrams, ReceivesTelegramsServer};
pub use dvb_dump::{ReducedTelegram, ReturnCode};

pub mod dvb_dump {
    tonic::include_proto!("dvbdump");
}

use super::Stop;

#[derive(Debug, Serialize)]
pub struct WebSocketTelegram {
    #[serde(flatten)]
    pub reduced: ReducedTelegram,

    #[serde(flatten)]
    pub meta_data: Stop,
}

impl Serialize for ReducedTelegram {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("ReducedTelegram", 11)?;
        s.serialize_field("time_stamp", &self.time_stamp)?;
        s.serialize_field("position_id", &self.position_id)?;
        s.serialize_field("direction", &self.direction)?;
        s.serialize_field("status", &self.status)?;
        s.serialize_field("line", &self.line)?;
        s.serialize_field("delay", &self.delay)?;
        s.serialize_field("destination_number", &self.destination_number)?;
        s.serialize_field("train_length", &self.train_length)?;
        s.serialize_field("run_number", &self.run_number)?;
        s.serialize_field("region_code", &self.region_code)?;
        s.end()
    }
}
