# Google Cloud c4d-standard-4 (AMD Turin) — example

Machine configuration:

- Machine type: c4d-standard-4 (4 vCPUs, 15GB memory)
- CPU platform: AMD Turin (5th Gen AMD EPYC)
- Architecture: x86_64
- Disk: 10GB SSD (assumed root disk)
- OS: Ubuntu24.04

Captured run (sysbench cpu, STREAM, fio 4GB direct=1, time_based=60s, ramp=10s):

- CPU (sysbench max prime 20000): 1 thread 1.93k eps, 2 threads 3.86k eps, 4 threads 4.06k eps.
- Memory (STREAM, 4 threads): Copy 57.4 GB/s; Scale 51.9 GB/s; Add 52.3 GB/s; Triad 52.3 GB/s.
- Disk (fio):
  - 4K randread: QD1 1.98k IOPS (p99 1.09 ms); QD32 3.06k IOPS (p99 17.2 ms)
  - 4K randwrite: QD1 2.42k IOPS (p99 1.01 ms); QD32 3.06k IOPS (p99 17.2 ms)
  - 1M sequential: read 153 MB/s (p99 179 ms); write 159 MB/s (p99 107 ms)
  - 4K randrw 70/30 QD16: read 2.14k / write 0.92k IOPS (p99 ~8.2 ms)

Artifacts in this folder:

- `summary.txt` (canonical log)
- `fio_*.json` (raw fio outputs)
- Optional notes: package versions, sudo usage, network conditions, etc.
