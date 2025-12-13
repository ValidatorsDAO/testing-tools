# ERPC Super VPS AMS (4 vCPU) — example

Machine (from provided panel and run output):

- CPU: AMD Ryzen 9 9950X (4 vCPU slice)
- Memory: 16GB
- Disk: 100GB
- OS: Ubuntu24.04
- Location: Amsterdam

Captured run (sysbench cpu, STREAM, fio 4GB direct=1, time_based=60s, ramp=10s):

- CPU (sysbench max prime 20000): 1 thread 1.97k eps, 2 threads 3.94k eps, 4 threads 7.88k eps.
- Memory (STREAM, 4 threads): Copy 41.6 GB/s; Scale 28.3 GB/s; Add 31.6 GB/s; Triad 31.6 GB/s.
- Disk (fio):
  - 4K randread: QD1 23.4k IOPS (p99 0.044 ms); QD32 52.3k IOPS (p99 0.63 ms)
  - 4K randwrite: QD1 21.1k IOPS (p99 0.048 ms); QD32 41.5k IOPS (p99 0.86 ms)
  - 1M sequential: read 27.3 GiB/s (p99 0.60 ms); write 7.7 GiB/s (p99 8.0 ms)
  - 4K randrw 70/30 QD16: read 32.8k / write 14.1k IOPS (p99 ~0.37 ms read / 1.00 ms write)

Artifacts in this folder:

- `summary.txt` (canonical log)
- `fio_*.json` (raw fio outputs)
- Optional notes: network conditions, package versions, sudo usage, etc.
