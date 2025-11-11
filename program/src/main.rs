#![no_main]

mod binance;
mod errors;
mod phala;

sp1_zkvm::entrypoint!(main);

// use std::time::{SystemTime};
use crate::{
    binance::{ApiResponse, PurchaseRecord, RedeemRecord},
    errors::{ZkErrorCode, ZktlsError},
    phala::{UserInfo, VmStatusMap},
};
use anyhow::{Context, Result};
use sp1_zkvm::io::{commit, read};
use serde_json::json;
use zktls_att_verification::attestation_data::{AttestationData, verify_attestation_data};
use crate::binance::PositionInfo;

fn app_main() -> Result<()> {
    // let now_ts = SystemTime::now();
    let attestation_data: String = read();

    // println!("attestation_data:{}", attestation_data);

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
        "url": ["https://cloud.phala.network/api/status/batch",
            "https://cloud-api.phala.network/api/v1/auth/me?",
            "https://www.binance.com/bapi/earn/v2/private/lending/union/purchaseRecord/list",
            "https://www.binance.com/bapi/earn/v1/private/lending/union/redemption/list?",
            "https://www.binance.com/bapi/earn/v2/private/lending/daily/token/position?"
        ]
    });
    // 1. Verify
    let (attestation_data, _, _messages) =
        verify_attestation_data(&attestation_data, &attestion_confg.to_string())?;
    println!("verify success");
    let source = extra_data_source(&attestation_data);
    println!("source is {source}");
    if source.eq("phala") {
        return handle_phala(&attestation_data);
    } else if source.eq("binance") {
        return handle_binance(&attestation_data);
    } else {
        ensure_zk!(true, zkerr!(ZkErrorCode::NotSupportSource));
    }
    Ok(())
}

// Extra source from attestation
fn extra_data_source(attestation_data: &AttestationData) -> &'static str {
    if let Some(request) = attestation_data
        .public_data
        .get(0)
        .and_then(|pd| pd.attestation.request.get(0))
    {
        let url = &request.url;

        if url.contains("cloud-api.phala.network") {
            "phala"
        } else if url.contains("www.binance.com") {
            "binance"
        } else {
            "unknown"
        }
    } else {
        "unknown"
    }
}

fn handle_binance(attestation_data: &AttestationData) -> Result<()> {
    let first_public = attestation_data
        .public_data
        .get(0)
        .context("public_data is empty")?;
    let now_ts = first_public.attestationTime;
    let (
        today_start,
        yesterday_start,
        yesterday_end,
        day_before_yesterday_start,
        day_before_yesterday_end,
    ) = get_day_ranges(now_ts);

    let mut today_asset: f64 = 0.0;

    let mut today_buy_amount: f64 = 0.0;
    let mut today_sell_amount: f64 = 0.0;

    let mut yesterday_buy_amount: f64 = 0.0;
    let mut yesterday_sell_amount: f64 = 0.0;

    let mut day_before_yesterday_buy_amount = 0.0;
    let mut day_before_yesterday_sell_amount: f64 = 0.0;

    //
    let default_token = "PHA";
    let mut user_id = String::new();
    if let Some(responses) = attestation_data.private_data.plain_json_response.as_ref() {
        for response in responses {
            if response.id.eq("subscriptionList") {
                let subscription_rsp: ApiResponse<Vec<PurchaseRecord>> =
                    serde_json::from_str(response.content.as_str())?;
                for sub in subscription_rsp.data {
                    if !default_token.eq(&sub.asset) {
                        continue;
                    }
                    // check time
                    let timestamp_str = sub.create_timestamp;
                    let timestamp: u64 = timestamp_str.parse::<u64>()?;
                    let buy_amount = sub.amount.parse::<f64>()?;
                    if timestamp >= today_start && timestamp < now_ts {
                        today_buy_amount += buy_amount
                    }
                    if timestamp >= yesterday_start && timestamp < yesterday_end {
                        yesterday_buy_amount += buy_amount
                    }
                    if timestamp >= day_before_yesterday_start
                        && timestamp < day_before_yesterday_end
                    {
                        day_before_yesterday_buy_amount += buy_amount
                    }
                }
                println!("{},today_buy_amount: {}", default_token, today_buy_amount);
                println!(
                    "{},yesterday_buy_amount:{}",
                    default_token, yesterday_buy_amount
                );
                println!(
                    "{},day_before_yesterday_buy_amount:{}",
                    default_token, day_before_yesterday_buy_amount
                );
            }
            if response.id.eq("redemptionList") {
                let redeem_rsp: ApiResponse<Vec<RedeemRecord>> =
                    serde_json::from_str(response.content.as_str())?;
                for red in redeem_rsp.data {
                    if !default_token.eq(&red.asset) {
                        continue;
                    }
                    // check time
                    let timestamp_str = red.create_timestamp;
                    let timestamp: u64 = timestamp_str.parse::<u64>()?;
                    let sell_amount = red.amount.parse::<f64>()?;
                    if timestamp >= today_start && timestamp < now_ts {
                        today_sell_amount += sell_amount
                    }
                    if timestamp >= yesterday_start && timestamp < yesterday_end {
                        yesterday_sell_amount += sell_amount
                    }
                    if timestamp >= day_before_yesterday_start
                        && timestamp < day_before_yesterday_end
                    {
                        day_before_yesterday_sell_amount += sell_amount
                    }
                }
                println!("{},today_sell_amount: {}", default_token, today_sell_amount);
                println!(
                    "{},yesterday_sell_amount:{}",
                    default_token, yesterday_sell_amount
                );
                println!(
                    "{},day_before_yesterday_sell_amount:{}",
                    default_token, day_before_yesterday_sell_amount
                );
            }
            if response.id.eq("assetDetails") {
                let asset_data_rsp: ApiResponse<Vec<PositionInfo>> =
                    serde_json::from_str(response.content.as_str())?;

                for asd in asset_data_rsp.data{
                    if user_id.is_empty() {
                        user_id = asd.user_id.clone()
                    }
                    if default_token.eq(&asd.asset) {
                        today_asset = asd.free_amount.parse::<f64>()?;
                        println!("{},{}", default_token, today_asset);
                        break;
                    }
                }
            }
        }
    }
    // compute average amount of the past 3 days
    let yesterday_end_amount = today_asset - today_buy_amount + today_sell_amount;
    let day_before_yesterday_end_amount =
        yesterday_end_amount - yesterday_buy_amount + yesterday_sell_amount;
    let two_day_before_yesterday_end_amount = day_before_yesterday_end_amount
        - day_before_yesterday_buy_amount
        + day_before_yesterday_sell_amount;
    let average_past_3_days = (yesterday_end_amount
        + day_before_yesterday_end_amount
        + two_day_before_yesterday_end_amount)
        / 3.0;
    println!("user_id = {}", user_id);
    println!(
        "{} average amount in the past 3 days:{}",
        default_token, average_past_3_days
    );
    commit(&user_id);
    commit(&average_past_3_days);

    Ok(())
}

