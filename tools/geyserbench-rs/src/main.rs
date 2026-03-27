pub use {
    bs58,
    bytes::Bytes,
    futures_util::stream::StreamExt,
    serde::{Deserialize, Serialize},
    std::{
        env,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        time::Instant,
    },
    tokio::{signal::ctrl_c, sync::broadcast, task},
};

mod analysis;
mod config;
mod erpc;
mod proto;
mod providers;
mod utils;

use std::collections::{HashMap, HashSet};
use std::process::Command;

use anyhow::{Result, anyhow};
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;
use utils::{Comparator, ProgressTracker, format_ping_median, get_current_timestamp};
const DEFAULT_CONFIG_PATH: &str = "config.toml";
const DEFAULT_PING_COUNT: usize = 3;
const FINAL_LOG_MESSAGE: &str = concat!(
    "\x1b[1;37m🚀⚡ Get an upgrade on ERPC Global for even more speed — ",
    "powerful servers and streaming connections\n\n",
    "🔗 https://erpc.global\x1b[0m",
);

struct CliArgs {
    config_path: Option<String>,
    region: Option<String>,
    list_regions: bool,
}

impl CliArgs {
    fn parse() -> Self {
        let mut args = env::args().skip(1);
        let mut parsed = CliArgs {
            config_path: None,
            region: None,
            list_regions: false,
        };

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--config" => {
                    let value = args.next().unwrap_or_else(|| {
                        eprintln!("Missing value for --config");
                        print_usage();
                        std::process::exit(1);
                    });
                    parsed.config_path = Some(value);
                }
                "--region" => {
                    let value = args.next().unwrap_or_else(|| {
                        eprintln!("Missing value for --region");
                        print_usage();
                        std::process::exit(1);
                    });
                    parsed.region = Some(value);
                }
                "--list-regions" => {
                    parsed.list_regions = true;
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                other => {
                    eprintln!("Unknown argument: {}", other);
                    print_usage();
                    std::process::exit(1);
                }
            }
        }

        parsed
    }
}

fn print_usage() {
    eprintln!(
        "Usage: geyserbench [OPTIONS]\n\n\
         Options:\n\
         \x20 --config <PATH>     Load configuration from a different TOML file (default: config.toml)\n\
         \x20 --region <REGION>   Filter results by leader region (e.g. frankfurt, tokyo, ny)\n\
         \x20 --list-regions      Show available ERPC regions and exit\n\
         \x20 -h, --help          Show this help message"
    );
}

fn ping_count_from_env() -> usize {
    match env::var("PING_NUM") {
        Ok(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                warn!("PING_NUM is empty; using default ping count");
                DEFAULT_PING_COUNT
            } else if let Ok(value) = trimmed.parse::<usize>() {
                if value == 0 {
                    warn!("PING_NUM must be >= 1; using default ping count");
                    DEFAULT_PING_COUNT
                } else {
                    value
                }
            } else {
                warn!(ping_num = %trimmed, "Invalid PING_NUM; using default ping count");
                DEFAULT_PING_COUNT
            }
        }
        Err(env::VarError::NotPresent) => DEFAULT_PING_COUNT,
        Err(err) => {
            warn!(error = %err, "Failed to read PING_NUM; using default ping count");
            DEFAULT_PING_COUNT
        }
    }
}

fn extract_host(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_scheme = match trimmed.split_once("://") {
        Some((_, rest)) => rest,
        None => trimmed,
    };
    let host_port = without_scheme.split('/').next().unwrap_or(without_scheme);
    if host_port.is_empty() {
        return None;
    }

    if let Some(stripped) = host_port.strip_prefix('[') {
        let end = stripped.find(']')?;
        return Some(stripped[..end].to_string());
    }

    Some(
        host_port
            .split_once(':')
            .map(|(host, _)| host)
            .unwrap_or(host_port)
            .to_string(),
    )
}

