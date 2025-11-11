use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct VmStatus {
    pub vm_uuid: String,
    pub status: String,
    pub uptime: String,
    pub in_progress: bool,
    pub boot_progress: Option<String>,
    pub boot_error: Option<String>,
    pub operation_type: Option<String>,
    pub operation_started_at: Option<String>,
    pub correlation_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub username: String,
    pub email: String,
    pub credits: f64,
    pub granted_credits: f64,
    pub role: String,
    pub avatar: String,
    pub flag_reset_password: bool,
    pub flag_has_password: bool,
    pub team_name: String,
    pub team_tier: String,
    pub trial_ended_at: Option<String>,
    pub email_verified: bool,
    pub totp_enabled: bool,
    pub backup_codes_count: i64,
    pub is_post_paid: bool,
    pub outstanding_amount: f64,
}



pub type VmStatusMap = HashMap<String, VmStatus>;




