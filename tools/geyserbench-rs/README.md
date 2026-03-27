# GeyserBench

Benchmark Solana gRPC data feeds with region-aware latency analysis.

## Use with SLV AI Console

If you're using SLV, the fastest path is:

```bash
curl -fsSL https://storage.slv.dev/slv/install | sh
slv onboard
slv c
```

Then ask the agent to benchmark your endpoints. For example:
- "Benchmark my gRPC endpoints"
- "Compare Yellowstone vs aRPC latency"
- "Run a Frankfurt-only benchmark"
- "Check my local Yellowstone feed on 127.0.0.1:1001"

The SLV AI Console can scaffold config, choose the right benchmarking flow, and run the checks for you.

## Features

- Benchmark multiple feeds simultaneously (Yellowstone, aRPC, Thor, Shredstream, Shreder, Jetstream)
- **Region-aware filtering**: focus benchmarks on specific leader regions (e.g., Frankfurt, Tokyo, NY)
- Ping endpoints to measure connection latency
- Track first-detection share, latency percentiles (P50/P95/P99), valid transactions, and backfill events
- Dual output: global results + region-filtered results side by side

## Installation

```bash
cargo build --release
```

## Quick Start

1. Run once to generate `config.toml`:
   ```bash
   ./target/release/geyserbench
   ```

2. Edit `config.toml` with your endpoints.

3. Run the benchmark:
   ```bash
   # All regions (global)
   ./target/release/geyserbench

   # Filter by leader region
   ./target/release/geyserbench --region frankfurt
   ```

## Region Filtering

When `--region` is specified (or `region` is set in `config.toml`), GeyserBench:

1. Runs the benchmark normally, collecting all transactions
2. After collection, queries the ERPC API to resolve which slots were produced by leaders in the target region
3. Filters results to only include transactions from matching slots
4. Displays both global and region-filtered results

This gives you accurate latency measurements specific to where the leader is located, eliminating the noise from leaders in distant regions.

### Requirements for Region Filtering

Add to `config.toml`:
```toml
[config]
region = "frankfurt"
erpc_url = "https://edge.erpc.global"
erpc_api_key = "your-api-key"
```

### Available Regions

```
amsterdam, frankfurt, ny, chicago, stockholm, saltlakecity,
tokyo, london, singapore, dublin, sydney, losangeles, hongkong
```

Use `--list-regions` to see the full list.

## Configuration

```toml
[config]
transactions = 1000
account = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"
commitment = "processed"
# region = "frankfurt"          # Optional: filter by leader region
# erpc_url = "https://edge.erpc.global"  # Required for region filter
# erpc_api_key = "your-key"    # Required for region filter

[[endpoint]]
name = "grpc"
url = "http://127.0.0.1:1001"
kind = "yellowstone"
# x_token = "optional-auth-token"
```

## CLI Options

| Flag | Description |
|---|---|
| `--config <PATH>` | Load config from a different file (default: `config.toml`) |
| `--region <REGION>` | Filter results by leader region (overrides config) |
| `--list-regions` | Show available regions and exit |
| `-h, --help` | Show usage |

## License

Apache-2.0
