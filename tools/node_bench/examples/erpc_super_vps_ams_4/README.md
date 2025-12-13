# ERPC Super VPS AMS (4 vCPU) — example

Machine (from provided panel and run output):

- Location: Amsterdam
- CPU: AMD Ryzen 9 9950X (4 vCPU slice)
- Memory: 16GB
- Disk: 100GB
- OS: Ubuntu

Captured run (fio 4GB, direct=1, time_based=60s, ramp=10s):

- 4K randread: QD1 ~23.8k IOPS; QD32 ~52.4k IOPS (p99 ~0.63 ms)
- 4K randwrite: QD1 ~21.3k IOPS; QD32 ~41.6k IOPS (p99 ~0.86 ms)
- 1M sequential: read ~25 GiB/s; write ~7.6 GiB/s (p99 ~6.5–8.0 ms)
- 4K randrw 70/30 QD16: read ~32.7k / write ~14.0k IOPS (p99 ~1.0 ms write)

Artifacts in this folder:

- `summary.txt` (canonical log)
- `fio_*.json` (raw fio outputs)
- Optional notes: network conditions, package versions, sudo usage, etc.
