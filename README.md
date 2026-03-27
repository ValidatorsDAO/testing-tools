# Validators DAO Performance Testing Tools

This repository contains practical performance testing tools maintained by Validators DAO.

The purpose of this repository is simple:  
to allow anyone to **measure, verify, and understand the performance characteristics of Linux nodes** in a reproducible way.

The tools here are designed for:

- VPS
- Bare metal servers
- Cloud instances

They are intended for **verification and comparison**, not for marketing claims.

All tools live under `tools/<tool_name>/`.

## Quick Start

If you just want to run a performance check on a Linux node, start with `node_bench`.

### node_bench

`node_bench` is a reproducible benchmark that measures CPU, memory (optional), and disk performance at the node level.

Run directly via `curl` (defaults: `--fio-dir /var/tmp`, `--fio-size-gb 32`, `--runtime-sec 60`, `--ramp-sec 10`):

```bash
curl -fsSL https://storage-for-testing.erpc.global/tools/node_bench.sh | bash
```

If the root disk is tight (≈10–20GB free), shrink the fio file:

```bash
curl -fsSL https://storage-for-testing.erpc.global/tools/node_bench.sh | bash -s -- --fio-size-gb 4
```

If you want a longer disk run on a large NVMe volume, bump the size:

```bash
curl -fsSL https://storage-for-testing.erpc.global/tools/node_bench.sh | bash -s -- --fio-size-gb 64
```

`--fio-dir` already defaults to `/var/tmp`; only set it when you need a different mount point.

The script:

- prints every command it executes
- logs all output to disk
- stops immediately if requirements are not met (permissions, disk space, paths)

## Tools

### geyserbench-rs

A source mirror of the `geyserbench-rs` benchmark tool for Solana gRPC-compatible data feeds.

Contents:
- Rust source tree mirrored under `tools/geyserbench-rs/`
- configuration, protobuf definitions, and benchmark logic as maintained upstream

This repository currently publishes `node_bench` directly to R2. `geyserbench-rs` is included here first as source so it can be reviewed, referenced, and wired into a dedicated publish flow later.

### node_bench

A node-level benchmark for Linux systems.

What it measures:

- **CPU**
  Using `sysbench` with a multi-thread sweep.

- **Memory**
  Using STREAM (auto-installed or built from source; skip with `--allow-missing-stream` if you accept missing memory results).

- **Disk**
  Using `fio` with:

  - direct I/O (`direct=1`)
  - fixed, explicit workloads
  - JSON output for independent analysis

Results are written to a timestamped directory:

```
$HOME/results/<hostname>_<UTC timestamp>/   # override with RESULTS_DIR if needed
```

Key files:

- `summary.txt` — full console log (canonical record)
- `fio_*.json` — raw fio results

Documentation:

- See `tools/node_bench/README.md` for detailed usage and result interpretation.

Execution (maintained script):

- [https://storage-for-testing.erpc.global/tools/node_bench.sh](https://storage-for-testing.erpc.global/tools/node_bench.sh)

Community:
- Join the Validators DAO official Discord for active benchmarking chatter, run logs, and Q&A: https://discord.com/invite/C7ZQSrCkYR
- Pull requests with new run results or examples are welcome.

## Viewing Results

After `node_bench` finishes, you can inspect results immediately.

Show the latest summary:

```bash
RESULTS_BASE=${RESULTS_DIR:-$HOME/results}; cat "$(ls -td "${RESULTS_BASE}"/* | head -n1)"/summary.txt
```

Page through it:

```bash
RESULTS_BASE=${RESULTS_DIR:-$HOME/results}; less "$(ls -td "${RESULTS_BASE}"/* | head -n1)"/summary.txt
```

Show only extracted fio metrics:

```bash
RESULTS_BASE=${RESULTS_DIR:-$HOME/results}; grep -n "extracted metrics" -A20 "$(ls -td "${RESULTS_BASE}"/* | head -n1)"/summary.txt
```

The script also prints the result directory path to the console when it completes.

## Design Principles

All tools in this repository follow the same principles:

- no silent execution
- no hidden fallbacks
- no cache-based shortcuts
- fail fast with clear errors
- safe defaults (no destructive operations)

If a condition is not suitable for accurate measurement, the tool will stop instead of producing misleading results.

## License

Apache License 2.0
