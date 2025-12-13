# node_bench

## Overview

node_bench is a reproducible benchmark for Linux nodes (VPS / Bare Metal / Cloud). It exercises CPU with sysbench, memory with STREAM (required by default; the script auto-installs or builds it from source, and you can use `--allow-missing-stream` to skip), and disk with fio using direct I/O, fixed profiles, and JSON output. It prints commands to the console, logs everything, and fails fast with clear reasons (permissions, missing directory, insufficient disk space). The maintained script is hosted at [https://storage-for-testing.erpc.global/tools/node_bench.sh](https://storage-for-testing.erpc.global/tools/node_bench.sh).

## What it measures

- CPU: `sysbench cpu` with a thread sweep (1, 2, 4, 8, 16, 32) clamped to available vCPUs; each run logs the full command.
- Memory: STREAM (auto-installs by default, builds from source if needed; falls back only with `--allow-missing-stream`), raw output.
- Disk: `fio` direct I/O, time_based workloads with JSON output:
  - 4K randread QD1 / QD32
  - 4K randwrite QD1 / QD32
  - 1M seqread QD16 / 1M seqwrite QD16
  - 4K randrw 70/30 QD16
    jq (if available) extracts key metrics into `summary.txt`.

## Quick Start (curl)

Small disks / test VMs (10–20GB):

```bash
curl -fsSL https://storage-for-testing.erpc.global/tools/node_bench.sh | bash -s -- --fio-dir /var/tmp --fio-size-gb 4
```

Large NVMe disks (50GB+):

```bash
curl -fsSL https://storage-for-testing.erpc.global/tools/node_bench.sh | bash -s -- --fio-dir /var/tmp --fio-size-gb 64
```

## Requirements

- bash required (do not run with sh/dash)
- Linux
- sysbench and fio are auto-installed if permitted (apt/dnf/yum/apk). If auto-install is disabled or sudo is not available, install them manually.
- STREAM is required by default: the script first attempts package installs (stream/stream-benchmark) and then falls back to fetching the official `stream.c` and compiling it with a local C compiler. If no compiler is present, it will try to install `gcc`/`build-base` when auto-install is enabled. Use `--allow-missing-stream` if you explicitly want to skip memory results.
- jq optional (for extracting p99 latency from fio JSON).

## Disk-size guidance

- 4GB is valid for small disks because direct I/O avoids the page cache and keeps the test file compact; the script checks free space to prevent ENOSPC.
- 64GB is recommended only when the disk has room (e.g., larger NVMe volumes) to provide a longer run; use it only when disk space permits.
- The script refuses to run if there is insufficient free disk space.

## Output location

- Results are stored under `$HOME/results/<hostname>_<UTC timestamp>/` by default (override with `RESULTS_DIR` if needed).
- `summary.txt` is the canonical log for the run.
- `fio_*.json` files are the raw fio outputs.
- The script prints the output directory path to the console.

## How to view results

Show latest summary:

```bash
RESULTS_BASE=${RESULTS_DIR:-$HOME/results}; cat "$(ls -td "${RESULTS_BASE}"/* | head -n1)"/summary.txt
```

Page through it:

```bash
RESULTS_BASE=${RESULTS_DIR:-$HOME/results}; less "$(ls -td "${RESULTS_BASE}"/* | head -n1)"/summary.txt
```

Show only extracted fio metrics sections:

```bash
RESULTS_BASE=${RESULTS_DIR:-$HOME/results}; grep -n "extracted metrics" -A20 "$(ls -td "${RESULTS_BASE}"/* | head -n1)"/summary.txt
```

## Safety notes

- Use `/var/tmp` as the `--fio-dir` rather than `/` to avoid filling the root filesystem.
- fio writes a temporary test file and the script removes it when the run finishes; if a previous run left `node_bench_fio_testfile.dat` (or older `erpc_bench_fio_testfile.dat`) anywhere under the target directory, the script deletes it before running new checks.

## Examples

Example environments and a place to store collected run artifacts live under `tools/node_bench/examples/`:

- `erpc_super_vps_ams_4/` — ERPC Super VPS (AMD Ryzen 9 9950X slice, 4 vCPU / 16GB RAM / 100GB disk, Amsterdam).
- `gcp_c4d_standard_4/` — Google Cloud c4d-standard-4 (AMD Turin, 4 vCPU / 15GB RAM / 10GB SSD).
