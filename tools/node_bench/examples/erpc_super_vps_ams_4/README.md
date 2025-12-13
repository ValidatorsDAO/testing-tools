# ERPC Super VPS AMS (4 vCPU) — example

Machine (from provided panel and run output):

- Location: Amsterdam
- CPU: AMD Ryzen 9 9950X (4 vCPU slice)
- Memory: 16GB
- Disk: 100GB
- OS: Ubuntu24.04

Captured run (sysbench cpu, STREAM, fio 4GB direct=1, time_based=60s, ramp=10s):

- CPU (sysbench max prime 20000): 1 thread 1.97k eps, 2 threads 3.94k eps, 4 threads 7.89k eps.
- Memory (STREAM, 4 threads): Copy 30.6 GB/s; Scale 30.7 GB/s; Add 35.4 GB/s; Triad 34.2 GB/s.
- Disk (fio):
  - 4K randread: QD1 23.4k IOPS (p99 0.044 ms); QD32 52.4k IOPS (p99 0.627 ms)
  - 4K randwrite: QD1 21.1k IOPS (p99 0.048 ms); QD32 41.1k IOPS (p99 0.856 ms)
  - 1M sequential: read 26.7 GiB/s (p99 0.60 ms); write 7.9 GiB/s (p99 7.96 ms)
  - 4K randrw 70/30 QD16: read 32.6k / write 14.0k IOPS (p99 ~0.37 ms read / 0.99 ms write)

Artifacts in this folder:

- `summary.txt` (canonical log)
- `fio_*.json` (raw fio outputs)
- Optional notes: network conditions, package versions, sudo usage, etc.
