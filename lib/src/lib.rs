use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct PublicValuesStruct {
    pub source: String,
    pub recipient: String,
    pub phala_average_balance: f64,
    pub source_user: String,
    pub meet_up_time: bool,
}


#[derive(Serialize, Deserialize, Default, Debug)]
pub struct SP1ZktlsProofFixture {
    pub public_values: PublicValuesStruct,
    pub vk: String,
    pub proof_id: String,
    pub proof: String,
}
