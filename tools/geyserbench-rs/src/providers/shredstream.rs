use futures_util::stream::StreamExt;
use std::{error::Error, sync::atomic::Ordering, time::Duration};
use tokio::task;
use tracing::{Level, error, info, warn};

use solana_pubkey::Pubkey;

use crate::{
    config::{Config, Endpoint},
    utils::{TransactionData, get_current_timestamp, open_log_file, write_log_entry},
};

use super::{
    GeyserProvider, ProviderContext,
    common::{TransactionAccumulator, record_observation},
};

#[allow(clippy::all, dead_code)]
pub mod shredstream {
    include!(concat!(env!("OUT_DIR"), "/shredstream.rs"));
}

/// Maximum number of reconnection attempts before giving up.
const MAX_RECONNECT_ATTEMPTS: u32 = 10;
/// Base delay for exponential backoff (doubles each attempt, capped at 30s).
const RECONNECT_BASE_DELAY: Duration = Duration::from_secs(1);
/// Maximum backoff delay between reconnection attempts.
const RECONNECT_MAX_DELAY: Duration = Duration::from_secs(30);
/// Timeout for receiving the next message from the stream.
const STREAM_READ_TIMEOUT: Duration = Duration::from_secs(30);

pub struct ShredstreamProvider;

impl GeyserProvider for ShredstreamProvider {
    fn process(
        &self,
        endpoint: Endpoint,
        config: Config,
        context: ProviderContext,
    ) -> task::JoinHandle<Result<(), Box<dyn Error + Send + Sync>>> {
        task::spawn(async move { process_shredstream_endpoint(endpoint, config, context).await })
    }
}

async fn connect_and_subscribe(
    endpoint_url: &str,
    _endpoint_name: &str,
) -> Result<
    tonic::Streaming<shredstream::Entry>,
    Box<dyn Error + Send + Sync>,
> {
    let client = shredstream::shredstream_proxy_client::ShredstreamProxyClient::connect(
        endpoint_url.to_string(),
    )
    .await?;
    let mut client = client;
    let request = shredstream::SubscribeEntriesRequest {};
    let stream = client.subscribe_entries(request).await?.into_inner();
    Ok(stream)
}

fn backoff_delay(attempt: u32) -> Duration {
    let delay = RECONNECT_BASE_DELAY * 2u32.saturating_pow(attempt);
    delay.min(RECONNECT_MAX_DELAY)
}