fn parse_ping_time(line: &str) -> Option<f64> {
    let offset = if let Some(pos) = line.find("time=") {
        pos + 5
    } else if let Some(pos) = line.find("time<") {
        pos + 5
    } else {
        return None;
    };

    let mut value = String::new();
    for ch in line[offset..].chars() {
        if ch.is_ascii_digit() || ch == '.' {
            value.push(ch);
        } else if !value.is_empty() {
            break;
        }
    }

    if value.is_empty() {
        None
    } else {
        value.parse::<f64>().ok()
    }
}

fn parse_ping_times(output: &str) -> Vec<f64> {
    output
        .lines()
        .filter_map(parse_ping_time)
        .collect::<Vec<_>>()
}

fn ping_host(host: &str, count: usize) -> Option<Vec<f64>> {
    let mut command = Command::new("ping");
    if cfg!(target_os = "windows") {
        command.arg("-n").arg(count.to_string());
    } else {
        command.arg("-c").arg(count.to_string());
    }
    command.arg(host);

    let output = command.output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let mut times = parse_ping_times(&stdout);
    if times.is_empty() {
        times = parse_ping_times(&stderr);
    }
    if times.is_empty() {
        None
    } else {
        Some(times)
    }
}

fn run_ping_checks(endpoints: &[config::Endpoint], count: usize) -> HashMap<String, Vec<f64>> {
    println!("\nPing results ({}x)", count);
    println!("--------------------------------------------");

    let mut results = HashMap::new();
    for endpoint in endpoints {
        let host = match extract_host(&endpoint.url) {
            Some(host) => host,
            None => {
                println!("{}: host not found", endpoint.name);
                continue;
            }
        };

        let times = ping_host(&host, count);
        match times {
            Some(values) => {
                println!(
                    "{} ({}) {}",
                    endpoint.name,
                    host,
                    format_ping_median(&values, true)
                );
                results.insert(endpoint.name.clone(), values);
            }
            None => {
                warn!(endpoint = %endpoint.name, host = %host, "Ping failed");
                println!("{} ({}) ping failed", endpoint.name, host);
            }
        }
    }

    results
}

