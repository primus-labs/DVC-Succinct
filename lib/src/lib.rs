use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct PublicValuesStruct {
    pub task_id: String,
    pub report_tx_hash: String,
    pub attestor: String,
    pub base_urls: Vec<String>,
    pub asset_balance: HashMap<String, f64>,
    pub timestamp: u128,
    pub status: i16,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct SP1ZktlsProofFixture {
    pub public_values: PublicValuesStruct,
    pub vk: String,
    pub proof_id: String,
    pub proof: String,
}
