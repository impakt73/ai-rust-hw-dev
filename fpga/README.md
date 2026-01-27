# FPGA Synthesis Files

This directory contains files for synthesizing the RISC-V CPU to an iCE40-HX8K FPGA using open-source tools.

## Quick Start

```bash
# Install prerequisites (Ubuntu/Debian)
sudo apt-get install -y yosys fpga-icestorm nextpnr-ice40

# Synthesize design
make

# Program FPGA (if hardware is connected)
sudo make program
```

## Files

- **`fpga_top.sv`**: Top-level FPGA wrapper module
- **`bram_imem.sv`**: Block RAM instruction memory (4 KB)
- **`bram_dmem.sv`**: Block RAM data memory (4 KB)
- **`ice40hx8k.pcf`**: Pin constraint file for HX8K-Breakout board
- **`Makefile`**: Build automation for synthesis workflow
- **`build/`**: Generated build artifacts (created during synthesis)

## Documentation

See **[docs/fpga-synthesis.md](../docs/fpga-synthesis.md)** for:
- Detailed installation instructions
- Step-by-step synthesis workflow
- Customization guide
- Troubleshooting tips
- Resource utilization information

## Makefile Targets

- `make` or `make all` - Full synthesis flow (JSON → ASC → BIN)
- `make timing` - Generate timing analysis report
- `make utilization` - Show resource utilization
- `make program` - Program connected FPGA board
- `make clean` - Remove build artifacts
- `make check-tools` - Verify required tools are installed
- `make help` - Show all available targets

## Hardware Requirements

- **Board**: Lattice iCE40-HX8K Breakout (HX8K-B-EVN)
- **Resources Used**: ~6,500 LUTs, ~2,500 FFs, 2 BRAMs
- **Clock**: 12 MHz (from on-board oscillator)
- **Peripherals**: 8 LEDs

## Default Test Program

The instruction memory includes a simple LED test program:

```assembly
lui  x15, 0x50000      # Load LED base address (0x50000000)
addi x14, x0, 0xAA     # Load pattern 0xAA
sw   x14, 0(x15)       # Write to LED register
loop:
    addi x13, x0, 0    # NOP
    j    loop          # Loop forever
```

This will display pattern `0xAA` (binary 10101010) on the 8 LEDs.

To change the program, edit the `initial` block in `bram_imem.sv`.
