// binance.rs
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};


// ---------- Bundle to hold parsed results ----------
#[derive(Debug, Serialize, Deserialize)]
pub struct BinanceData {
    pub purchases: Option<ApiResponse<Vec<PurchaseRecord>>>,
    pub redeems:   Option<ApiResponse<Vec<RedeemRecord>>>,
    pub assets:    Option<ApiResponse<AssetData>>,
}

/// Generic API response wrapper.
/// Different Binance endpoints can reuse this with various `T` types.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub code: String,
    pub message: Option<String>,
    #[serde(rename = "messageDetail")]
    pub message_detail: Option<String>,
    pub data: T,
    pub success: bool,
}

// =======Position info
#[derive(Debug, Serialize, Deserialize)]
pub struct PositionInfo {
    #[serde(rename = "userId")]
    pub user_id: String,

    pub asset: String,
    pub token: String,

    #[serde(rename = "productId")]
    pub product_id: String,

    #[serde(rename = "productName")]
    pub product_name: String,

    pub apr: String,

    #[serde(rename = "dailyInterestRate")]
    pub daily_interest_rate: Option<String>,

    #[serde(rename = "annualInterestRate")]
    pub annual_interest_rate: String,

    #[serde(rename = "avgAnnualInterestRate")]
    pub avg_annual_interest_rate: Option<String>,

    #[serde(rename = "marketApr")]
    pub market_apr: String,

    #[serde(rename = "exchangeRate")]
    pub exchange_rate: String,

    #[serde(rename = "tokenAmount")]
    pub token_amount: String,

    #[serde(rename = "totalAmount")]
    pub total_amount: String,

    #[serde(rename = "experienceCouponTotalInterest")]
    pub experience_coupon_total_interest: Option<String>,

    #[serde(rename = "lockedAmount")]
    pub locked_amount: String,

    #[serde(rename = "freeAmount")]
    pub free_amount: String,

    #[serde(rename = "freezeAmount")]
    pub freeze_amount: String,

    #[serde(rename = "totalInterest")]
    pub total_interest: String,

    #[serde(rename = "expectedInterest")]
    pub expected_interest: Option<String>,

    #[serde(rename = "canRedeem")]
    pub can_redeem: bool,

    #[serde(rename = "redeemingAmount")]
    pub redeeming_amount: String,

    #[serde(rename = "redeemingRecordList")]
    pub redeeming_record_list: Option<Vec<String>>,
}




/// ========= Asset Overview (first response) =========

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetData {
    pub assets: Vec<String>,
    #[serde(rename = "assetDetails")]
    pub asset_details: Vec<AssetDetail>,
    #[serde(rename = "productDetails")]
    pub product_details: Vec<ProductDetail>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetDetail {
    pub asset: String,
    #[serde(rename = "asset2")]
    pub asset2: Option<String>,
    pub amount: String,
    #[serde(rename = "amountInBTC")]
    pub amount_in_btc: String,
    #[serde(rename = "amountInUSD")]
    pub amount_in_usd: String,
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(rename = "pdTradeDeadline")]
    pub pd_trade_deadline: Option<String>,
    #[serde(rename = "pdDepositDeadline")]
    pub pd_deposit_deadline: Option<String>,
    #[serde(rename = "pdAnnounceUrl")]
    pub pd_announce_url: Option<String>,
    #[serde(rename = "isPhaseOut")]
    pub is_phase_out: bool,
    pub percentage: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProductDetail {
    #[serde(rename = "businessType")]
    pub business_type: String,
    #[serde(rename = "amountInBTC")]
    pub amount_in_btc: String,
    #[serde(rename = "amountInUSD")]
    pub amount_in_usd: String,
    pub percentage: String,
}

/// ========= Redeem Record (second response) =========

#[derive(Debug, Serialize, Deserialize)]
pub struct RedeemRecord {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "projectId")]
    pub project_id: String,
    pub lot: Option<String>,
    #[serde(rename = "createTimestamp")]
    pub create_timestamp: String, // Millisecond timestamp as string
    pub amount: String,
    pub principal: String,
    pub interest: Option<String>,
    #[serde(rename = "payedPrincipal")]
    pub payed_principal: Option<String>,
    #[serde(rename = "payedInterest")]
    pub payed_interest: Option<String>,
    #[serde(rename = "startTime")]
    pub start_time: Option<String>,
    pub asset: String,
    #[serde(rename = "projectName")]
    pub project_name: String,
    pub status: String,
    #[serde(rename = "lendingType")]
    pub lending_type: String,
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(rename = "clientRedeemId")]
    pub client_redeem_id: Option<String>,
    #[serde(rename = "deliverDate")]
    pub deliver_date: Option<String>,
    pub id: String,
    #[serde(rename = "currencyTarget")]
    pub currency_target: String,
}