#[tokio::main]
async fn main() -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .compact()
        .try_init()
        .map_err(|err| anyhow!(err))?;

    let cli = CliArgs::parse();

    // --list-regions
    if cli.list_regions {
        println!("Available ERPC regions:");
        for region in config::KNOWN_REGIONS {
            println!("  {}", region);
        }
        return Ok(());
    }

    let config_path = cli.config_path.as_deref().unwrap_or(DEFAULT_CONFIG_PATH);
    let config = config::ConfigToml::load_or_create(config_path)?;
    info!(config_path = config_path, "Loaded configuration");

    // Determine the target region (CLI flag overrides config file)
    let target_region = cli
        .region
        .as_deref()
        .or(config.config.region.as_deref())
        .map(|r| r.to_lowercase());

    if let Some(ref region) = target_region {
        info!(region = %region, "Region filter enabled — results will be filtered post-analysis");

        // Validate ERPC credentials
        if config.config.erpc_url.is_none() || config.config.erpc_api_key.is_none() {
            eprintln!(
                "Error: --region requires erpc_url and erpc_api_key in config.toml\n\
                 Add:\n  erpc_url = \"https://edge.erpc.global\"\n  erpc_api_key = \"your-key\""
            );
            std::process::exit(1);
        }
    }

    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    let start_time_local = get_current_timestamp();
    let comparator = Arc::new(Comparator::new());
    let start_instant = Instant::now();
    let shared_counter = Arc::new(AtomicUsize::new(0));
    let shared_shutdown = Arc::new(AtomicBool::new(false));
    let aborted = Arc::new(AtomicBool::new(false));

    let ping_count = ping_count_from_env();
    let ping_results = run_ping_checks(&config.endpoint, ping_count);

    let mut handles = Vec::new();
    let endpoint_names: Vec<String> = config.endpoint.iter().map(|e| e.name.clone()).collect();
    let global_target = if config.config.transactions > 0 {
        Some(config.config.transactions as usize)
    } else {
        None
    };
    let progress_tracker = global_target.map(|target| Arc::new(ProgressTracker::new(target)));

    let total_producers = config.endpoint.len();
    for endpoint in config.endpoint.clone().into_iter() {
        let provider = providers::create_provider(&endpoint.kind);
        let shared_config = config.config.clone();
        let context = providers::ProviderContext {
            shutdown_tx: shutdown_tx.clone(),
            shutdown_rx: shutdown_tx.subscribe(),
            start_wallclock_secs: start_time_local,
            start_instant,
            comparator: comparator.clone(),
            shared_counter: shared_counter.clone(),
            shared_shutdown: shared_shutdown.clone(),
            target_transactions: global_target,
            total_producers,
            progress: progress_tracker.clone(),
        };

        handles.push(provider.process(endpoint, shared_config, context));
    }

    tokio::spawn({
        let shutdown_tx = shutdown_tx.clone();
        let shared_shutdown = shared_shutdown.clone();
        let aborted = aborted.clone();
        async move {
            match ctrl_c().await {
                Ok(()) => {
                    let already_aborting = aborted.swap(true, Ordering::AcqRel);
                    if already_aborting {
                        info!("Received additional Ctrl+C; shutdown already in progress");
                    } else {
                        info!("Received Ctrl+C; initiating shutdown");
                    }
                    shared_shutdown.store(true, Ordering::Release);
                    let _ = shutdown_tx.send(());
                }
                Err(err) => error!(error = %err, "Failed to listen for Ctrl+C"),
            }
        }
    });

    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => error!(error = ?e, "Provider task returned error"),
            Err(e) => error!(error = ?e, "Provider join error"),
        }
    }

    let run_aborted = aborted.load(Ordering::Acquire);

    if run_aborted {
        info!("Benchmark aborted before completion; no results were generated");
        return Ok(());
    }

    // ── Phase 1: Global results (all slots) ──
    let global_summary = analysis::compute_run_summary(
        comparator.as_ref(),
        &endpoint_names,
        None,
        None,
    );

    // ── Phase 2: Region-filtered results ──
    let region_summary = if let Some(ref region) = target_region {
        let erpc_url = config.config.erpc_url.as_deref().unwrap();
        let api_key = config.config.erpc_api_key.as_deref().unwrap();

        // Collect all unique slots from observations
        let observed_slots = analysis::collect_slots(comparator.as_ref());
        info!(
            total_slots = observed_slots.len(),
            region = %region,
            "Fetching leader region data for observed slots"
        );

        // Fetch leader region info from ERPC
        let slot_regions = erpc::fetch_slot_regions(erpc_url, api_key, &observed_slots).await?;

        // Filter to target region
        let matching_slots: HashSet<u64> = erpc::filter_slots_by_region(&slot_regions, region)
            .into_iter()
            .collect();

        info!(
            matching_slots = matching_slots.len(),
            total_slots = observed_slots.len(),
            region = %region,
            "Region filter applied"
        );

        let summary = analysis::compute_run_summary(
            comparator.as_ref(),
            &endpoint_names,
            Some(&matching_slots),
            Some(region),
        );

        Some(summary)
    } else {
        None
    };

    // ── Display results ──
    if target_region.is_some() {
        // Show global results first
        println!("\n\x1b[1;33m═══ All Regions (Global) ═══\x1b[0m");
        analysis::display_run_summary(&global_summary, Some(&ping_results));

        // Then region-filtered results
        if let Some(ref region_sum) = region_summary {
            println!(
                "\n\x1b[1;32m═══ Region: {} ═══\x1b[0m",
                target_region.as_deref().unwrap()
            );
            analysis::display_run_summary(region_sum, Some(&ping_results));
        }

        let metrics_json = analysis::build_metrics_report(
            region_summary.as_ref().unwrap_or(&global_summary),
        );
        debug!(metrics = %metrics_json, "Computed run metrics");
    } else {
        analysis::display_run_summary(&global_summary, Some(&ping_results));
        let metrics_json = analysis::build_metrics_report(&global_summary);
        debug!(metrics = %metrics_json, "Computed run metrics");
    }

    println!("{FINAL_LOG_MESSAGE}");

    Ok(())
}
