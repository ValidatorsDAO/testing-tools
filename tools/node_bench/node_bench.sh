#!/usr/bin/env bash
set -Eeuo pipefail

###############################################################################
# node_bench.sh
#
# Reproducible node benchmark for Linux (VPS / Bare Metal / Cloud).
#
# Policy (default behavior):
# - Auto-install required tools by default (AUTO_INSTALL=1).
# - CPU / Memory / Disk are treated equally: if any required benchmark tool
#   cannot run (sysbench/fio/stream), the run is considered invalid and fails.
#
# Benchmarks:
# - CPU: sysbench cpu (thread sweep)
# - RAM: STREAM (required by default; auto-installs if missing)
# - Disk: fio (direct I/O, fixed profiles; JSON + extracted metrics)
#
# Output (default base: $HOME/results or override RESULTS_DIR):
# - $HOME/results/<hostname>_<UTC timestamp>/summary.txt
# - $HOME/results/<hostname>_<UTC timestamp>/fio_*.json
###############################################################################

# -----------------------------------------------------------------------------
# Bash guard
# -----------------------------------------------------------------------------
if [[ -z "${BASH_VERSION:-}" ]]; then
  echo "ERROR: This script requires bash. Do not run with 'sh'." >&2
  echo "Use: curl -fsSL <url> | bash -s -- [args]" >&2
  exit 2
fi

# -----------------------------------------------------------------------------
# Defaults (override via args or environment)
# -----------------------------------------------------------------------------
AUTO_INSTALL="${AUTO_INSTALL:-1}"                 # default: auto-install deps
INSTALL_JQ="${INSTALL_JQ:-1}"                     # default: best-effort install jq
INSTALL_STREAM="${INSTALL_STREAM:-1}"             # default: STREAM required + auto-install
ALLOW_MISSING_STREAM="${ALLOW_MISSING_STREAM:-0}" # default: do NOT allow missing STREAM

FIO_DIR="${FIO_DIR:-/var/tmp}"
FIO_SIZE_GB="${FIO_SIZE_GB:-32}"
FIO_RUNTIME_SEC="${FIO_RUNTIME_SEC:-60}"
FIO_RAMP_SEC="${FIO_RAMP_SEC:-10}"
FIO_NUMJOBS="${FIO_NUMJOBS:-1}"
FIO_IOENGINE="${FIO_IOENGINE:-libaio}"
SYSBENCH_CPU_MAX_PRIME="${SYSBENCH_CPU_MAX_PRIME:-20000}"
RESULTS_DIR="${RESULTS_DIR:-}"

CPU_THREADS_LIST_DEFAULT=("1" "2" "4" "8" "16" "32")
CPU_THREADS_LIST=("${CPU_THREADS_LIST_DEFAULT[@]}")

STREAM_INSTALL_ATTEMPTED=0
STREAM_INSTALL_MESSAGE=""
STREAM_INSTALL_PACKAGE=""
STREAM_PRESENT=0
STREAM_BINARY="stream"
STREAM_BUILD_MESSAGE=""
STREAM_CC=""
STREAM_FETCHER=""
NEEDRESTART_CONFIGURED=0
STREAM_ARRAY_TOTAL_MB_DEFAULT=4096
STREAM_ARRAY_TOTAL_MB="${STREAM_ARRAY_TOTAL_MB:-$STREAM_ARRAY_TOTAL_MB_DEFAULT}"
STREAM_ARRAY_BYTES=0
STREAM_ARRAY_ELEMENTS=0
STREAM_SIZE_CHECKED=0
STREAM_REBUILT=0

# -----------------------------------------------------------------------------
# CLI flags
# -----------------------------------------------------------------------------
usage() {
  cat <<'EOF'
Usage:
  node_bench.sh [options]

Options:
  --no-install                 Do not install missing dependencies (fail instead)
  --no-jq                      Do not install/use jq (JSON extraction will be skipped)
  --allow-missing-stream       Continue without STREAM even if unavailable (NOT recommended)
  --fio-dir DIR                Directory to place fio test file (must be writable)
  --fio-size-gb N              fio file size in GB (default: 32)
  --runtime-sec N              fio runtime in seconds (default: 60)
  --ramp-sec N                 fio ramp/warmup in seconds (default: 10)
  --numjobs N                  fio numjobs (default: 1)
  --ioengine NAME              fio ioengine (default: libaio)
  --cpu-max-prime N            sysbench cpu max prime (default: 20000)
  -h, --help                   Show this help

Examples:
  curl -fsSL https://host/node_bench.sh | bash -s -- --fio-dir /var/tmp --fio-size-gb 4
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-install) AUTO_INSTALL=0; shift ;;
    --no-jq) INSTALL_JQ=0; shift ;;
    --allow-missing-stream) ALLOW_MISSING_STREAM=1; shift ;;
    --fio-dir) FIO_DIR="$2"; shift 2 ;;
    --fio-size-gb) FIO_SIZE_GB="$2"; shift 2 ;;
    --runtime-sec) FIO_RUNTIME_SEC="$2"; shift 2 ;;
    --ramp-sec) FIO_RAMP_SEC="$2"; shift 2 ;;
    --numjobs) FIO_NUMJOBS="$2"; shift 2 ;;
    --ioengine) FIO_IOENGINE="$2"; shift 2 ;;
    --cpu-max-prime) SYSBENCH_CPU_MAX_PRIME="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "ERROR: Unknown argument: $1" >&2; usage; exit 2 ;;
  esac
