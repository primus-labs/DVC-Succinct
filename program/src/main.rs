// These two lines are necessary for the program to properly compile.
//
// Under the hood, we wrap your main function with some extra code so that it behaves properly
// inside the zkVM.
#![no_main]
sp1_zkvm::entrypoint!(main);

use anyhow::Result;
use serde_json::json;
use sp1_zkvm::io::commit;
use std::collections::{HashMap, HashSet};
use zktls_att_verification::attestation_data::verify_attestation_data;
use zktls_lib::{AttestationMetaStruct, PublicValuesStruct};

mod errors;
use errors::{ZkErrorCode, ZktlsError};

const RISK_URL: &str = "https://papi.binance.com/papi/v1/um/positionRisk";
const BALANCE_URL: &str = "https://papi.binance.com/papi/v1/balance";
const SPOT_BALANCE_URL: &str = "https://api.binance.com/api/v3/account";
const FEATURE_BALANCE_URL: &str = "https://fapi.binance.com/fapi/v3/balance";

const ASTER_SPOT_BALANCE_URL: &str = "https://sapi.asterdex.com/api/v1/account";
const ASTER_FEATURE_BALANCE_URL: &str = "https://fapi.asterdex.com/fapi/v2/balance";

const STABLE_COINS: &[&str] = &[
    "USDT", "USDC", "FDUSD", "TUSD", "USDE", "XUSD", "USD1", "BFUSD", "USDP", "DAI",
];
const ATTESTORS: &[&str] = &[
    "0xd638c623833aeb02c8049837bdd54e02540e7031",
    "0x3d436d4c130e7e80df715f07b6ca5db927dd45f2",
    "0x2f211ef8068ff70c8d851c145baca53ccec0aa07",
    "0x6bfa68fab4d930f19c281f5f1f57a2e4ede5a848",
    "0x96c3cac72a914eb0e6a1d74cdf2c8d6fa9d02320",
    "0x172f48f7aa734ee18ab7fa3413dc4d974866a3ae",
];

const EPSILON_VALUE: f64 = 0.00000000001;

