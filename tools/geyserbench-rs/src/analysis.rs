use crate::utils::{format_ping_median, percentile, Comparator, TransactionData};
use comfy_table::{ContentArrangement, Table};
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

const COLOR_WHITE: &str = "\x1b[1;37m";
const COLOR_BLUE: &str = "\x1b[38;5;39m";
const COLOR_GREEN: &str = "\x1b[38;5;34m";
const COLOR_RESET: &str = "\x1b[0m";

#[cfg(target_os = "windows")]
#[inline]
fn table_preset() -> &'static str {
    comfy_table::presets::ASCII_FULL
}

#[cfg(not(target_os = "windows"))]
#[inline]
fn table_preset() -> &'static str {
    comfy_table::presets::UTF8_FULL
}
use std::time::Duration;

#[derive(Default)]
pub struct EndpointStats {
    pub total_observations: usize,
    pub first_detections: usize,
    pub delays_ms: Vec<f64>,
    pub backfill_transactions: usize,
}

#[derive(Debug, Default, Clone)]
pub struct EndpointSummary {
    pub name: String,
    pub first_share: f64,
    pub p50_delay_ms: Option<f64>,
    pub p95_delay_ms: Option<f64>,
    pub p99_delay_ms: Option<f64>,
    pub valid_transactions: usize,
    pub first_detections: usize,
    pub backfill_transactions: usize,
}

#[derive(Debug, Clone)]
pub struct RunSummary {
    pub endpoints: Vec<EndpointSummary>,
    pub fastest_endpoint: Option<String>,
    pub has_data: bool,
    pub total_signatures: usize,
    pub backfill_signatures: usize,
    /// Region filter applied, if any.
    pub region_filter: Option<String>,
    /// Total signatures before region filter.
    pub total_before_filter: usize,
    /// Signatures that matched the region.
    pub region_matched: usize,
}

/// Collect all unique slots from the comparator.
pub fn collect_slots(comparator: &Comparator) -> Vec<u64> {
    let mut slots = HashSet::new();
    for entry in comparator.iter() {
        for tx_data in entry.value().values() {
            if let Some(slot) = tx_data.slot {
                slots.insert(slot);
            }
        }
    }
    let mut sorted: Vec<u64> = slots.into_iter().collect();
    sorted.sort_unstable();
    sorted
}

pub fn compute_run_summary(
    comparator: &Comparator,
    endpoint_names: &[String],
    region_slots: Option<&HashSet<u64>>,
    region_name: Option<&str>,
) -> RunSummary {
    let mut endpoint_stats: HashMap<String, EndpointStats> = HashMap::new();
    let expected_producers = endpoint_names.len();
    let mut total_signatures = 0usize;
    let mut backfill_signatures = 0usize;
    let mut total_before_filter = 0usize;
    let mut region_matched = 0usize;

    for endpoint_name in endpoint_names {
        endpoint_stats.insert(endpoint_name.clone(), EndpointStats::default());
    }

    for sig_entry in comparator.iter() {
        let sig_data = sig_entry.value();
        if expected_producers > 0 && sig_data.len() != expected_producers {
            continue;
        }

        let is_historical = sig_data
            .values()
            .any(|tx| tx.wallclock_secs < tx.start_wallclock_secs);

        if is_historical {
            backfill_signatures += 1;
            for endpoint in sig_data.keys() {
                if let Some(stats) = endpoint_stats.get_mut(endpoint) {
                    stats.backfill_transactions += 1;
                }
            }
            continue;
        }

        total_before_filter += 1;

        // Region filter: skip if this signature's slot is not in the target region
        if let Some(allowed_slots) = region_slots {
            let sig_slot = sig_data.values().find_map(|tx| tx.slot);
            match sig_slot {
                Some(slot) if allowed_slots.contains(&slot) => {
                    region_matched += 1;
                }
                Some(_) => continue, // Slot not in target region, skip
                None => continue,    // No slot info, skip
            }
        }

        let Some((first_endpoint, first_tx)) =
            sig_data.iter().min_by_key(|(_, tx)| tx.elapsed_since_start)
        else {
            continue;
        };

        total_signatures += 1;
        let first_endpoint_name = first_endpoint.clone();

        for (endpoint, tx) in sig_data.iter() {
            if let Some(stats) = endpoint_stats.get_mut(endpoint) {
                stats.total_observations += 1;
                if endpoint == &first_endpoint_name {
                    stats.first_detections += 1;
                    stats.delays_ms.push(0.0);
                } else {
                    let delay_ms = diff_ms(tx, first_tx).max(0.0);
                    stats.delays_ms.push(delay_ms);
                }
            }
        }
    }

    let endpoints: Vec<EndpointSummary> = endpoint_stats
        .into_iter()
        .map(|(endpoint, stats)| build_summary(endpoint, stats, total_signatures))
        .collect();

    let has_data = total_signatures > 0;

    let fastest_endpoint = endpoints
        .iter()
        .filter(|summary| summary.valid_transactions > 0)
        .min_by(|a, b| compare_latency(a, b))
        .map(|summary| summary.name.clone());

    RunSummary {
        endpoints,
        fastest_endpoint,
        has_data,
        total_signatures,
        backfill_signatures,
        region_filter: region_name.map(|s| s.to_string()),
        total_before_filter,
        region_matched: if region_slots.is_some() {
            region_matched
        } else {
            total_before_filter
        },
    }
}

