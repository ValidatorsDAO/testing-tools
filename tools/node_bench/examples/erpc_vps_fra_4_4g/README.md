# ERPC VPS FRA (4 vCPU, 4G) — example

Machine (from provided panel and run output):

- CPU platform: AMD Genoa (4th Gen AMD EPYC)
- CPU: 4 vCPU
- Memory: 16GB
- Disk: 50GB
- OS: Ubuntu24.04
- Location: Frankfurt

Run notes:

- Command: `curl -fsSL https://storage-for-testing.erpc.global/tools/node_bench.sh | bash -s -- --fio-size-gb 4`
- Defaults left intact: `--fio-dir /var/tmp`, `--runtime-sec 60`, `--ramp-sec 10`, `--numjobs 1`, `--ioengine libaio`.
- Timestamp: 2025-12-13T09:04Z on host `ubuntu` (see `summary.txt` for full log).

Captured run (sysbench cpu, STREAM, fio 4GB direct=1, time_based=60s, ramp=10s):

- CPU (sysbench max prime 20000): 1 thread 1.84k eps, 2 threads 3.65k eps, 4 threads 7.25k eps.
- Memory (STREAM, 4 threads): Copy 115.3 GB/s; Scale 137.1 GB/s; Add 148.0 GB/s; Triad 148.3 GB/s.
- Disk (fio):
  - 4K randread: QD1 19.0k IOPS (p99 0.065 ms); QD32 156.5k IOPS (p99 0.31 ms)
  - 4K randwrite: QD1 17.2k IOPS (p99 0.071 ms); QD32 114.9k IOPS (p99 0.40 ms)
  - 1M sequential: read 17.2 GiB/s (p99 1.16 ms); write 3.4 GiB/s (p99 11.6 ms)
  - 4K randrw 70/30 QD16: read 85.8k / write 36.8k IOPS (p99 ~0.18 ms read / 0.30 ms write)

Artifacts in this folder:

- `summary.txt` (canonical log)
- `fio_*.json` (raw fio outputs)
- Optional notes: package versions, sudo usage, network conditions, etc.

More runs and discussion:
- Validators DAO official Discord (active benchmarking chatter): https://discord.com/invite/C7ZQSrCkYR
- PRs with new runs/results are welcome.
