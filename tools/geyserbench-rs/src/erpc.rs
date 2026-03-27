//! ERPC getLeaderSlots API client for region-aware benchmarking.
//!
//! Calls the ERPC JSON-RPC endpoint to resolve slot → leader region mappings.

use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    result: Option<GetLeaderSlotsResult>,
    error: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GetLeaderSlotsResult {
    success: bool,
    data: Vec<LeaderSlotEntry>,
    total: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct LeaderSlotEntry {
    pub identity: String,
    pub epoch: u64,
    pub slot: String,
    pub ip_address: Option<String>,
    pub leader_region: Option<String>,
    pub leader_city: Option<String>,
    pub leader_country: Option<String>,
    pub leader_lat: Option<f64>,
    pub leader_lon: Option<f64>,
    pub leader_org: Option<String>,
    pub leader_timezone: Option<String>,
}

/// Slot → region mapping.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SlotRegionInfo {
    pub region: String,
    pub city: Option<String>,
    pub country: Option<String>,
    pub identity: String,
}

/// Fetch leader slot → region mappings from ERPC for the given slots.
///
/// Groups slots into batches starting from the minimum slot and fetches
/// leader schedule data. Returns a map of slot → SlotRegionInfo.
pub async fn fetch_slot_regions(
    erpc_url: &str,
    api_key: &str,
    slots: &[u64],
) -> Result<HashMap<u64, SlotRegionInfo>> {
    if slots.is_empty() {
        return Ok(HashMap::new());
    }

    let client = Client::new();
    let mut result_map: HashMap<u64, SlotRegionInfo> = HashMap::new();

    // Deduplicate and sort slots
    let mut unique_slots: Vec<u64> = slots.to_vec();
    unique_slots.sort_unstable();
    unique_slots.dedup();

    // The API returns 100 entries starting from the given slot.
    // We need to batch requests to cover all unique slots.
    let mut covered_up_to: u64 = 0;
    let mut request_id: u64 = 1;

    for &slot in &unique_slots {
        if slot < covered_up_to {
            continue; // Already covered by a previous batch
        }

        let url = format!("{}?api-key={}", erpc_url, api_key);
        let body = JsonRpcRequest {
            jsonrpc: "2.0",
            id: request_id,
            method: "getLeaderSlots",
            params: vec![serde_json::Value::Number(slot.into())],
        };
        request_id += 1;

        debug!(slot, "Fetching leader slots from ERPC");

        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to call ERPC getLeaderSlots")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            warn!(status = %status, "ERPC API returned error: {}", text);
            continue;
        }

        let rpc_resp: JsonRpcResponse = resp
            .json()
            .await
            .context("Failed to parse ERPC response")?;

        if let Some(err) = rpc_resp.error {
            warn!("ERPC returned error: {:?}", err);
            continue;
        }

        let data = rpc_resp
            .result
            .ok_or_else(|| anyhow!("ERPC response missing result"))?;

        for entry in &data.data {
            let entry_slot: u64 = entry.slot.parse().unwrap_or(0);
            if entry_slot == 0 {
                continue;
            }

            if let Some(ref region) = entry.leader_region {
                result_map.insert(
                    entry_slot,
                    SlotRegionInfo {
                        region: region.clone(),
                        city: entry.leader_city.clone(),
                        country: entry.leader_country.clone(),
                        identity: entry.identity.clone(),
                    },
                );
            }

            if entry_slot >= covered_up_to {
                covered_up_to = entry_slot + 1;
            }
        }
    }

    info!(
        slots_requested = unique_slots.len(),
        slots_resolved = result_map.len(),
        "Fetched leader region data from ERPC"
    );

    Ok(result_map)
}

/// Filter a slot-region map to only include entries matching the target region.
pub fn filter_slots_by_region(
    slot_regions: &HashMap<u64, SlotRegionInfo>,
    target_region: &str,
) -> Vec<u64> {
    let target = target_region.to_lowercase();
    slot_regions
        .iter()
        .filter(|(_, info)| info.region.to_lowercase() == target)
        .map(|(&slot, _)| slot)
        .collect()
}