pub fn display_run_summary(
    summary: &RunSummary,
    ping_results: Option<&HashMap<String, Vec<f64>>>,
) {
    // Region filter header
    if let Some(ref region) = summary.region_filter {
        println!(
            "\n{}🌍 Region filter: {} — {}/{} transactions matched{}",
            COLOR_GREEN,
            region,
            summary.region_matched,
            summary.total_before_filter,
            COLOR_RESET,
        );
    }

    println!("\n{}Finished test results{}", COLOR_WHITE, COLOR_RESET);
    println!("--------------------------------------------");

    if !summary.has_data {
        println!("Not enough data");
    } else {
        let fastest_name_ref = summary.fastest_endpoint.as_deref();
        let mut summary_rows: Vec<&EndpointSummary> = summary.endpoints.iter().collect();
        summary_rows.sort_by(|a, b| compare_latency(a, b));

        let mut rank = 0usize;
        let rank_emojis = ["🥇", "🥈", "🥉"];
        for summary in summary_rows {
            if summary.valid_transactions == 0 {
                println!("{}: Not enough data", summary.name);
                continue;
            }
            rank += 1;
            let rank_prefix = if rank <= rank_emojis.len() {
                format!("{} ", rank_emojis[rank - 1])
            } else {
                format!("{}th ", rank)
            };

            let raw_win_rate = format_percent(summary.first_share);
            let win_rate = if raw_win_rate == "—" {
                raw_win_rate
            } else {
                format!("{}%", raw_win_rate)
            };
            let is_fastest = fastest_name_ref == Some(summary.name.as_str());

            if is_fastest {
                println!(
                    "{}{}: Win rate {}, p50 0.00ms (fastest)",
                    rank_prefix, summary.name, win_rate,
                );
            } else {
                let p50_delay = summary
                    .p50_delay_ms
                    .map(|v| format!("{:.2}ms", v))
                    .unwrap_or_else(|| "—".to_string());
                println!(
                    "{}{}: Win rate {}, p50 {}",
                    rank_prefix, summary.name, win_rate, p50_delay
                );
            }
        }
    }

    println!("\n{}Detailed test results{}", COLOR_WHITE, COLOR_RESET);
    println!("--------------------------------------------");

    if !summary.has_data {
        println!("Not enough data");
        return;
    }

    let mut table_rows: Vec<&EndpointSummary> = summary.endpoints.iter().collect();
    table_rows.sort_by(|a, b| compare_latency(a, b));

    let mut table = Table::new();
    table.load_preset(table_preset());
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec![
        "Endpoint", "First %", "P50 ms", "P95 ms", "P99 ms", "Ping ms", "Valid Tx", "Firsts",
        "Backfill",
    ]);

    let fastest_name_ref = summary.fastest_endpoint.as_deref();
    for summary in table_rows {
        let is_fastest = fastest_name_ref == Some(summary.name.as_str());
        let ping_value = ping_results
            .and_then(|results| results.get(&summary.name))
            .map(|values| format_ping_median(values, false))
            .unwrap_or_else(|| "—".to_string());
        table.add_row(vec![
            summary.name.clone(),
            format_percent(summary.first_share),
            format_latency_value(summary.p50_delay_ms, is_fastest),
            format_latency_value(summary.p95_delay_ms, is_fastest),
            format_latency_value(summary.p99_delay_ms, is_fastest),
            ping_value,
            summary.valid_transactions.to_string(),
            summary.first_detections.to_string(),
            summary.backfill_transactions.to_string(),
        ]);
    }

    println!("{}{}{}", COLOR_BLUE, table, COLOR_RESET);
}