fn app_binance_unified(
    pv: &mut AttestationMetaStruct,
    attestation_data: &String,
    asset_bals: &mut HashMap<String, f64>,
) -> Result<(), ZktlsError> {
    //
    // 0. Make attestation config
    let v: serde_json::Value = serde_json::from_str(&attestation_data)
        .map_err(|e| zkerr!(ZkErrorCode::ParseAttestationData, e.to_string()))?;
    let task_id = v
        .get("public_data")
        .and_then(|pd| pd.get(0))
        .and_then(|item| item.get("taskId"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| zkerr!(ZkErrorCode::GetTaskIdFail))?;
    let report_tx_hash = v
        .get("public_data")
        .and_then(|pd| pd.get(0))
        .and_then(|item| item.get("reportTxHash"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| zkerr!(ZkErrorCode::GetReportTxHashFail))?;
    let attestor_addr = v
        .get("public_data")
        .and_then(|pd| pd.get(0))
        .and_then(|item| item.get("attestor"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| zkerr!(ZkErrorCode::GetAttestorAddressFail))?;
    ensure_zk!(
        ATTESTORS.contains(&attestor_addr.to_ascii_lowercase().as_str()),
        zkerr!(ZkErrorCode::InvalidAttestor)
    );
    let attestion_confg = json!({
        "attestor_addr": attestor_addr,
        "url": [RISK_URL, BALANCE_URL]
    });
    pv.task_id = task_id.to_string();
    pv.report_tx_hash = report_tx_hash.to_string();
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
            if !um_price.is_empty() {
                um_prices.push(um_price);
            }
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

    Ok(())
}

fn app_binance_spot(
    pv: &mut AttestationMetaStruct,
    attestation_data: &String,
    asset_bals: &mut HashMap<String, f64>,
) -> Result<(), ZktlsError> {
    //
    // 0. Make attestation config
    let v: serde_json::Value = serde_json::from_str(&attestation_data)
        .map_err(|e| zkerr!(ZkErrorCode::ParseAttestationData, e.to_string()))?;
    let task_id = v
        .get("public_data")
        .and_then(|pd| pd.get(0))
        .and_then(|item| item.get("taskId"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| zkerr!(ZkErrorCode::GetTaskIdFail))?;
    let report_tx_hash = v
        .get("public_data")
        .and_then(|pd| pd.get(0))
        .and_then(|item| item.get("reportTxHash"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| zkerr!(ZkErrorCode::GetReportTxHashFail))?;
    let attestor_addr = v
        .get("public_data")
        .and_then(|pd| pd.get(0))
        .and_then(|item| item.get("attestor"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| zkerr!(ZkErrorCode::GetAttestorAddressFail))?;
    ensure_zk!(
        ATTESTORS.contains(&attestor_addr.to_ascii_lowercase().as_str()),
        zkerr!(ZkErrorCode::InvalidAttestor)
    );
    let attestion_confg = json!({
        "attestor_addr": attestor_addr,
        "url": [SPOT_BALANCE_URL]
    });
    pv.task_id = task_id.to_string();
    pv.report_tx_hash = report_tx_hash.to_string();
    pv.attestor = attestor_addr.to_string();
    pv.base_urls.push(SPOT_BALANCE_URL.to_string());

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
    ensure_zk!(requests_len == msg_len, zkerr!(ZkErrorCode::InvalidMessagesLength));

    let mut i = 0;
    let mut uid_paths = vec![];
    uid_paths.push("$.uid");

    let mut bal_paths = vec![];
    bal_paths.push("$.balances[*].asset");
    bal_paths.push("$.balances[*].free");
    bal_paths.push("$.balances[*].locked");

    pv.timestamp = u128::MAX;
    let mut uids = vec![];
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

        // check url
        if !request.url.starts_with(SPOT_BALANCE_URL) {
            return Err(zkerr!(ZkErrorCode::InvalidRequestUrl));
        }

        {
            // uid
            let json_value = messages[0][i]
                .get_json_values(&uid_paths)
                .map_err(|e| zkerr!(ZkErrorCode::GetJsonValueFail, e.to_string()))?;

            ensure_zk!(json_value.len() == 1, zkerr!(ZkErrorCode::InvalidJsonValueSize));

            let uid = json_value[0].trim_matches('"').to_string();
            uids.push(uid);
        }

        {
            // balance
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
                let free: f64 = json_value[size + j].trim_matches('"').parse().unwrap_or(0.0);
                let locked: f64 = json_value[size * 2 + j].trim_matches('"').parse().unwrap_or(0.0);
                *asset_bals.entry(asset.to_string()).or_insert(0.0) += free + locked;
            }
        }

        i += 1;
    }

    // Is the account duplicate?
    let mut seen = HashSet::new();
    ensure_zk!(
        !uids.iter().any(|x| !seen.insert(x)),
        zkerr!(ZkErrorCode::DuplicateAccount)
    );

    Ok(())
}

fn app_binance_feature(
    pv: &mut AttestationMetaStruct,
    attestation_data: &String,
    asset_bals: &mut HashMap<String, f64>,
) -> Result<(), ZktlsError> {
    //
    // 0. Make attestation config
    let v: serde_json::Value = serde_json::from_str(&attestation_data)
        .map_err(|e| zkerr!(ZkErrorCode::ParseAttestationData, e.to_string()))?;
    let task_id = v
        .get("public_data")
        .and_then(|pd| pd.get(0))
        .and_then(|item| item.get("taskId"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| zkerr!(ZkErrorCode::GetTaskIdFail))?;
    let report_tx_hash = v
        .get("public_data")
        .and_then(|pd| pd.get(0))
        .and_then(|item| item.get("reportTxHash"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| zkerr!(ZkErrorCode::GetReportTxHashFail))?;
    let attestor_addr = v
        .get("public_data")
        .and_then(|pd| pd.get(0))
        .and_then(|item| item.get("attestor"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| zkerr!(ZkErrorCode::GetAttestorAddressFail))?;
    ensure_zk!(
        ATTESTORS.contains(&attestor_addr.to_ascii_lowercase().as_str()),
        zkerr!(ZkErrorCode::InvalidAttestor)
    );
    let attestion_confg = json!({
        "attestor_addr": attestor_addr,
        "url": [FEATURE_BALANCE_URL]
    });
    pv.task_id = task_id.to_string();
    pv.report_tx_hash = report_tx_hash.to_string();
    pv.attestor = attestor_addr.to_string();
    pv.base_urls.push(FEATURE_BALANCE_URL.to_string());

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
    ensure_zk!(requests_len == msg_len, zkerr!(ZkErrorCode::InvalidMessagesLength));

    let mut i = 0;
    let mut uid_paths = vec![];
    uid_paths.push("$.[*].accountAlias");

    let mut bal_paths = vec![];
    bal_paths.push("$.[*].asset");
    bal_paths.push("$.[*].balance");
    bal_paths.push("$.[*].crossUnPnl");

    pv.timestamp = u128::MAX;
    let mut uids = vec![];
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

        // check url
        if !request.url.starts_with(FEATURE_BALANCE_URL) {
            return Err(zkerr!(ZkErrorCode::InvalidRequestUrl));
        }

        {
            // uid
            let json_value = messages[0][i]
                .get_json_values(&uid_paths)
                .map_err(|e| zkerr!(ZkErrorCode::GetJsonValueFail, e.to_string()))?;
            if json_value.len() == 0 {
                continue; // no any data of feature response
            }

            ensure_zk!(json_value.len() > 0, zkerr!(ZkErrorCode::InvalidJsonValueSize));

            let uid = json_value[0].trim_matches('"').to_string();
            uids.push(uid);
        }

        {
            // balance
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
                let un_pnl: f64 = json_value[size * 2 + j].trim_matches('"').parse().unwrap_or(0.0);
                *asset_bals.entry(asset.to_string()).or_insert(0.0) += bal + un_pnl;
            }
        }

        i += 1;
    }

    // Is the account duplicate?
    let mut seen = HashSet::new();
    ensure_zk!(
        !uids.iter().any(|x| !seen.insert(x)),
        zkerr!(ZkErrorCode::DuplicateAccount)
    );

    Ok(())
}

fn app_aster_spot(
    pv: &mut AttestationMetaStruct,
    attestation_data: &String,
    asset_bals: &mut HashMap<String, f64>,
) -> Result<(), ZktlsError> {
    //
    // 0. Make attestation config
    let v: serde_json::Value = serde_json::from_str(&attestation_data)
        .map_err(|e| zkerr!(ZkErrorCode::ParseAttestationData, e.to_string()))?;
    let task_id = v
        .get("public_data")
        .and_then(|pd| pd.get(0))
        .and_then(|item| item.get("taskId"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| zkerr!(ZkErrorCode::GetTaskIdFail))?;
    let report_tx_hash = v
        .get("public_data")
        .and_then(|pd| pd.get(0))
        .and_then(|item| item.get("reportTxHash"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| zkerr!(ZkErrorCode::GetReportTxHashFail))?;
    let attestor_addr = v
        .get("public_data")
        .and_then(|pd| pd.get(0))
        .and_then(|item| item.get("attestor"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| zkerr!(ZkErrorCode::GetAttestorAddressFail))?;
    ensure_zk!(
        ATTESTORS.contains(&attestor_addr.to_ascii_lowercase().as_str()),
        zkerr!(ZkErrorCode::InvalidAttestor)
    );
    let attestion_confg = json!({
        "attestor_addr": attestor_addr,
        "url": [ASTER_SPOT_BALANCE_URL]
    });
    pv.task_id = task_id.to_string();
    pv.report_tx_hash = report_tx_hash.to_string();
    pv.attestor = attestor_addr.to_string();
    pv.base_urls.push(ASTER_SPOT_BALANCE_URL.to_string());

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
    ensure_zk!(requests_len == msg_len, zkerr!(ZkErrorCode::InvalidMessagesLength));

    let mut i = 0;
    let mut uid_paths = vec![];
    uid_paths.push("$.updateTime");

    let mut bal_paths = vec![];
    bal_paths.push("$.balances[*].asset");
    bal_paths.push("$.balances[*].free");
    bal_paths.push("$.balances[*].locked");

    pv.timestamp = u128::MAX;
    let mut uids = vec![];
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

        // check url
        if !request.url.starts_with(ASTER_SPOT_BALANCE_URL) {
            return Err(zkerr!(ZkErrorCode::InvalidRequestUrl));
        }

        let update_time;
        {
            // uid
            let json_value = messages[0][i]
                .get_json_values(&uid_paths)
                .map_err(|e| zkerr!(ZkErrorCode::GetJsonValueFail, e.to_string()))?;

            ensure_zk!(json_value.len() == 1, zkerr!(ZkErrorCode::InvalidJsonValueSize));

            update_time = json_value[0].trim_matches('"').to_string();
        }

        {
            // balance
            let json_value = messages[0][i]
                .get_json_values(&bal_paths)
                .map_err(|e| zkerr!(ZkErrorCode::GetJsonValueFail, e.to_string()))?;

            ensure_zk!(
                json_value.len() % bal_paths.len() == 0,
                zkerr!(ZkErrorCode::InvalidJsonValueSize)
            );

            let mut _uid = vec![];
            let size = json_value.len() / bal_paths.len();
            for j in 0..size {
                let asset = json_value[j].trim_matches('"').to_ascii_uppercase();
                let free: f64 = json_value[size + j].trim_matches('"').parse().unwrap_or(0.0);
                let locked: f64 = json_value[size * 2 + j].trim_matches('"').parse().unwrap_or(0.0);
                *asset_bals.entry(asset.to_string()).or_insert(0.0) += free + locked;

                // for uid check
                let v = format!("{}:{}:{}", asset, free, locked);
                _uid.push(v);
            }
            _uid.sort();
            let _uid = _uid.join(",");
            if !_uid.is_empty() {
                let _uid = format!("{}:{}", update_time, _uid);
                uids.push(_uid);
            }
        }

        i += 1;
    }

    // Is the account duplicate?
    let mut seen = HashSet::new();
    ensure_zk!(
        !uids.iter().any(|x| !seen.insert(x)),
        zkerr!(ZkErrorCode::DuplicateAccount)
    );

    Ok(())
}

fn app_aster_feature(
    pv: &mut AttestationMetaStruct,
    attestation_data: &String,
    asset_bals: &mut HashMap<String, f64>,
) -> Result<(), ZktlsError> {
    //
    // 0. Make attestation config
    let v: serde_json::Value = serde_json::from_str(&attestation_data)
        .map_err(|e| zkerr!(ZkErrorCode::ParseAttestationData, e.to_string()))?;
    let task_id = v
        .get("public_data")
        .and_then(|pd| pd.get(0))
        .and_then(|item| item.get("taskId"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| zkerr!(ZkErrorCode::GetTaskIdFail))?;
    let report_tx_hash = v
        .get("public_data")
        .and_then(|pd| pd.get(0))
        .and_then(|item| item.get("reportTxHash"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| zkerr!(ZkErrorCode::GetReportTxHashFail))?;
    let attestor_addr = v
        .get("public_data")
        .and_then(|pd| pd.get(0))
        .and_then(|item| item.get("attestor"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| zkerr!(ZkErrorCode::GetAttestorAddressFail))?;
    ensure_zk!(
        ATTESTORS.contains(&attestor_addr.to_ascii_lowercase().as_str()),
        zkerr!(ZkErrorCode::InvalidAttestor)
    );
    let attestion_confg = json!({
        "attestor_addr": attestor_addr,
        "url": [ASTER_FEATURE_BALANCE_URL]
    });
    pv.task_id = task_id.to_string();
    pv.report_tx_hash = report_tx_hash.to_string();
    pv.attestor = attestor_addr.to_string();
    pv.base_urls.push(ASTER_FEATURE_BALANCE_URL.to_string());

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
    ensure_zk!(requests_len == msg_len, zkerr!(ZkErrorCode::InvalidMessagesLength));

    let mut i = 0;
    let mut uid_paths = vec![];
    uid_paths.push("$.[*].accountAlias");

    let mut bal_paths = vec![];
    bal_paths.push("$.[*].asset");
    bal_paths.push("$.[*].balance");
    bal_paths.push("$.[*].crossUnPnl");

    pv.timestamp = u128::MAX;
    let mut uids = vec![];
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

        // check url
        if !request.url.starts_with(ASTER_FEATURE_BALANCE_URL) {
            return Err(zkerr!(ZkErrorCode::InvalidRequestUrl));
        }

        {
            // uid
            let json_value = messages[0][i]
                .get_json_values(&uid_paths)
                .map_err(|e| zkerr!(ZkErrorCode::GetJsonValueFail, e.to_string()))?;
            if json_value.len() == 0 {
                continue; // no any data of feature response
            }

            ensure_zk!(json_value.len() > 0, zkerr!(ZkErrorCode::InvalidJsonValueSize));

            let uid = json_value[0].trim_matches('"').to_string();
            uids.push(uid);
        }

        {
            // balance
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
                let un_pnl: f64 = json_value[size * 2 + j].trim_matches('"').parse().unwrap_or(0.0);
                *asset_bals.entry(asset.to_string()).or_insert(0.0) += bal + un_pnl;
            }
        }

        i += 1;
    }

    // Is the account duplicate?
    let mut seen = HashSet::new();
    ensure_zk!(
        !uids.iter().any(|x| !seen.insert(x)),
        zkerr!(ZkErrorCode::DuplicateAccount)
    );

    Ok(())
}

fn app_binance(
    pv: &mut PublicValuesStruct,
    unified_data: String,
    spot_data: String,
    feature_data: String,
) -> Result<(), ZktlsError> {
    // Verify Unified, Spot and Feature
    let mut asset_bals: HashMap<String, f64> = HashMap::new();

    let mut unified_am = AttestationMetaStruct::default();
    app_binance_unified(&mut unified_am, &unified_data, &mut asset_bals)?;
    pv.attestation_meta.push(unified_am);

    let mut spot_am = AttestationMetaStruct::default();
    app_binance_spot(&mut spot_am, &spot_data, &mut asset_bals)?;
    pv.attestation_meta.push(spot_am);

    let mut feature_am = AttestationMetaStruct::default();
    app_binance_feature(&mut feature_am, &feature_data, &mut asset_bals)?;
    pv.attestation_meta.push(feature_am);

    // Summary assets by Category
    let mut stablecoin_sum = 0.0;
    for (k, v) in asset_bals {
        if STABLE_COINS.contains(&k.as_str()) {
            stablecoin_sum += v;
        } else {
            if v > EPSILON_VALUE {
                pv.asset_balance.insert(k, v);
            }
        }
    }
    if stablecoin_sum > EPSILON_VALUE {
        pv.asset_balance.insert("STABLECOIN".to_string(), stablecoin_sum);
    }

    Ok(())
}

fn app_aster(pv: &mut PublicValuesStruct, spot_data: String, feature_data: String) -> Result<(), ZktlsError> {
    // Verify Spot and Feature
    let mut asset_bals: HashMap<String, f64> = HashMap::new();

    let mut spot_am = AttestationMetaStruct::default();
    app_aster_spot(&mut spot_am, &spot_data, &mut asset_bals)?;
    pv.attestation_meta.push(spot_am);

    let mut feature_am = AttestationMetaStruct::default();
    app_aster_feature(&mut feature_am, &feature_data, &mut asset_bals)?;
    pv.attestation_meta.push(feature_am);

    // Summary assets by Category
    let mut stablecoin_sum = 0.0;
    for (k, v) in asset_bals {
        if STABLE_COINS.contains(&k.as_str()) {
            stablecoin_sum += v;
        } else {
            if v > EPSILON_VALUE {
                pv.asset_balance.insert(k, v);
            }
        }
    }
    if stablecoin_sum > EPSILON_VALUE {
        pv.asset_balance.insert("STABLECOIN".to_string(), stablecoin_sum);
    }

    Ok(())
}

fn app_main(pv: &mut PublicValuesStruct) -> Result<(), ZktlsError> {
    let attestation_data: String = sp1_zkvm::io::read();

    let v: serde_json::Value = serde_json::from_str(&attestation_data)
        .map_err(|e| zkerr!(ZkErrorCode::ParseAttestationData, e.to_string()))?;
    let unified_data = v.get("unified").map(|a| a.to_string());
    let spot_data = v.get("spot").map(|a| a.to_string());
    let feature_data = v.get("feature").map(|a| a.to_string());
    let aster_spot_data = v.get("asterSpot").map(|a| a.to_string());
    let aster_feature_data = v.get("asterFeature").map(|a| a.to_string());

    let mut has_data = false;
    if let (Some(unified), Some(spot), Some(feature)) = (unified_data, spot_data, feature_data) {
        has_data |= true;
        app_binance(pv, unified, spot, feature)?;
    }
    if let (Some(spot), Some(feature)) = (aster_spot_data, aster_feature_data) {
        has_data |= true;
        app_aster(pv, spot, feature)?;
    }
    ensure_zk!(has_data, zkerr!(ZkErrorCode::MissingRequiredData));

    Ok(())
}

pub fn main() {
    let mut pv = PublicValuesStruct::default();
    if let Err(e) = app_main(&mut pv) {
        println!("Error: {} {}", e.icode(), e.msg());
        pv.status = e.icode();
    } else {
        println!("OK");
    }
    commit(&pv);
}