done

# -----------------------------------------------------------------------------
# Helpers
# -----------------------------------------------------------------------------
ts_utc() { date -u +"%Y%m%dT%H%M%SZ"; }
have() { command -v "$1" >/dev/null 2>&1; }

fatal() {
  echo
  echo "FATAL: $*" >&2
  echo >&2
  exit 1
}

note() {
  echo
  echo "==> $*"
}

cmd() {
  echo "\$ $*"
  "$@"
}

on_error() {
  local exit_code=$?
  echo
  echo "ERROR: node_bench failed (exit code: ${exit_code}). Last command:" >&2
  echo "  ${BASH_COMMAND}" >&2
  echo >&2
  echo "Troubleshooting checklist:" >&2
  echo "  - Run with bash (not sh): curl ... | bash -s --" >&2
  echo "  - Ensure FIO_DIR exists and is writable" >&2
  echo "  - Ensure enough free disk space for --fio-size-gb + overhead" >&2
  echo "  - Ensure sudo is permitted non-interactively (or run as root) for auto-install" >&2
  exit "${exit_code}"
}
trap on_error ERR

need_root_or_sudo() {
  if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
    return 0
  fi
  if have sudo; then
    sudo -n true >/dev/null 2>&1 || return 1
    return 0
  fi
  return 1
}

sudo_prefix() {
  if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
    echo ""
  else
    echo "sudo -n"
  fi
}

pkg_install() {
  local pkgs=("$@")

  [[ "${AUTO_INSTALL}" -eq 1 ]] || fatal "Missing dependencies: ${pkgs[*]} (auto-install disabled via --no-install)."

  need_root_or_sudo || fatal "Auto-install is default, but sudo is not available non-interactively. Run as root or pre-install: ${pkgs[*]}"

  local SUDO
  SUDO="$(sudo_prefix)"

  if have apt-get; then
    configure_needrestart_quiet || true
    note "Installing dependencies via apt-get: ${pkgs[*]}"
    cmd ${SUDO} apt-get update -y
    cmd ${SUDO} apt-get install -y "${pkgs[@]}"
  elif have dnf; then
    note "Installing dependencies via dnf: ${pkgs[*]}"
    cmd ${SUDO} dnf install -y "${pkgs[@]}"
  elif have yum; then
    note "Installing dependencies via yum: ${pkgs[*]}"
    cmd ${SUDO} yum install -y "${pkgs[@]}"
  elif have apk; then
    note "Installing dependencies via apk: ${pkgs[*]}"
    cmd ${SUDO} apk add --no-cache "${pkgs[@]}"
  else
    fatal "No supported package manager found (apt-get/dnf/yum/apk). Install manually: sysbench fio (jq optional; stream required)."
  fi
}

configure_needrestart_quiet() {
  [[ "${NEEDRESTART_CONFIGURED}" -eq 1 ]] && return 0
  NEEDRESTART_CONFIGURED=1

  have apt-get || return 0
  need_root_or_sudo || return 0

  local SUDO; SUDO="$(sudo_prefix)"
  local conf="/etc/needrestart/needrestart.conf"
  local body='$nrconf{restart} = '\''a'\'';'$'\n''$nrconf{kernelhints} = 0;'$'\n''$nrconf{verbosity} = 0;'

  note "Disabling needrestart prompts (non-interactive)"
  cmd ${SUDO} mkdir -p /etc/needrestart
  printf '%s\n' "$body" | cmd ${SUDO} tee "$conf" >/dev/null
}