fn handle_phala(attestation_data: &AttestationData) -> Result<()> {
    if let Some(responses) = attestation_data.private_data.plain_json_response.as_ref() {
        let mut up_time_enough = false;

        for response in responses {
            let id = response.id.as_str();
            let content = response.content.as_str();
            if "userInfo".eq(id) {
                let user_info: UserInfo = serde_json::from_str(content)?;
                println!("userInfo: {:?}", user_info);
                commit(&user_info.email);
            } else {
                let vms: VmStatusMap = serde_json::from_str(content)?;
                up_time_enough = check_all_vm_uptime(&vms);
            }
        }

        ensure_zk!(up_time_enough, zkerr!(ZkErrorCode::UpTimeNotEnough));
    } else {
        ensure_zk!(true, zkerr!(ZkErrorCode::EmptyPlainResponse));
    }
    Ok(())
}

fn check_all_vm_uptime(vms: &VmStatusMap) -> bool {
    let mut total_seconds: u64 = 0;

    for (_uuid, vm_status) in vms {
        let uptime = &vm_status.uptime;
        // if time unit is hour and others, uptime meets the requirement
        if uptime.contains("day")
            || uptime.contains("days")
            || uptime.contains("hour")
            || uptime.contains("h")
            || uptime.contains("hours")
            || uptime.contains("month")
            || uptime.contains("months")
            || uptime.contains("year")
            || uptime.contains("years")
        {
            return true;
        }
        // Compute total time of cvms
        total_seconds += parse_minutes_seconds(uptime);
    }
    println!("VM uptime: {}", total_seconds);
    total_seconds >= 10 * 60
}

fn parse_minutes_seconds(uptime: &str) -> u64 {
    let mut seconds = 0u64;

    for part in uptime.split_whitespace() {
        if part.ends_with('m') {
            if let Ok(n) = part.trim_end_matches('m').parse::<u64>() {
                seconds += n * 60;
            }
        } else if part.ends_with('s') {
            if let Ok(n) = part.trim_end_matches('s').parse::<u64>() {
                seconds += n;
            }
        }
    }

    seconds
}

fn get_day_ranges(now_ts: u64) -> (u64, u64, u64, u64, u64) {
    const SECS_PER_DAY: u64 = 24 * 60 * 60 * 1000;

    // today start
    let today_start = now_ts - (now_ts % SECS_PER_DAY);

    // yesterday start and end
    let yesterday_start = today_start - SECS_PER_DAY;
    let yesterday_end = today_start - 1;

    // the day before yesterday start and end
    let day_before_start = today_start - 2 * SECS_PER_DAY;
    let day_before_end = yesterday_start - 1;

    (
        today_start,
        yesterday_start,
        yesterday_end,
        day_before_start,
        day_before_end,
    )
}
pub fn main() {
    if let Err(e) = app_main() {
        println!("Error: {:?}", e);
        // panic or not?
        panic!("error {:?}", e);
    }
}
