# FPGA Synthesis for Alchitry Cu v1

This directory contains files for synthesizing the RISC-V CPU to the Alchitry Cu v1 board (iCE40-HX8K-CB132) using open-source tools.

## Quick Start

```bash
# Install prerequisites - Build Yosys from source (REQUIRED)
# Yosys 0.33 from apt has known issues. Version 0.61+ from source is required.
cd /tmp
git clone https://github.com/YosysHQ/yosys
cd yosys
git submodule update --init
sudo apt-get install -y build-essential clang bison flex libreadline-dev \
    gawk tcl-dev libffi-dev git graphviz xdot pkg-config python3 \
    libboost-system-dev libboost-python-dev libboost-filesystem-dev \
    zlib1g-dev libfl-dev
make config-gcc && make -j$(nproc) && sudo make install

# Install nextpnr and icestorm
sudo apt-get install -y fpga-icestorm nextpnr-ice40

# Synthesize design
cd /path/to/repo/fpga
make

# Program FPGA (if hardware is connected)
sudo make program
```

## Files

- **`fpga_top.sv`**: Top-level FPGA wrapper module
- **`bram_imem.sv`**: Block RAM instruction memory (4 KB)
- **`bram_dmem.sv`**: Block RAM data memory (4 KB)
- **`ice40hx8k.pcf`**: Pin constraint file for Alchitry Cu v1 board
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

- **Board**: Alchitry Cu v1 (Lattice iCE40-HX8K-CB132)
- **Resources Used**: ~6,500 LUTs, ~2,500 FFs, 2 BRAMs (estimated)
- **Clock**: 100 MHz (from on-board oscillator)
- **Peripherals**: 8 LEDs on main board

## Known Limitations

**FPU Synthesis**: Even with Yosys 0.61+, the FPU module has partial SystemVerilog compatibility issues. Work is in progress to resolve these. The base RV32IMAC (without F extension) synthesizes successfully.

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