ensure_c_compiler_for_stream() {
  STREAM_CC=""

  local cc
  cc="$(command -v cc || command -v gcc || command -v clang || true)"
  if [[ -n "$cc" ]]; then
    STREAM_CC="$cc"
    return 0
  fi

  [[ "${AUTO_INSTALL}" -eq 1 ]] || { STREAM_BUILD_MESSAGE="No C compiler found for STREAM build and auto-install disabled (--no-install or AUTO_INSTALL=0)."; return 1; }
  if ! need_root_or_sudo; then
    STREAM_BUILD_MESSAGE="No C compiler found for STREAM build and sudo/root is not available non-interactively."
    return 1
  fi

  local SUDO; SUDO="$(sudo_prefix)"
  if have apt-get; then
    configure_needrestart_quiet || true
    note "Installing gcc for STREAM build via apt-get"
    cmd ${SUDO} apt-get update -y
    if ! cmd ${SUDO} apt-get install -y gcc; then
      STREAM_BUILD_MESSAGE="Failed to install gcc via apt-get for STREAM build."
      return 1
    fi
  elif have dnf; then
    note "Installing gcc for STREAM build via dnf"
    if ! cmd ${SUDO} dnf install -y gcc; then
      STREAM_BUILD_MESSAGE="Failed to install gcc via dnf for STREAM build."
      return 1
    fi
  elif have yum; then
    note "Installing gcc for STREAM build via yum"
    if ! cmd ${SUDO} yum install -y gcc; then
      STREAM_BUILD_MESSAGE="Failed to install gcc via yum for STREAM build."
      return 1
    fi
  elif have apk; then
    note "Installing build-base for STREAM build via apk"
    if ! cmd ${SUDO} apk add --no-cache build-base; then
      STREAM_BUILD_MESSAGE="Failed to install build-base via apk for STREAM build."
      return 1
    fi
  else
    STREAM_BUILD_MESSAGE="No supported package manager to install a C compiler for STREAM build."
    return 1
  fi

  cc="$(command -v cc || command -v gcc || command -v clang || true)"
  if [[ -z "$cc" ]]; then
    STREAM_BUILD_MESSAGE="C compiler install attempted but compiler is still unavailable."
    return 1
  fi

  STREAM_CC="$cc"
  return 0
}

ensure_fetcher_for_stream() {
  STREAM_FETCHER=""

  if have curl; then
    STREAM_FETCHER="curl"
    return 0
  fi
  if have wget; then
    STREAM_FETCHER="wget"
    return 0
  fi

  [[ "${AUTO_INSTALL}" -eq 1 ]] || { STREAM_BUILD_MESSAGE="Cannot fetch STREAM source: need curl or wget and auto-install is disabled (--no-install or AUTO_INSTALL=0)."; return 1; }
  if ! need_root_or_sudo; then
    STREAM_BUILD_MESSAGE="Cannot fetch STREAM source: curl/wget missing and sudo/root is not available non-interactively."
    return 1
  fi

  local SUDO; SUDO="$(sudo_prefix)"
  local pm=""
  local install_cmd=()

  if have apt-get; then
    pm="apt-get"
    configure_needrestart_quiet || true
    note "Installing curl/wget for STREAM fetch via ${pm}"
    cmd ${SUDO} apt-get update -y
    install_cmd=(${SUDO} apt-get install -y curl wget)
  elif have dnf; then
    pm="dnf"
    note "Installing curl/wget for STREAM fetch via ${pm}"
    install_cmd=(${SUDO} dnf install -y curl wget)
  elif have yum; then
    pm="yum"
    note "Installing curl/wget for STREAM fetch via ${pm}"
    install_cmd=(${SUDO} yum install -y curl wget)
  elif have apk; then
    pm="apk"
    note "Installing curl/wget for STREAM fetch via ${pm}"
    install_cmd=(${SUDO} apk add --no-cache curl wget)
  else
    STREAM_BUILD_MESSAGE="Cannot fetch STREAM source: no supported package manager to install curl/wget (need apt-get/dnf/yum/apk)."
    return 1
  fi

  if ! cmd "${install_cmd[@]}"; then
    local exit_code=$?
    STREAM_BUILD_MESSAGE="Failed to install curl/wget via ${pm} (exit ${exit_code})."
    return 1
  fi

  hash -r || true
  if have curl; then
    STREAM_FETCHER="curl"
    return 0
  fi
  if have wget; then
    STREAM_FETCHER="wget"
    return 0
  fi

  STREAM_BUILD_MESSAGE="Cannot fetch STREAM source: curl/wget still unavailable after install attempt."
  return 1
}

