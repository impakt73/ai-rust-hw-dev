#!/usr/bin/env bash
#
# Run the checks from .github/workflows/ci.yml locally.
#
# This script assumes the required system packages and Rust target are already
# installed. It does not install apt packages or use the GitHub Actions caches.
#
# Usage:
#   ./scripts/run-ci-local.sh [--skip-fpga]
#
# Options:
#   --skip-fpga   Skip the FPGA synthesis and bitstream checks.
#   -h, --help    Show this help text.

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

usage() {
    sed -n '2,13p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

SKIP_FPGA=0
for arg in "$@"; do
    case "$arg" in
        --skip-fpga)
            SKIP_FPGA=1
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "error: unknown option: $arg" >&2
            usage >&2
            exit 2
            ;;
    esac
done

step() {
    printf '\n==> %s\n' "$*"
}

require_commands() {
    local missing=0
    local command_name

    for command_name in "$@"; do
        if ! command -v "$command_name" >/dev/null 2>&1; then
            echo "error: required command not found: $command_name" >&2
            missing=1
        fi
    done

    if ((missing)); then
        echo "Install the missing prerequisites and retry." >&2
        exit 1
    fi
}

required_commands=(
    cargo
    rustup
    verilator
    find
    make
    python3
)

if ((SKIP_FPGA == 0)); then
    required_commands+=(
        yosys
        nextpnr-ecp5
        ecppack
        openFPGALoader
    )
fi

require_commands "${required_commands[@]}"

step "Verify Verilator"
verilator --version

step "Run RTL lint"
find rtl/common -name '*.sv' -exec verilator --lint-only -Wno-MULTITOP {} +

if ((SKIP_FPGA == 0)); then
    step "Verify FPGA tools"
    yosys --version
    nextpnr-ecp5 --version
fi

step "Install RISC-V Rust target"
rustup target add riscv32imafc-unknown-none-elf
rustup target list --installed | grep -qx riscv32imafc-unknown-none-elf

step "Check formatting"
cargo fmt -- --check

step "Run clippy"
cargo clippy -- -D warnings

step "Check rust-test-program formatting"
(
    cd rust-test-program
    cargo fmt -- --check
)

step "Run clippy on rust-test-program"
(
    cd rust-test-program
    cargo clippy --release -- -D warnings
)

step "Build"
cargo build --verbose

step "Run tests"
cargo test --verbose

step "Build rust-test-program"
(
    cd rust-test-program
    cargo build --release --verbose
)

if ((SKIP_FPGA == 0)); then
    step "Verify default FPGA synthesis"
    (
        cd rtl/fpga
        make
    )

    step "Verify default FPGA bitstream"
    test -f rtl/fpga/build/ecp5_icepi_zero/riscv_fpga.bit
fi

step "CI-equivalent workflow completed successfully"
