# Google Cloud c4d-standard-4 (AMD Turin) — example

Machine configuration:
- Machine type: c4d-standard-4 (4 vCPUs, 15GB memory)
- CPU platform: AMD Turin
- Architecture: x86_64
- Disk: 10GB SSD (assumed root disk)
- Minimum CPU platform: None

Recommended run command (small disk):
```bash
curl -fsSL https://storage-for-testing.erpc.global/tools/node_bench.sh \
  | bash -s -- --fio-dir /var/tmp --fio-size-gb 4
```

Collecting results into this folder:
```bash
RESULTS_BASE=${RESULTS_DIR:-$HOME/results}
LATEST_RUN=$(ls -td "${RESULTS_BASE}"/* | head -n1)
cp -a "${LATEST_RUN}" "./tools/node_bench/examples/gcp_c4d_standard_4/"
```

Expected artifacts to store here:
- `summary.txt` (canonical log)
- `fio_*.json` (raw fio outputs)
- Any additional notes about the run (e.g., package versions, whether sudo was required)
