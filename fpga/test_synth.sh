#!/bin/bash
# Quick synthesis test script
# This script performs a basic synthesis check without place-and-route
# Useful for quickly verifying that the RTL is synthesizable

set -e  # Exit on error

echo "==================================="
echo "RISC-V FPGA Synthesis Test"
echo "==================================="
echo ""

# Check for required tools
echo "Checking for required tools..."
if ! command -v yosys &> /dev/null; then
    echo "ERROR: yosys not found. Please install it first."
    echo "  Ubuntu/Debian: sudo apt-get install yosys"
    exit 1
fi
echo "✓ yosys found: $(yosys --version | head -1)"

echo ""
echo "Running synthesis test..."
echo ""

# Create temporary directory for test
TEST_DIR=$(mktemp -d)
echo "Using temporary directory: $TEST_DIR"

# Run Yosys synthesis
yosys -p "
    read_verilog -sv ../rtl/alu.sv;
    read_verilog -sv ../rtl/branch_unit.sv;
    read_verilog -sv ../rtl/csr_file.sv;
    read_verilog -sv ../rtl/decoder.sv;
    read_verilog -sv ../rtl/decompress.sv;
    read_verilog -sv ../rtl/div_unit.sv;
    read_verilog -sv ../rtl/fetch_buffer.sv;
    read_verilog -sv ../rtl/fp_regfile.sv;
    read_verilog -sv ../rtl/fpu.sv;
    read_verilog -sv ../rtl/mem_interface.sv;
    read_verilog -sv ../rtl/regfile.sv;
    read_verilog -sv ../rtl/writeback_mux.sv;
    read_verilog -sv ../rtl/top.sv;
    read_verilog -sv ../rtl/peripherals/led_controller.sv;
    read_verilog -sv ../rtl/top_with_peripherals.sv;
    read_verilog -sv bram_imem.sv;
    read_verilog -sv bram_dmem.sv;
    read_verilog -sv fpga_top.sv;
    hierarchy -check -top fpga_top;
    synth_ice40 -top fpga_top -json $TEST_DIR/test.json;
" 2>&1 | tee $TEST_DIR/synth.log

# Check if synthesis succeeded
if [ ${PIPESTATUS[0]} -eq 0 ]; then
    echo ""
    echo "==================================="
    echo "✓ Synthesis test PASSED"
    echo "==================================="
    echo ""
    echo "Resource utilization:"
    grep -A 20 "Printing statistics" $TEST_DIR/synth.log | grep -E "^\s*(Number|SB_)" || true
    echo ""
    echo "Log file: $TEST_DIR/synth.log"
    echo "JSON file: $TEST_DIR/test.json"
    echo ""
    echo "To clean up: rm -rf $TEST_DIR"
    exit 0
else
    echo ""
    echo "==================================="
    echo "✗ Synthesis test FAILED"
    echo "==================================="
    echo "Check the log file for errors: $TEST_DIR/synth.log"
    exit 1
fi