/// ========= Purchase Record (third response) =========

#[derive(Debug, Serialize, Deserialize)]
pub struct PurchaseRecord {
    pub id: String,
    #[serde(rename = "createTimestamp")]
    pub create_timestamp: String, // Millisecond timestamp as string
    #[serde(rename = "productName")]
    pub product_name: String,
    #[serde(rename = "productId")]
    pub product_id: String,
    #[serde(rename = "userId")]
    pub user_id: String,
    pub asset: String,
    pub lot: String,
    pub amount: String,
    #[serde(rename = "startTime")]
    pub start_time: Option<String>,
    pub status: String,
    #[serde(rename = "lendingType")]
    pub lending_type: String,
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(rename = "clientPurchaseId")]
    pub client_purchase_id: Option<String>,
    #[serde(rename = "currencySource")]
    pub currency_source: String,
    #[serde(rename = "currencySourceDetail")]
    pub currency_source_detail: Option<String>,
}

/// ========= Parsing Helpers =========

/// Parse the asset summary response.
#[allow(dead_code)]
pub fn parse_assets(json: &str) -> Result<ApiResponse<AssetData>> {
    serde_json::from_str(json).context("parse_assets: invalid JSON")
}

/// Parse the redeem record list response.
#[allow(dead_code)]
pub fn parse_redeems(json: &str) -> Result<ApiResponse<Vec<RedeemRecord>>> {
    serde_json::from_str(json).context("parse_redeems: invalid JSON")
}

/// Parse the purchase record list response.
#[allow(dead_code)]
pub fn parse_purchases(json: &str) -> Result<ApiResponse<Vec<PurchaseRecord>>> {
    serde_json::from_str(json).context("parse_purchases: invalid JSON")
}

/// ========= Utility Helpers =========

/// Convert string amount to `f64`.
/// Returns `None` if parsing fails.
#[allow(dead_code)]
pub fn parse_amount(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok()
}

/// Sum up all `amount_in_usd` fields from the asset details.
#[allow(dead_code)]
pub fn total_usd(asset_data: &AssetData) -> f64 {
    asset_data
        .asset_details
        .iter()
        .filter_map(|d| parse_amount(&d.amount_in_usd))
        .sum()
}

/// Convert a millisecond timestamp string to seconds (`i64`).
#[allow(dead_code)]
pub fn ts_ms_to_secs(ms_str: &str) -> Option<i64> {
    let ms = ms_str.trim().parse::<i64>().ok()?;
    Some(ms / 1_000)
}

/// Get the latest purchase record (by timestamp).
#[allow(dead_code)]
pub fn latest_purchase<'a>(list: &'a [PurchaseRecord]) -> Option<&'a PurchaseRecord> {
    list.iter().max_by_key(|r| r.create_timestamp.as_str())
}

/// Get the latest redeem record (by timestamp).
#[allow(dead_code)]
pub fn latest_redeem<'a>(list: &'a [RedeemRecord]) -> Option<&'a RedeemRecord> {
    list.iter().max_by_key(|r| r.create_timestamp.as_str())
}
