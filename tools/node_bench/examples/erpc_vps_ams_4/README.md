# ERPC VPS AMS (4 vCPU) — example

Machine (from provided panel and run output):

- Location: Amsterdam
- CPU platform: AMD Turin (5th Gen AMD EPYC)
- CPU: 4 vCPU
- Memory: 16GB
- Disk: 50GB
- OS: Ubuntu24.04

Captured run (sysbench cpu, STREAM, fio 4GB direct=1, time_based=60s, ramp=10s):

- CPU (sysbench max prime 20000): 1 thread 1.96k eps, 2 threads 3.93k eps, 4 threads 7.83k eps.
- Memory (STREAM, 4 threads): Copy 293 GB/s; Scale 238 GB/s; Add 255 GB/s; Triad 215 GB/s.
- Disk (fio):
  - 4K randread: QD1 19.9k IOPS (p99 0.059 ms); QD32 51.2k IOPS (p99 0.676 ms)
  - 4K randwrite: QD1 17.7k IOPS (p99 0.064 ms); QD32 40.0k IOPS (p99 1.17 ms)
  - 1M sequential: read 27.3 GiB/s (p99 0.668 ms); write 5.71 GiB/s (p99 8.16 ms)
  - 4K randrw 70/30 QD16: read 31.1k / write 13.3k IOPS (p99 ~0.45 ms read / 1.07 ms write)
