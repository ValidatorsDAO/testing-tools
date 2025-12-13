# Validators DAO VPS (Amsterdam) — example

Machine (from provided panel):
- Location: Amsterdam (VPS Cluster #30035)
- vCPU: 4
- Memory: 16GB
- Disk: 100GB
- OS: Ubuntu (per panel)

Recommended run command (aligned to 4GB for cross-environment comparison):
```bash
curl -fsSL https://storage-for-testing.erpc.global/tools/node_bench.sh \
  | bash -s -- --fio-dir /var/tmp --fio-size-gb 4
```

Collecting results into this folder:
```bash
RESULTS_BASE=${RESULTS_DIR:-$HOME/results}
LATEST_RUN=$(ls -td "${RESULTS_BASE}"/* | head -n1)
cp -a "${LATEST_RUN}" "./tools/node_bench/examples/validators_vps_amsterdam/"
```

Expected artifacts to store here:
- `summary.txt` (canonical log)
- `fio_*.json` (raw fio outputs)
- Any additional notes about the run (network conditions, package versions, etc.)