build_stream_from_source() {
  STREAM_BUILD_MESSAGE=""

  if ! ensure_c_compiler_for_stream; then
    return 1
  fi
  local cc="$STREAM_CC"
  if ! ensure_fetcher_for_stream; then
    return 1
  fi
  local fetcher="$STREAM_FETCHER"

  local build_dir
  build_dir="$(mktemp -d /tmp/node_bench_stream_XXXXXX 2>/dev/null || true)"
  [[ -n "$build_dir" ]] || { STREAM_BUILD_MESSAGE="Failed to create temporary directory for STREAM build."; return 1; }
  local cleanup_cmd="rm -rf \"$build_dir\" >/dev/null 2>&1"
  trap "$cleanup_cmd" RETURN

  local src="${build_dir}/stream.c"
  local url="https://www.cs.virginia.edu/stream/FTP/Code/stream.c"

  if [[ "$fetcher" == "curl" ]]; then
    note "Fetching STREAM source from ${url}"
    if ! curl -fsSL "$url" -o "$src"; then
      STREAM_BUILD_MESSAGE="Failed to download STREAM source from ${url}"
      return 1
    fi
  else
    note "Fetching STREAM source from ${url} (via wget)"
    if ! wget -qO "$src" "$url"; then
      STREAM_BUILD_MESSAGE="Failed to download STREAM source from ${url}"
      return 1
    fi
  fi

  local target="${build_dir}/stream"
  local compile_log="${build_dir}/compile.log"
  local build_variant=""
  local arr_total_mb="$STREAM_ARRAY_TOTAL_MB"

  # Choose array size (total across 3 arrays) to exceed LLC; clamp to available mem (~70%) to avoid OOM.
  local memavail_kb
  memavail_kb="$(awk '/MemAvailable:/ {print $2}' /proc/meminfo 2>/dev/null || true)"
  local memavail_bytes=0
  if [[ -n "$memavail_kb" ]]; then
    memavail_bytes=$((memavail_kb * 1024))
  fi
  local desired_bytes=$((arr_total_mb * 1024 * 1024))
  local cap_bytes="$desired_bytes"
  if [[ "$memavail_bytes" -gt 0 ]]; then
    local cap=$((memavail_bytes * 7 / 10))
    if [[ "$cap" -gt 0 && "$desired_bytes" -gt "$cap" ]]; then
      cap_bytes="$cap"
    fi
  fi
  # Require at least 128MiB total to keep runs meaningful.
  local min_bytes=$((128 * 1024 * 1024))
  if [[ "$cap_bytes" -lt "$min_bytes" ]]; then
    cap_bytes="$min_bytes"
  fi
  STREAM_ARRAY_BYTES="$cap_bytes"
  STREAM_ARRAY_ELEMENTS=$((STREAM_ARRAY_BYTES / 8 / 3))
  if [[ "$STREAM_ARRAY_ELEMENTS" -lt 1000000 ]]; then
    STREAM_ARRAY_ELEMENTS=1000000
    STREAM_ARRAY_BYTES=$((STREAM_ARRAY_ELEMENTS * 8 * 3))
  fi
  local arr_mb=$((STREAM_ARRAY_BYTES / 1024 / 1024))
  note "STREAM array sizing: requested ~${arr_total_mb} MiB total; using ~${arr_mb} MiB total across 3 arrays (MemAvailable ~${memavail_bytes} bytes)."
  local stream_array_flag="-DSTREAM_ARRAY_SIZE=${STREAM_ARRAY_ELEMENTS}"

  note "Building STREAM from source using ${cc} (attempting OpenMP, falling back if unavailable)"
  if "$cc" -O3 -fopenmp "$stream_array_flag" "$src" -o "$target" >"$compile_log" 2>&1; then
    chmod +x "$target" || true
    build_variant="with OpenMP"
  else
    note "Retrying STREAM build without OpenMP flags"
    if "$cc" -O3 "$stream_array_flag" "$src" -o "$target" >"$compile_log" 2>&1; then
      chmod +x "$target" || true
      build_variant="without OpenMP"
    else
      local tail_log
      tail_log="$(tail -n 20 "$compile_log" 2>/dev/null || true)"
      STREAM_BUILD_MESSAGE="Failed to build STREAM from source with ${cc}${tail_log:+; tail of build log:\n${tail_log}}"
      return 1
    fi
  fi

  local install_target="/usr/local/bin/stream"
  local SUDO; SUDO="$(sudo_prefix)"
  if ! need_root_or_sudo; then
    STREAM_BUILD_MESSAGE="Built STREAM binary (${build_variant}) but cannot install to ${install_target}: sudo/root not available non-interactively."
    return 1
  fi

  note "Installing STREAM binary to ${install_target}"
  if ! cmd ${SUDO} install -m 0755 "$target" "$install_target"; then
    STREAM_BUILD_MESSAGE="Built STREAM binary (${build_variant}) but failed to install to ${install_target}."
    return 1
  fi

  hash -r || true
  STREAM_BINARY="$install_target"
  STREAM_BUILD_MESSAGE="Built STREAM from source using ${cc} (${build_variant}) and installed to ${install_target}"
  STREAM_REBUILT=1

  # Clean up build dir and remove RETURN trap for subsequent functions
  rm -rf "$build_dir" >/dev/null 2>&1 || true
  trap - RETURN
  return 0
}

