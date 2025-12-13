# ERPC VPS AMS (4 vCPU) — example

Machine (from provided panel and run output):

- CPU platform: AMD Turin (5th Gen AMD EPYC)
- CPU: 4 vCPU
- Memory: 16GB
- Disk: 50GB
- OS: Ubuntu24.04
- Location: Amsterdam

Captured run (sysbench cpu, STREAM, fio 4GB direct=1, time_based=60s, ramp=10s):

- CPU (sysbench max prime 20000): 1 thread 1.97k eps, 2 threads 3.94k eps, 4 threads 7.85k eps.
- Memory (STREAM, 4 threads): Copy 178.7 GB/s; Scale 155.1 GB/s; Add 162.5 GB/s; Triad 152.0 GB/s.
- Disk (fio):
  - 4K randread: QD1 19.7k IOPS (p99 0.057 ms); QD32 50.7k IOPS (p99 0.67 ms)
  - 4K randwrite: QD1 18.5k IOPS (p99 0.064 ms); QD32 39.2k IOPS (p99 1.14 ms)
  - 1M sequential: read 27.1 GiB/s (p99 0.63 ms); write 5.8 GiB/s (p99 7.96 ms)
  - 4K randrw 70/30 QD16: read 31.2k / write 13.4k IOPS (p99 ~0.42 ms read / 1.06 ms write)

Artifacts in this folder:

- `summary.txt` (canonical log)
- `fio_*.json` (raw fio outputs)
- Optional notes: package versions, sudo usage, network conditions, etc.
