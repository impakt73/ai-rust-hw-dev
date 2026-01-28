# LED Pattern Demo for Alchitry Cu v1

This is a minimal LED pattern demo that synthesizes to the Alchitry Cu v1 board (iCE40-HX8K-CB132) using open-source tools.

## Description

The design displays an alternating pattern (0xAA = 10101010) on the 8-bit LED output that shifts left by one position every second. This creates a visually appealing rotating animation effect.

### Features

- **Clock**: 100 MHz (on-board oscillator)
- **Reset**: Active-low reset button (P8)
- **LED Pattern**: Initial pattern 0xAA (alternating bits)
- **Shift Rate**: 1 position per second (left rotation)

## Resource Utilization Report

Synthesis completed with **Yosys 0.33** for iCE40-HX8K target.

### iCE40-HX8K Resources Used

| Resource | Used | Available | Utilization |
|----------|------|-----------|-------------|
| SB_LUT4 (Logic Cells) | 64 | 7,680 | 0.83% |
| SB_DFF (all variants) | 37 | 7,680 | 0.48% |
| SB_CARRY (Carry Logic) | 51 | 7,680 | 0.66% |

### Detailed Cell Usage

| Cell Type | Count | Description |
|-----------|-------|-------------|
| SB_LUT4 | 64 | 4-input Look-Up Tables |
| SB_CARRY | 51 | Carry chain cells (for counter) |
| SB_DFF | 2 | Basic D Flip-Flops |
| SB_DFFER | 4 | D Flip-Flops with Enable & Reset |
| SB_DFFES | 4 | D Flip-Flops with Enable & Set |
| SB_DFFR | 27 | D Flip-Flops with Reset |

### Summary

- **Total Cells**: 152
- **Total Wires**: 18 (197 bits)
- **Memory Blocks (BRAM)**: 0
- **Design Complexity**: Minimal (~1% of available resources)

This minimal design demonstrates that the synthesis toolchain is working correctly and leaves ~99% of FPGA resources available for additional logic.

## Quick Start

### Prerequisites

Install the open-source FPGA toolchain:

```bash
# Install yosys for synthesis
sudo apt-get update && sudo apt-get install -y yosys

# (Optional) Install full toolchain for bitstream generation
sudo apt-get install -y fpga-icestorm nextpnr-ice40
```

### Build

```bash
# Run synthesis only (default)
cd fpga/led_demo
make synth

# View resource utilization
make utilization

# Full build with bitstream (requires nextpnr and icestorm)
make bitstream
```

## Files

| File | Description |
|------|-------------|
| `led_pattern_top.sv` | Top-level SystemVerilog module |
| `alchitry_cu.pcf` | Pin constraint file for Alchitry Cu v1 |
| `Makefile` | Build automation for synthesis |
| `build/` | Generated build artifacts (created during synthesis) |

## Pin Assignments

| Signal | Pin | Description |
|--------|-----|-------------|
| clk | P7 | 100 MHz on-board oscillator |
| rst_n_btn | P8 | Active-low reset button |
| led[0] | J11 | LED 0 (LSB) |
| led[1] | K11 | LED 1 |
| led[2] | K12 | LED 2 |
| led[3] | K14 | LED 3 |
| led[4] | L12 | LED 4 |
| led[5] | L14 | LED 5 |
| led[6] | M12 | LED 6 |
| led[7] | N14 | LED 7 (MSB) |

## Design Details

### Architecture

The design consists of three main components:

1. **Reset Synchronizer**: 2-FF synchronizer for metastability protection on the async reset input
2. **Second Counter**: 27-bit counter that counts to 100,000,000 (1 second at 100 MHz)
3. **LED Pattern Register**: 8-bit register that rotates left on each second tick

### Timing

- Counter width: 27 bits (sufficient for 100M count)
- Shift pulse: Generated when counter reaches SHIFT_COUNT - 1
- Pattern rotation: MSB wraps to LSB position

### Expected LED Behavior

| Time (sec) | LED Pattern | Binary |
|------------|-------------|--------|
| 0 | 0xAA | 10101010 |
| 1 | 0x55 | 01010101 |
| 2 | 0xAA | 10101010 |
| 3 | 0x55 | 01010101 |
| ... | ... | ... |

The pattern creates a visually pleasing alternating animation between 0xAA and 0x55.
