// These two lines are necessary for the program to properly compile.
//
// Under the hood, we wrap your main function with some extra code so that it behaves properly
// inside the zkVM.
#![no_main]
sp1_zkvm::entrypoint!(main);

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sp1_zkvm::io::commit;
use std::collections::{HashMap, HashSet};
use zktls_att_verification::attestation_data::verify_attestation_data;

mod errors;
use errors::{ZkErrorCode, ZktlsError};

// const RISK_URL: &str = "https://papi.binance.com/papi/v1/um/positionRisk";
// const BALANCE_URL: &str = "https://papi.binance.com/papi/v1/balance";
const RISK_URL: &str = "https://exchange.unipay.dev/public/positionRisk";
const BALANCE_URL: &str = "https://exchange.unipay.dev/public/balance";
const STABLE_COINS: &[&str] = &[
    "USDT", "USDC", "FDUSD", "TUSD", "USDE", "XUSD", "USD1", "BFUSD", "USDP", "DAI",
];

#[derive(Serialize, Deserialize, Default, Debug)]
struct PublicValueStruct {
    attestor: String,
    base_urls: Vec<String>,
    asset_balance: HashMap<String, f64>,
    timestamp: u128,
    status: i16,
}

fn app_main(pv: &mut PublicValueStruct) -> Result<(), ZktlsError> {
    let attestation_data: String = sp1_zkvm::io::read();

    //
    // 0. Make attestation config
    let v: serde_json::Value = serde_json::from_str(&attestation_data)
        .map_err(|e| zkerr!(ZkErrorCode::ParseAttestationData, e.to_string()))?;
    let attestor_addr = v
        .get("public_data")
        .and_then(|pd| pd.get(0))
        .and_then(|item| item.get("attestor"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| zkerr!(ZkErrorCode::GetAttestorAddressFail))?;
    let attestion_confg = json!({
        "attestor_addr": attestor_addr,
        "url": [RISK_URL, BALANCE_URL]
    });
    pv.attestor = attestor_addr.to_string();
    pv.base_urls.push(RISK_URL.to_string());
    pv.base_urls.push(BALANCE_URL.to_string());

    //
    // 1. Verify
    let (attestation_data, _, messages) = verify_attestation_data(&attestation_data, &attestion_confg.to_string())
        .map_err(|e| zkerr!(ZkErrorCode::VerifyAttestation, e.to_string()))?;

    //
    // 2. Do some valid checks
    // In the vast majority of cases, it is legal. Data is extracted while the inspection is conducted.
    let msg_len = messages[0].len();
    let requests = attestation_data.public_data[0].attestation.request.clone();
    let requests_len = requests.len();
    ensure_zk!(requests_len % 2 == 0, zkerr!(ZkErrorCode::InvalidRequestLength));
    ensure_zk!(requests_len == msg_len, zkerr!(ZkErrorCode::InvalidMessagesLength));

    let mut i = 0;
    let mut um_paths = vec![];
    um_paths.push("$.[*].symbol");
    um_paths.push("$.[*].entryPrice");

    let mut bal_paths = vec![];
    bal_paths.push("$.[*].asset");
    bal_paths.push("$.[*].totalWalletBalance");
    bal_paths.push("$.[*].umUnrealizedPNL");

    pv.timestamp = u128::MAX;
    let mut asset_bals = HashMap::new();
    let mut um_prices = vec![];
    // strict order: um1 bal1 um2 bal2 ...
    for request in requests {
        let ts = request
            .url
            .split("timestamp=")
            .nth(1)
            .and_then(|s| s.split('&').next())
            .filter(|s| !s.is_empty())
            .ok_or(zkerr!(ZkErrorCode::CannotFoundTimestamp))?
            .parse::<u128>()
            .map_err(|_| zkerr!(ZkErrorCode::ParseTimestampFailed))?;
        pv.timestamp = pv.timestamp.min(ts);

        // check url and get assets' balance
        if request.url.starts_with(RISK_URL) {
            ensure_zk!(i % 2 == 0, zkerr!(ZkErrorCode::InvalidRequestOrder));

            let json_value = messages[0][i]
                .get_json_values(&um_paths)
                .map_err(|e| zkerr!(ZkErrorCode::GetJsonValueFail, e.to_string()))?;

            ensure_zk!(
                json_value.len() % um_paths.len() == 0,
                zkerr!(ZkErrorCode::InvalidJsonValueSize)
            );

            // Collects UM (asset => entryPrice) info
            let mut prices = vec![];
            let size = json_value.len() / um_paths.len();
            for j in 0..size {
                let asset = json_value[j].trim_matches('"').to_ascii_uppercase();
                let price = json_value[size + j].trim_matches('"').to_string();
                let v = format!("{}:{}", asset, price);
                prices.push(v);
            }
            prices.sort();
            let um_price = prices.join(",");
            um_prices.push(um_price);
        } else if request.url.starts_with(BALANCE_URL) {
            let json_value = messages[0][i]
                .get_json_values(&bal_paths)
                .map_err(|e| zkerr!(ZkErrorCode::GetJsonValueFail, e.to_string()))?;

            ensure_zk!(
                json_value.len() % bal_paths.len() == 0,
                zkerr!(ZkErrorCode::InvalidJsonValueSize)
            );

            let size = json_value.len() / bal_paths.len();
            for j in 0..size {
                let asset = json_value[j].trim_matches('"').to_ascii_uppercase();
                let bal: f64 = json_value[size + j].trim_matches('"').parse().unwrap_or(0.0);
                let pnl: f64 = json_value[size * 2 + j].trim_matches('"').parse().unwrap_or(0.0);
                *asset_bals.entry(asset.to_string()).or_insert(0.0) += bal + pnl;
            }
        } else {
            return Err(zkerr!(ZkErrorCode::InvalidRequestUrl));
        }

        i += 1;
    }

    // Is the account duplicate?
    let mut seen = HashSet::new();
    ensure_zk!(
        !um_prices.iter().any(|x| !seen.insert(x)),
        zkerr!(ZkErrorCode::DuplicateAccount)
    );

    // Summary by Category
    let mut stablecoin_sum = 0.0;
    for (k, v) in asset_bals {
        if STABLE_COINS.contains(&k.as_str()) {
            stablecoin_sum += v;
        } else {
            pv.asset_balance.insert(k, v);
        }
    }
    pv.asset_balance.insert("STABLECOIN".to_string(), stablecoin_sum);

    Ok(())
}

pub fn main() {
    let mut pv = PublicValueStruct::default();
    if let Err(e) = app_main(&mut pv) {
        println!("Error: {} {}", e.icode(), e.msg());
        pv.status = e.icode();
    } else {
        println!("OK");
    }
    commit(&pv);
    println!("{:#?}", pv);
}