install_stream_with_fallback() {
  STREAM_INSTALL_ATTEMPTED=1
  STREAM_INSTALL_MESSAGE=""
  STREAM_INSTALL_PACKAGE=""
  STREAM_BINARY="stream"

  if [[ "${AUTO_INSTALL}" -ne 1 ]]; then
    STREAM_INSTALL_MESSAGE="Install not attempted: auto-install disabled (--no-install or AUTO_INSTALL=0)."
    return 1
  fi

  local SUDO; SUDO="$(sudo_prefix)"

  local pm=""
  local update_cmd=()
  local install_cmd=()

  if have apt-get; then
    pm="apt-get"
    configure_needrestart_quiet || true
    update_cmd=(${SUDO} apt-get update -y)
    install_cmd=(${SUDO} apt-get install -y)
  elif have dnf; then
    pm="dnf"
    install_cmd=(${SUDO} dnf install -y)
  elif have yum; then
    pm="yum"
    install_cmd=(${SUDO} yum install -y)
  elif have apk; then
    pm="apk"
    install_cmd=(${SUDO} apk add --no-cache)
  else
    STREAM_INSTALL_MESSAGE="No supported package manager found for stream (need apt-get/dnf/yum/apk)."
    pm=""
  fi

  local candidates=("stream" "stream-benchmark")
  local install_error=""

  if [[ -n "$pm" && need_root_or_sudo ]]; then
    if [[ "${#update_cmd[@]}" -gt 0 ]]; then
      note "Updating package index via ${pm} for STREAM"
      if ! cmd "${update_cmd[@]}"; then
        install_error="${pm} update failed while preparing to install stream."
      fi
    fi

    if [[ -z "${install_error}" ]]; then
      local last_error=""
      for pkg in "${candidates[@]}"; do
        note "Attempting to install STREAM via ${pm}: package '${pkg}'"
        if cmd "${install_cmd[@]}" "$pkg"; then
          STREAM_INSTALL_PACKAGE="$pkg"
          STREAM_INSTALL_MESSAGE="Installed '${pkg}' via ${pm}"
          hash -r || true
          STREAM_BINARY="$(command -v stream || echo "stream")"
          local prev_present="$STREAM_PRESENT"
          STREAM_PRESENT=1
          ensure_stream_big_arrays || { STREAM_PRESENT="$prev_present"; return 1; }
          STREAM_PRESENT="$prev_present"
          if [[ "${STREAM_REBUILT}" -eq 1 ]]; then
            STREAM_INSTALL_MESSAGE="${STREAM_INSTALL_MESSAGE}; rebuilt STREAM with large arrays"
          fi
          return 0
        else
          local exit_code=$?
          last_error="${pm} install exited ${exit_code} for package '${pkg}'"
        fi
      done

      install_error="STREAM install failed via ${pm} (tried: ${candidates[*]}${last_error:+; last error: ${last_error}})"
    fi
  elif [[ -n "$pm" ]]; then
    install_error="STREAM package install skipped: sudo/root not available non-interactively for ${pm}."
  else
    install_error="No supported package manager found for stream (need apt-get/dnf/yum/apk)."
  fi

  if build_stream_from_source; then
    STREAM_INSTALL_MESSAGE="${install_error:+${install_error}; }${STREAM_BUILD_MESSAGE}"
    return 0
  fi

  if [[ -n "${STREAM_BUILD_MESSAGE}" ]]; then
    STREAM_INSTALL_MESSAGE="${install_error}${install_error:+; }${STREAM_BUILD_MESSAGE}"
  else
    STREAM_INSTALL_MESSAGE="${install_error:-"STREAM install failed."}"
  fi
  return 1
}

clamp_threads() {
  local max_threads="$1"
  local out=()
  for t in "${CPU_THREADS_LIST[@]}"; do
    if [[ "$t" -le "$max_threads" ]]; then out+=("$t"); fi
  done
  [[ "${#out[@]}" -gt 0 ]] || out=("1")
  echo "${out[*]}"
}

ensure_dir_writable() {
  local d="$1"

  if [[ ! -d "$d" ]]; then
    if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
      note "FIO_DIR does not exist. Creating: $d"
      cmd mkdir -p "$d"
    elif have sudo && sudo -n true >/dev/null 2>&1; then
      note "FIO_DIR does not exist. Creating with sudo: $d"
      cmd sudo -n mkdir -p "$d"
    else
      fatal "FIO_DIR does not exist: $d (and cannot create without root/sudo)."
    fi
  fi

  local testfile="${d}/.node_bench_write_test_$$"
  if ! ( : > "$testfile" ) 2>/dev/null; then
    fatal "Cannot write to FIO_DIR: $d (choose a writable directory like /var/tmp, or run with sudo)."
  fi
  rm -f "$testfile" || true
}

check_free_space() {
  local d="$1"
  local need_kb="$2"

  local free_kb
  free_kb="$(df -k --output=avail "$d" | tail -n1 | tr -d ' ')"

  if [[ -z "$free_kb" || "$free_kb" -lt "$need_kb" ]]; then
    local free_gb=$((free_kb/1024/1024))
    local need_gb=$((need_kb/1024/1024))
    fatal "Not enough free space in $d. Need ~${need_gb}GB; available ~${free_gb}GB."
  fi
}

canonicalize_dir() {
  local d="$1"
  if have realpath; then
    realpath -m "$d" 2>/dev/null || true
  else
    (cd "$d" 2>/dev/null && pwd -P) || true
  fi
}

