# node_bench examples

This directory holds reference run artifacts and metadata for common environments. Each subdirectory should include:

- A README with machine specs, the exact `node_bench` command (defaults: `--fio-dir /var/tmp`, `--fio-size-gb 32`, `--runtime-sec 60`, `--ramp-sec 10`), and any notes (sudo usage, network conditions, etc.).
- The captured `summary.txt` and `fio_*.json` files copied from the run output directory (`$HOME/results/<hostname>_<UTC timestamp>/`, or `RESULTS_DIR` if set).

How to add/update an example:

1. Run the maintained script: `curl -fsSL https://storage-for-testing.erpc.global/tools/node_bench.sh | bash` (adjust flags only if needed, e.g., `--fio-size-gb 4` for small disks).
2. Locate the latest results:
   ```bash
   RESULTS_BASE=${RESULTS_DIR:-$HOME/results}
   latest=$(ls -td "${RESULTS_BASE}"/* | head -n1)
   ```
3. Copy `summary.txt` and all `fio_*.json` files into a new subdirectory under `tools/node_bench/examples/<provider_shape>/`.
4. Document the hardware, command used, and any deviations from defaults in that subdirectory’s README.

Available examples:

- `erpc_vps_ams_4/` — ERPC VPS (Amsterdam), AMD Turin (EPYC), 4 vCPU / 16GB RAM / 50GB disk.
- `erpc_vps_fra_4_4g/` — ERPC VPS (Frankfurt), AMD Genoa (EPYC), 4 vCPU / 16GB RAM / 50GB disk.
- `erpc_super_vps_ams_4/` — ERPC Super VPS (Amsterdam), AMD Ryzen 9 9950X slice, 4 vCPU / 16GB RAM / 100GB disk.
- `gcp_c4d_standard_4/` — Google Cloud c4d-standard-4 (AMD Turin), 4 vCPU / 15GB RAM / 10GB SSD.

More runs and discussion:
- Validators DAO official Discord (active benchmarking chatter): https://discord.com/invite/C7ZQSrCkYR
- PRs with new runs/results are welcome.