pub fn build_metrics_report(summary: &RunSummary) -> Value {
    let mut per_endpoint = Map::new();
    for endpoint in &summary.endpoints {
        let payload = json!({
            "first_detection_rate": endpoint.first_share,
            "p50_latency_ms": endpoint.p50_delay_ms,
            "p95_latency_ms": endpoint.p95_delay_ms,
            "p99_latency_ms": endpoint.p99_delay_ms,
            "observations": endpoint.valid_transactions,
            "first_detections": endpoint.first_detections,
            "backfill_transactions": endpoint.backfill_transactions,
        });
        per_endpoint.insert(endpoint.name.clone(), payload);
    }

    json!({
        "total_signatures": summary.total_signatures,
        "backfill_signatures": summary.backfill_signatures,
        "region_filter": summary.region_filter,
        "total_before_filter": summary.total_before_filter,
        "region_matched": summary.region_matched,
        "per_endpoint": per_endpoint
    })
}

fn diff_ms(tx: &TransactionData, first_tx: &TransactionData) -> f64 {
    let delta: Duration = tx
        .elapsed_since_start
        .saturating_sub(first_tx.elapsed_since_start);
    delta.as_secs_f64() * 1_000.0
}

fn build_summary(
    endpoint: String,
    stats: EndpointStats,
    total_signatures: usize,
) -> EndpointSummary {
    let mut summary = EndpointSummary {
        name: endpoint,
        valid_transactions: stats.total_observations,
        first_detections: stats.first_detections,
        backfill_transactions: stats.backfill_transactions,
        ..Default::default()
    };

    if total_signatures > 0 {
        summary.first_share = stats.first_detections as f64 / total_signatures as f64;
    }

    if !stats.delays_ms.is_empty() {
        let mut sorted = stats.delays_ms.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        summary.p50_delay_ms = Some(percentile(&sorted, 0.5));
        summary.p95_delay_ms = Some(percentile(&sorted, 0.95));
        summary.p99_delay_ms = Some(percentile(&sorted, 0.99));
    }

    summary
}

fn format_latency_value(value: Option<f64>, is_fastest: bool) -> String {
    if is_fastest {
        "-".to_string()
    } else {
        value
            .map(|v| format!("{:.2}", v))
            .unwrap_or_else(|| "—".to_string())
    }
}

fn compare_latency(lhs: &EndpointSummary, rhs: &EndpointSummary) -> Ordering {
    match (lhs.p50_delay_ms, rhs.p50_delay_ms) {
        (Some(l), Some(r)) => l
            .partial_cmp(&r)
            .unwrap_or(Ordering::Equal)
            .then_with(|| lhs.name.cmp(&rhs.name)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => lhs.name.cmp(&rhs.name),
    }
}

fn format_percent(value: f64) -> String {
    if value.is_finite() {
        format!("{:.2}", value * 100.0)
    } else {
        "—".to_string()
    }
}