async fn process_shredstream_endpoint(
    endpoint: Endpoint,
    config: Config,
    context: ProviderContext,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let ProviderContext {
        shutdown_tx,
        mut shutdown_rx,
        start_wallclock_secs,
        start_instant,
        comparator,
        shared_counter,
        shared_shutdown,
        target_transactions,
        total_producers,
        progress,
    } = context;
    let account_pubkey = config.account.parse::<Pubkey>()?;
    let endpoint_name = endpoint.name.clone();
    let mut log_file = if tracing::enabled!(Level::TRACE) {
        Some(open_log_file(&endpoint_name)?)
    } else {
        None
    };

    let endpoint_url = endpoint.url.clone();
    let mut accumulator = TransactionAccumulator::new();
    let mut transaction_count = 0usize;
    let mut reconnect_attempts = 0u32;

    'outer: loop {
        // Check if shutdown was requested before (re)connecting
        if shared_shutdown.load(Ordering::Acquire) {
            break;
        }

        info!(endpoint = %endpoint_name, url = %endpoint_url, attempt = reconnect_attempts, "Connecting");

        let mut stream = match connect_and_subscribe(&endpoint_url, &endpoint_name).await {
            Ok(s) => {
                reconnect_attempts = 0; // reset on successful connect
                info!(endpoint = %endpoint_name, "Connected");
                s
            }
            Err(err) => {
                reconnect_attempts += 1;
                if reconnect_attempts > MAX_RECONNECT_ATTEMPTS {
                    error!(
                        endpoint = %endpoint_name,
                        attempts = MAX_RECONNECT_ATTEMPTS,
                        error = %err,
                        "Failed to connect after max attempts; giving up"
                    );
                    break;
                }
                let delay = backoff_delay(reconnect_attempts - 1);
                warn!(
                    endpoint = %endpoint_name,
                    attempt = reconnect_attempts,
                    delay_secs = delay.as_secs(),
                    error = %err,
                    "Connection failed; retrying after backoff"
                );
                tokio::select! {
                    _ = shutdown_rx.recv() => break,
                    _ = tokio::time::sleep(delay) => continue,
                }
            }
        };

        // Stream processing loop
        loop {
            tokio::select! { biased;
                _ = shutdown_rx.recv() => {
                    info!(endpoint = %endpoint_name, "Received stop signal");
                    break 'outer;
                }

                result = tokio::time::timeout(STREAM_READ_TIMEOUT, stream.next()) => {
                    match result {
                        // Timeout — no data received within STREAM_READ_TIMEOUT
                        Err(_elapsed) => {
                            warn!(
                                endpoint = %endpoint_name,
                                timeout_secs = STREAM_READ_TIMEOUT.as_secs(),
                                "Stream read timed out; reconnecting"
                            );
                            break; // break inner loop → reconnect in outer loop
                        }
                        // Stream yielded a message
                        Ok(Some(Ok(slot_entry))) => {
                            let entries = match bincode::deserialize::<Vec<solana_entry::entry::Entry>>(
                                &slot_entry.entries,
                            ) {
                                Ok(e) => e,
                                Err(e) => {
                                    error!(endpoint = %endpoint_name, error = %e, "Failed to deserialize shredstream entries");
                                    continue;
                                }
                            };
                            for entry in entries {
                                for tx in entry.transactions {
                                    let has_account = tx
                                        .message
                                        .static_account_keys()
                                        .iter()
                                        .any(|key| key == &account_pubkey);

                                    if !has_account {
                                        continue;
                                    }

                                    let wallclock = get_current_timestamp();
                                    let elapsed = start_instant.elapsed();
                                    let signature = tx.signatures[0].to_string();

                                    if let Some(file) = log_file.as_mut() {
                                        write_log_entry(file, wallclock, &endpoint_name, &signature)?;
                                    }

                                    let tx_data = TransactionData {
                                        wallclock_secs: wallclock,
                                        elapsed_since_start: elapsed,
                                        start_wallclock_secs,
                                        slot: Some(slot_entry.slot),
                                    };

                                    let updated = accumulator.record(
                                        signature.clone(),
                                        tx_data.clone(),
                                    );

                                    if updated
                                        && record_observation(
                                            &comparator,
                                            &endpoint_name,
                                            &signature,
                                            tx_data,
                                            total_producers,
                                        )
                                    {
                                        if let Some(target) = target_transactions {
                                            let shared = shared_counter
                                                .fetch_add(1, Ordering::AcqRel)
                                                + 1;
                                            if let Some(tracker) = progress.as_ref() {
                                                tracker.record(shared);
                                            }
                                            if shared >= target
                                                && !shared_shutdown.swap(true, Ordering::AcqRel)
                                            {
                                                info!(endpoint = %endpoint_name, target, "Reached shared signature target; broadcasting shutdown");
                                                let _ = shutdown_tx.send(());
                                            }
                                        }
                                    }

                                    transaction_count += 1;
                                }
                            }
                        }
                        // Stream error
                        Ok(Some(Err(e))) => {
                            error!(endpoint = %endpoint_name, error = ?e, "Stream error; reconnecting");
                            break; // break inner loop → reconnect
                        }
                        // Stream closed by server
                        Ok(None) => {
                            warn!(endpoint = %endpoint_name, "Stream closed by server; reconnecting");
                            break; // break inner loop → reconnect
                        }
                    }
                }
            }
        }

        // Backoff before reconnecting (unless shutdown)
        reconnect_attempts += 1;
        if reconnect_attempts > MAX_RECONNECT_ATTEMPTS {
            error!(
                endpoint = %endpoint_name,
                attempts = MAX_RECONNECT_ATTEMPTS,
                "Max reconnection attempts reached; giving up"
            );
            break;
        }
        let delay = backoff_delay(reconnect_attempts - 1);
        warn!(
            endpoint = %endpoint_name,
            attempt = reconnect_attempts,
            delay_secs = delay.as_secs(),
            "Reconnecting after backoff"
        );
        tokio::select! {
            _ = shutdown_rx.recv() => break,
            _ = tokio::time::sleep(delay) => {}
        }
    }

    let unique_signatures = accumulator.len();
    let collected = accumulator.into_inner();
    comparator.add_batch(&endpoint_name, collected);
    info!(
        endpoint = %endpoint_name,
        total_transactions = transaction_count,
        unique_signatures,
        "Stream closed after dispatching transactions"
    );
    Ok(())
}
