# Google Cloud c4d-standard-4 (AMD Turin) — example

Machine configuration:

- Machine type: c4d-standard-4 (4 vCPUs, 15GB memory)
- CPU platform: AMD Turin (5th Gen AMD EPYC)
- Architecture: x86_64
- Disk: 10GB SSD (assumed root disk)

Captured run (fio 4GB, direct=1, time_based=60s, ramp=10s):

- 4K randread: QD1 ~2.0k IOPS; QD32 ~3.1k IOPS (p99 ~17 ms)
- 4K randwrite: QD1 ~2.5k IOPS; QD32 ~3.1k IOPS (p99 ~17 ms)
- 1M sequential: read ~155 MiB/s; write ~155 MiB/s (p99 ~100–135 ms)
- 4K randrw 70/30 QD16: read ~2.1k / write ~0.9k IOPS (p99 ~8.2 ms)

Artifacts in this folder:

- `summary.txt` (canonical log)
- `fio_*.json` (raw fio outputs)
- Optional notes: package versions, sudo usage, network conditions, etc.