cleanup_stale_fio_artifacts() {
  local d="$1"
  local find_base=(
    "$d" -maxdepth 2
    \( -path "$d"/systemd-private\* -o -path "$d"/*/systemd-private\* \) -prune -o
    -type f \( -name 'node_bench_fio_testfile.dat' -o -name 'erpc_bench_fio_testfile.dat' \)
  )

  local found
  found="$(find "${find_base[@]}" -print 2>/dev/null || true)"
  if [[ -n "$found" ]]; then
    note "Removing stale fio test files under $d (depth<=2):"
    echo "$found"
    find "${find_base[@]}" -exec rm -f {} + 2>/dev/null || true
  fi
}

cleanup_stale_fio_file() {
  local f="$1"
  if [[ -f "$f" ]]; then
    local size_h=""
    size_h="$(du -h "$f" 2>/dev/null | awk '{print $1}')" || true
    note "Found existing fio test file: $f${size_h:+ (size: $size_h)}"
    echo "Removing stale fio test file before disk-space check."
    rm -f "$f" || fatal "Failed to remove stale fio test file: $f"
  fi
}

stream_total_mib() {
  local out
  out="$({ OMP_NUM_THREADS=1 "${STREAM_BINARY:-stream}" 2>/dev/null; } | awk '/Total memory required/ {for(i=1;i<=NF;i++) if($i=="MiB"){print $(i-1); exit}}' | head -n1)"
  echo "${out:-0}"
}

ensure_stream_big_arrays() {
  [[ "${STREAM_PRESENT}" -eq 1 ]] || return 1
  [[ "${STREAM_SIZE_CHECKED}" -eq 1 ]] && return 0
  STREAM_SIZE_CHECKED=1

  local want="${STREAM_ARRAY_TOTAL_MB}"
  local have_mib
  have_mib="$(stream_total_mib || echo 0)"
  # stream_total_mib may return decimals; coerce to integer for arithmetic
  have_mib="${have_mib%%.*}"
  [[ -z "$have_mib" ]] && have_mib=0

  if [[ "$have_mib" == "0" ]]; then
    note "STREAM size check: could not parse current stream output; rebuilding STREAM with large arrays."
    build_stream_from_source || return 1
    return 0
  fi

  if (( have_mib + 64 < want )); then
    note "STREAM size check: current Total memory required ~${have_mib} MiB < target ${want} MiB. Rebuilding STREAM with large arrays."
    build_stream_from_source || return 1
  else
    note "STREAM size check: current Total memory required ~${have_mib} MiB meets target (${want} MiB)."
  fi
}

# -----------------------------------------------------------------------------
# Dependency resolution (CPU/Disk are required; JQ optional)
# -----------------------------------------------------------------------------
note "Dependency check"
missing=()
have sysbench || missing+=("sysbench")
have fio || missing+=("fio")

if [[ "${#missing[@]}" -gt 0 ]]; then
  pkg_install "${missing[@]}"
fi

if [[ "${INSTALL_JQ}" -eq 1 ]] && ! have jq; then
  pkg_install jq || true
fi

have sysbench || fatal "sysbench not found after install attempt."
have fio || fatal "fio not found after install attempt."

# -----------------------------------------------------------------------------
# Output directory + logging (console + file)
# -----------------------------------------------------------------------------
HOST="$(hostname -s 2>/dev/null || hostname)"
TS="$(ts_utc)"
RESULTS_BASE_RAW="${RESULTS_DIR:-$HOME/results}"

if have realpath; then
  RESULTS_BASE="$(realpath -m "$RESULTS_BASE_RAW" 2>/dev/null || true)"
else
  mkdir -p "$RESULTS_BASE_RAW" || fatal "Cannot create results base directory: $RESULTS_BASE_RAW"
  RESULTS_BASE="$(canonicalize_dir "$RESULTS_BASE_RAW")"
fi

[[ -n "${RESULTS_BASE:-}" && "$RESULTS_BASE" = /* ]] || fatal "Failed to canonicalize RESULTS_DIR to an absolute path."
mkdir -p "$RESULTS_BASE" || fatal "Cannot create results base directory: $RESULTS_BASE"

OUTDIR="${RESULTS_BASE}/${HOST}_${TS}"
mkdir -p "$OUTDIR"
SUMMARY="${OUTDIR}/summary.txt"

exec > >(tee -a "$SUMMARY") 2>&1

COPYRIGHT_LINE="Copyright (c) ELSOUL LABO B.V. and Validators DAO. All rights reserved."
echo "$COPYRIGHT_LINE"

note "Benchmark started"
echo "Output directory: $OUTDIR"
echo "Summary log:      $SUMMARY"

# -----------------------------------------------------------------------------
# STREAM availability (required by default; auto-install if missing)
# -----------------------------------------------------------------------------
note "STREAM availability"
if have stream; then
  STREAM_PRESENT=1
  STREAM_BINARY="$(command -v stream || echo "stream")"
  ensure_stream_big_arrays || fatal "STREAM is present but could not ensure large-array build."
  STREAM_INSTALL_MESSAGE="stream already present."
  if [[ "${STREAM_REBUILT}" -eq 1 ]]; then
    STREAM_INSTALL_MESSAGE="${STREAM_INSTALL_MESSAGE}; rebuilt STREAM with large arrays"
  fi
else
  STREAM_PRESENT=0
  if [[ "${INSTALL_STREAM}" -eq 1 ]]; then
    if install_stream_with_fallback; then
      STREAM_PRESENT=1
    fi
  else
    STREAM_INSTALL_MESSAGE="STREAM install disabled by config (INSTALL_STREAM=0)."
  fi
fi

echo "STREAM_STATUS=$([[ "$STREAM_PRESENT" -eq 1 ]] && echo "present" || echo "absent")"
echo "STREAM_INSTALL_ATTEMPTED=$([[ "$STREAM_INSTALL_ATTEMPTED" -eq 1 ]] && echo "yes" || echo "no")"
[[ -n "$STREAM_INSTALL_PACKAGE" ]] && echo "STREAM_INSTALL_PACKAGE=${STREAM_INSTALL_PACKAGE}"
[[ -n "$STREAM_INSTALL_MESSAGE" ]] && echo "STREAM_NOTE=${STREAM_INSTALL_MESSAGE}"
if [[ "$STREAM_PRESENT" -eq 1 && -n "$STREAM_BINARY" ]]; then
  echo "STREAM_PATH=${STREAM_BINARY}"
fi

if [[ "$STREAM_PRESENT" -eq 0 && "$ALLOW_MISSING_STREAM" -eq 1 ]]; then
  echo "STREAM_NOT_MEASURED=1 (allowed by --allow-missing-stream)"
fi
if [[ "$STREAM_PRESENT" -eq 0 && "$ALLOW_MISSING_STREAM" -ne 1 ]]; then
  fatal "STREAM binary not available; memory benchmark cannot run. ${STREAM_INSTALL_MESSAGE:-"Install failed or was skipped."} (Use --allow-missing-stream only if you explicitly accept missing memory results.)"
fi

# -----------------------------------------------------------------------------
# Sanity checks for fio dir
# -----------------------------------------------------------------------------
note "Disk target checks"
echo "Requested FIO_DIR: $FIO_DIR"
[[ "$FIO_DIR" == "/" || "$FIO_DIR" == "" ]] && fatal "Refusing to use FIO_DIR='${FIO_DIR:-<empty>}' (choose a writable directory like /var/tmp)."
ensure_dir_writable "$FIO_DIR"

FIO_DIR="$(canonicalize_dir "$FIO_DIR")"
[[ -n "$FIO_DIR" && "$FIO_DIR" = /* ]] || fatal "Failed to canonicalize FIO_DIR to an absolute path."

# IMPORTANT: keep the on-disk filename relative to --directory to avoid path interpretation issues.
FIO_FILENAME="node_bench_fio_testfile.dat"
FIO_FILE="${FIO_DIR%/}/${FIO_FILENAME}"

cleanup_stale_fio_artifacts "$FIO_DIR"
cleanup_stale_fio_file "$FIO_FILE"

NEEDED_KB=$((FIO_SIZE_GB * 1024 * 1024 + 1024 * 1024))
check_free_space "$FIO_DIR" "$NEEDED_KB"

# -----------------------------------------------------------------------------
# System info
# -----------------------------------------------------------------------------
note "Environment"
echo "UTC timestamp:          $TS"
echo "Hostname:               $HOST"
echo "Kernel:                 $(uname -a)"
VCPUS="$(getconf _NPROCESSORS_ONLN || echo 1)"
echo "vCPUs:                  $VCPUS"
echo "SYSBENCH_CPU_MAX_PRIME: $SYSBENCH_CPU_MAX_PRIME"
echo "FIO_DIR:                $FIO_DIR"
echo "FIO_SIZE_GB:            $FIO_SIZE_GB"
echo "FIO_RUNTIME_SEC:        $FIO_RUNTIME_SEC"
echo "FIO_RAMP_SEC:           $FIO_RAMP_SEC"
echo "FIO_NUMJOBS:            $FIO_NUMJOBS"
echo "FIO_IOENGINE:           $FIO_IOENGINE"
echo "Tools:"
echo "  sysbench: $(sysbench --version 2>/dev/null | head -n1 || true)"
echo "  fio:      $(fio --version 2>/dev/null || true)"
echo "  jq:       $(jq --version 2>/dev/null || echo 'n/a')"
stream_info="absent"
if [[ "$STREAM_PRESENT" -eq 1 ]]; then
  stream_info="${STREAM_BINARY:-$(command -v stream || echo "present")}"
fi
echo "  stream:   ${stream_info}"

note "Hardware / OS details"
cmd bash -lc 'echo "--- lscpu ---"; lscpu || true; echo; echo "--- free -h ---"; free -h || true; echo; echo "--- lsblk ---"; lsblk -o NAME,TYPE,SIZE,MODEL,ROTA,MOUNTPOINT,FSTYPE || true; echo; echo "--- df -hT ---"; df -hT || true'

# -----------------------------------------------------------------------------
# CPU Benchmark: sysbench
# -----------------------------------------------------------------------------
note "CPU Benchmark: sysbench cpu"
THREADS_LIST_STR="$(clamp_threads "$VCPUS")"
read -r -a THREADS_LIST <<< "$THREADS_LIST_STR"
echo "Thread sweep (clamped): ${THREADS_LIST[*]}"
echo "Each run: sysbench cpu --cpu-max-prime=${SYSBENCH_CPU_MAX_PRIME}"

for t in "${THREADS_LIST[@]}"; do
  note "sysbench cpu (threads=$t)"
  cmd sysbench cpu --threads="$t" --cpu-max-prime="$SYSBENCH_CPU_MAX_PRIME" run
done

# -----------------------------------------------------------------------------
# RAM Benchmark: STREAM
# -----------------------------------------------------------------------------
note "RAM Benchmark: STREAM"
if [[ "${STREAM_PRESENT}" -eq 1 ]]; then
  echo "Running STREAM (raw output)."
  export OMP_NUM_THREADS="${VCPUS}"
  export OMP_PROC_BIND=true
  export OMP_PLACES=cores
  cmd "${STREAM_BINARY:-stream}"
else
  echo "STREAM NOT RUN: stream binary unavailable."
  echo "Reason: ${STREAM_INSTALL_MESSAGE:-unknown}"
  echo "Proceeding without memory results because --allow-missing-stream was provided."
fi

# -----------------------------------------------------------------------------
# Disk Benchmark: fio
# -----------------------------------------------------------------------------
note "Disk Benchmark: fio"
echo "fio will create a test file:"
echo "  ${FIO_FILE}"
echo "Profiles:"
echo "  - 4K randread QD1 / QD32"
echo "  - 4K randwrite QD1 / QD32"
echo "  - 1M seqread QD16"
echo "  - 1M seqwrite QD16"
echo "  - 4K randrw 70/30 QD16"
echo "All runs are direct=1 (bypass page cache), time_based=1."
echo

FIO_COMMON_OPTS=(
  "--directory=${FIO_DIR}"
  "--filename=${FIO_FILENAME}"
  "--ioengine=${FIO_IOENGINE}"
  "--direct=1"
  "--time_based=1"
  "--runtime=${FIO_RUNTIME_SEC}"
  "--ramp_time=${FIO_RAMP_SEC}"
  "--size=${FIO_SIZE_GB}G"
  "--numjobs=${FIO_NUMJOBS}"
  "--group_reporting=1"
  "--eta=always"
)

fio_run() {
  local jobname="$1"; shift
  local json_out="${OUTDIR}/${jobname}.json"

  note "fio: ${jobname}"
  echo "Expected time: ~$(($FIO_RAMP_SEC + $FIO_RUNTIME_SEC))s (ramp + runtime), plus overhead."
  echo "JSON output:   ${json_out}"
  echo

  echo "\$ fio ${FIO_COMMON_OPTS[*]} --name=${jobname} $* --output-format=json --output=${json_out}"
  fio "${FIO_COMMON_OPTS[@]}" \
    --name="$jobname" \
    "$@" \
    --output-format=json \
    --output="$json_out"

  if have jq; then
    echo
    echo "---- extracted metrics (from JSON) ----"
    jq -r '
      .jobs[0] as $j |
      "job: \($j.jobname)\n" +
      (if $j.read then
        ("read:  iops=" + ($j.read.iops|tostring) +
         " bw(KiB/s)=" + ($j.read.bw|tostring) +
         " clat_mean(ns)=" + ($j.read.clat_ns.mean|tostring) +
         " clat_p99(ns)=" + (($j.read.clat_ns.percentile["99.000000"] // "n/a")|tostring))
       else "" end) + "\n" +
      (if $j.write then
        ("write: iops=" + ($j.write.iops|tostring) +
         " bw(KiB/s)=" + ($j.write.bw|tostring) +
         " clat_mean(ns)=" + ($j.write.clat_ns.mean|tostring) +
         " clat_p99(ns)=" + (($j.write.clat_ns.percentile["99.000000"] // "n/a")|tostring))
       else "" end)
    ' "$json_out" || true
  else
    echo
    echo "Note: jq not available; skipping JSON extraction."
  fi
}

fio_run "fio_4k_randread_qd1"      --rw=randread  --bs=4k --iodepth=1
fio_run "fio_4k_randread_qd32"     --rw=randread  --bs=4k --iodepth=32
fio_run "fio_4k_randwrite_qd1"     --rw=randwrite --bs=4k --iodepth=1
fio_run "fio_4k_randwrite_qd32"    --rw=randwrite --bs=4k --iodepth=32
fio_run "fio_1m_seqread_qd16"      --rw=read      --bs=1m --iodepth=16
fio_run "fio_1m_seqwrite_qd16"     --rw=write     --bs=1m --iodepth=16
fio_run "fio_4k_randrw_70r30_qd16" --rw=randrw --rwmixread=70 --bs=4k --iodepth=16

# -----------------------------------------------------------------------------
# Cleanup
# -----------------------------------------------------------------------------
note "Cleanup"
echo "Removing fio test file: ${FIO_FILE}"
rm -f "$FIO_FILE" || true

# -----------------------------------------------------------------------------
# Done
# -----------------------------------------------------------------------------
note "Benchmark completed"
echo "Results saved in: $OUTDIR"
echo "  - Summary:  $SUMMARY"
echo "  - fio JSON: $OUTDIR/fio_*.json"
echo "$COPYRIGHT_LINE"
