# FPGA Synthesis for Alchitry Cu v1

This directory contains files for synthesizing the RISC-V CPU to FPGA targets using open-source tools for the Lattice boards and a Vivado batch flow for the Alchitry Au target.

Currently supported targets:
- **`TARGET=ice40_alchitry_cu`** (default): Alchitry Cu v1 (iCE40-HX8K-CB132)
- **`TARGET=ecp5_icepi_zero`**: iCE Pi Zero (ECP5-25F)
- **`TARGET=artix7_alchitry_au`**: Alchitry Au (Artix-7 XC7A35T-FTG256-1)

## Status: ✅ Successfully Synthesized

The FPGA design is configured to run on the iCE40-HX8K within resource constraints:

- **Extensions**: M (multiply/divide) disabled by default; F (floating-point) disabled
- **ISA supported**: RV32I base instruction set + C (compressed) + A (atomic) + Zicsr
- **Resource usage**: 4,399 SB_LUT4s and 7,306 total mapped cells after disabling M by default
- **Clock frequency**: 25 MHz (via PLL), latest build achieves 39.50 MHz max
- **Communication**: CPU communicates with host over USB serial (UART) using the host bus protocol
- **External memory**: DRAM accesses are forwarded to the host computer over UART

## What's Included

The FPGA implementation includes:

- ✅ **RISC-V RV32IAC CPU**: Base integer + Atomic + Compressed instruction sets (M and F disabled by default on iCE40)
- ✅ **LED Controller Peripheral**: 8-bit LED output mapped at 0x50000000
- ✅ **Clock Peripheral**: Elapsed time counters (us/ms/s) mapped at 0x60000000
- ✅ **SRAM Peripheral**: 12KB on-chip SRAM mapped at 0x70000000
- ✅ **System Controller**: CPU boot and reset control mapped at 0x20000000
- ✅ **UART Host Interface**: USB serial communication for host-initiated and CPU-initiated bus requests
- ✅ **PLL Clock Generation**: 100 MHz input → 25 MHz system clock for timing closure

## Quick Start

### Install Tools (Option 1: Package Manager - Recommended for Quick Testing)

```bash
# Update package list
sudo apt-get update

# Install all tools
sudo apt-get install -y yosys fpga-icestorm nextpnr-ice40

# Verify installation
cd fpga
make check-tools
```

### Install Tools (Option 2: Build from Source - Latest Features)

If you need the latest version or package manager version doesn't work:

```bash
# Install dependencies
sudo apt-get install -y build-essential clang bison flex \
    libreadline-dev gawk tcl-dev libffi-dev git \
    graphviz xdot pkg-config python3 python3-dev \
    libboost-all-dev cmake

# Install IceStorm (bitstream tools)
git clone https://github.com/YosysHQ/icestorm.git
cd icestorm && make -j$(nproc) && sudo make install && cd ..

# Install Yosys (synthesis)
git clone https://github.com/YosysHQ/yosys.git
cd yosys && make -j$(nproc) && sudo make install && cd ..

# Install nextpnr (place and route)
git clone https://github.com/YosysHQ/nextpnr.git
cd nextpnr
cmake -DARCH=ice40 -DCMAKE_INSTALL_PREFIX=/usr/local .
make -j$(nproc) && sudo make install && cd ..
```

### Synthesize the Design

```bash
# Navigate to fpga directory
cd fpga

# Run full synthesis flow for default target (iCE40)
make

# Run full synthesis flow for ECP5 iCE Pi Zero
make TARGET=ecp5_icepi_zero

# Run full synthesis flow for Artix-7 Alchitry Au
# (requires Xilinx Vivado in PATH; the proprietary flow is driven by TCL)
make TARGET=artix7_alchitry_au

# This generates:
# - build/<target>/riscv_fpga.*  (target-specific synthesis outputs)
```

### Generate Standardized FPGA Design Stats

Use the standardized stats workflow whenever you need concise, machine-friendly
resource utilization and max-frequency information for a supported FPGA target.
The stats tooling requires **Python 3.10 or newer**.

```bash
# From rtl/fpga/
make TARGET=ice40_alchitry_cu stats STATS_FORMAT=text
make TARGET=ecp5_icepi_zero stats STATS_FORMAT=json
make TARGET=artix7_alchitry_au stats STATS_FORMAT=markdown
```

This workflow:

1. Runs the normal synthesis/place-and-route flow for the selected target
2. Extracts the authoritative max-frequency result for that target
3. Extracts post-route resource utilization plus post-synthesis cell counts
4. Writes normalized artifacts to `build/<target>/`

Generated files:

- `build/<target>/riscv_fpga_stats.json`
- `build/<target>/riscv_fpga_stats.md`

If you already have up-to-date build artifacts and only need to reformat them:

```bash
python3 fpga_design_stats.py --target ice40_alchitry_cu --format json
```

The script rejects `--build-dir` together with `--build` because the build flow
always writes to the target's standard `build/<target>/` output directory.

### Timing / Utilization Sources Used by the Stats Workflow

- **`ice40_alchitry_cu`**
  - Routed Fmax: `build/ice40_alchitry_cu/nextpnr.log`
  - Resource utilization: `build/ice40_alchitry_cu/nextpnr.log`
  - Synthesis cell counts: `build/ice40_alchitry_cu/yosys.log`

- **`ecp5_icepi_zero`**
  - Routed Fmax: `build/ecp5_icepi_zero/nextpnr.log`
  - Resource utilization: `build/ecp5_icepi_zero/nextpnr.log`
  - Synthesis cell counts: `build/ecp5_icepi_zero/yosys.log`

- **`artix7_alchitry_au`**
  - Timing summary: `build/artix7_alchitry_au/riscv_fpga_timing.rpt`
  - Utilization report: `build/artix7_alchitry_au/riscv_fpga_utilization.rpt`

### Program FPGA (Requires Hardware)

```bash
# Connect Alchitry Cu v1 board via USB
sudo make program

# Use SRAM explicitly if you want a volatile load instead of the default flash programming:
sudo make program PROGRAM_MODE=sram

# Or manually using openFPGALoader:
sudo openFPGALoader -b ice40_generic -m build/ice40_alchitry_cu/riscv_fpga.bin  # SRAM
sudo openFPGALoader -b ice40_generic -f build/ice40_alchitry_cu/riscv_fpga.bin  # Flash
```

## Files

- **`common/fpga_common_top.sv`**: Shared FPGA top logic (CPU + host UART + reset-button synchronization)
- **`ice40_alchitry_cu/ice40_alchitry_cu_top.sv`**: iCE40 top-level FPGA wrapper module
- **`ice40_alchitry_cu/ice40_alchitry_cu.pcf`**: iCE40 pin constraint file for Alchitry Cu v1 board
- **`ice40_alchitry_cu/timing_async.sdc`**: iCE40 asynchronous external I/O timing exceptions
- **`ecp5_icepi_zero/ecp5_icepi_zero_top.sv`**: ECP5 top-level FPGA wrapper for iCE Pi Zero
- **`../common/fpu/*.sv`**: Shared floating-point unit implementation sources (always included for synthesis; `ENABLE_F_EXT` remains disabled by default)
- **`ecp5_icepi_zero/ecp5_icepi_zero.lpf`**: ECP5 LPF constraint file for iCE Pi Zero
- **`ecp5_icepi_zero/timing_async.sdc`**: ECP5 asynchronous external I/O timing exceptions
- **`artix7_alchitry_au/artix7_alchitry_au_top.sv`**: Artix-7 top-level FPGA wrapper for Alchitry Au
- **`artix7_alchitry_au/alchitry_au.xdc`**: Artix-7 XDC constraint file for Alchitry Au
- **`Makefile`**: Build automation for synthesis workflow
- **`build/`**: Generated build artifacts (created during synthesis)

## Makefile Targets

- `make` or `make all` - Full synthesis flow (target-specific bitstream generation)
- `make timing` - Generate timing analysis report
- `make utilization` - Show resource utilization
- `make program` - Program connected FPGA board (default: flash on Alchitry Cu, SRAM on other targets)
- `make clean` - Remove build artifacts
- `make check-tools` - Verify required tools are installed
- `make help` - Show all available targets
- `make program PROGRAM_MODE=flash` - Program the persistent configuration flash
- `make program PROGRAM_MODE=sram` - Program SRAM explicitly for a volatile load

## Hardware Requirements

- **Board**: Alchitry Cu v1 (Lattice iCE40-HX8K-CB132)
- **Resources Used**: 4,399 SB_LUT4s, 7,306 total mapped cells, 30 BRAMs
- **Clock**: 100 MHz input → 25 MHz system clock (via PLL)
- **Peripherals**: 8 LEDs on main board
- **Programming**: USB cable for openFPGALoader (the Alchitry Cu works with `-b ice40_generic`)

## Artix-7 (Alchitry Au) Toolchain Notes

The Artix-7 target now uses the proprietary Vivado CLI flow in batch mode. The flow is encapsulated in `artix7_alchitry_au/vivado_build.tcl`, which reads all RTL/XDC inputs, runs synthesis/place/route, and emits the bitstream plus reports into `build/artix7_alchitry_au/`.

The Yosys-based targets keep board pin constraints in PCF/LPF files and store asynchronous external-I/O timing exceptions in target-specific `.sdc` files. The Makefile now feeds those SDC files into the nextpnr phase when the installed nextpnr build supports `--sdc`; otherwise the files remain checked-in timing-intent artifacts alongside the existing open-source flow.

By default the Makefile expects `vivado` to be available in your `PATH`. If needed, override the executable path:

```bash
make TARGET=artix7_alchitry_au \
  VIVADO=/opt/Xilinx/Vivado/2025.1/bin/vivado
```

## Pin Assignments (Alchitry Cu v1)

| Signal | Pin | Description |
|--------|-----|-------------|
| clk | P7 | 100 MHz on-board oscillator |
| rst_n | P8 | Active-low reset button |
| led[0] | J11 | LED 0 (LSB) |
| led[1] | K11 | LED 1 |
| led[2] | K12 | LED 2 |
| led[3] | K14 | LED 3 |
| led[4] | L12 | LED 4 |
| led[5] | L14 | LED 5 |
| led[6] | M12 | LED 6 |
| led[7] | N14 | LED 7 (MSB) |

Reference: [Alchitry Cu PCF](https://github.com/r1cebank/alchitry-cu-utils/blob/main/alchitry_cu.pcf)

## Default Test Program

The `led_demo/` subdirectory contains a standalone LED rotation demo (`led_demo/led_pattern_top.sv`) that displays an alternating pattern on the 8 LEDs.

The main iCE40 FPGA design (`ice40_alchitry_cu/ice40_alchitry_cu_top.sv`) does not pre-load a fixed test program. Instead, programs are loaded at runtime by the host computer via the UART host bus interface. Use `fpga-host` or `sim-view --runtime fpga` to load and run RISC-V ELF programs on the FPGA.

**Example LED pattern (pseudo-assembly):**

```assembly
# Initialize
lui  x10, 0x50000      # LED controller base (0x50000000)
addi x11, x0, 0xAA     # Initial pattern 0xAA (10101010)
lui  x13, 0x017D8      # Delay count upper bits
addi x13, x13, -1984   # Delay = 25,000,000 cycles (1 second at 25 MHz)

# Main loop
sw   x11, 0(x10)       # Write pattern to LED
addi x12, x0, 0        # counter = 0

# Delay loop (count to 25M)
delay:
    addi x12, x12, 1   # counter++
    bne  x12, x13, delay

# Rotate pattern left by 1 bit
slli x14, x11, 1       # Shift left
srli x15, x11, 7       # Extract MSB
or   x11, x14, x15     # Combine
andi x11, x11, 0xFF    # Mask to 8 bits
jal  x0, main_loop     # Repeat
```

**Expected behavior:**
- Pattern alternates: 0xAA (10101010) ↔ 0x55 (01010101)
- Updates every 1 second (25M cycles at 25 MHz)

## Troubleshooting

### "Timing not met" error

The design uses a PLL to generate 25 MHz from the 100 MHz input clock, which ensures timing closure. If you need a different frequency, update:

1. **PLL parameters**: Edit `ice40_alchitry_cu/ice40_alchitry_cu_top.sv` PLL configuration (DIVR, DIVF, DIVQ)
2. **Makefile**: Change `--freq 25` to match your target frequency
3. **Test program**: Update delay loop count in your program to match the new cycle count

### "Insufficient resources" error

The design uses ~74% of HX8K logic resources. If synthesis fails:

1. **Already optimized**: M and F extensions are disabled by default on the iCE40 target
2. **Reduce BRAM usage**: Minimize the on-chip SRAM allocation if possible

### "make program" fails

Check USB connection and permissions:

```bash
# List USB devices (should show FTDI device)
lsusb | grep 0403:6010

# Add user to dialout group for USB access
sudo usermod -a -G dialout $USER
# Log out and back in for changes to take effect
```

For `TARGET=ice40_alchitry_cu`, `make program` now programs flash by default because SRAM programming has been unreliable on the Alchitry Cu. Use `make program PROGRAM_MODE=sram` if you want a volatile SRAM load instead. Other targets still default to SRAM programming.

### Simulation works but FPGA doesn't

Common issues:

1. **Clock/Reset**: Verify reset button (P8) is not stuck
2. **UART connection**: Ensure the host UART interface is connected and the host software is running
3. **Timing violations**: Run `make timing` to check for timing errors
4. **Pin mismatch**: Verify PCF matches your board

## Customization

### Running a Custom Program

The FPGA design loads programs at runtime via the host UART interface. To run your own program:

1. Build a RISC-V ELF targeting the CPU's memory map (SRAM at 0x70000000, DRAM forwarded to host)
2. Use `sim-view --runtime fpga --fpga-device /dev/ttyUSB0` to load and run the ELF
3. Or use the `fpga-host` crate directly for programmatic control

### Changing Clock Frequency

To modify the system clock frequency:

1. Update PLL parameters in `ice40_alchitry_cu/ice40_alchitry_cu_top.sv` (DIVR, DIVF, DIVQ)
2. Change `--freq` in `Makefile` to match your target frequency
3. Re-run synthesis and check timing

## Next Steps

- **Optimize Timing**: Add pipeline stages to run at higher clock speeds
- **Enable F Extension**: Implement FPGA-friendly floating-point or increase target device capacity
- **IO Shield Integration**: Leverage DIP switches, buttons, and segment display for richer demos

## References

- [Yosys Documentation](https://yosyshq.net/yosys/)
- [nextpnr Documentation](https://github.com/YosysHQ/nextpnr)
- [IceStorm Project](https://github.com/YosysHQ/icestorm)
- [iCE40 Family Handbook](https://www.latticesemi.com/ice40)
- [Alchitry Cu Documentation](https://alchitry.com/cu/)
- [RISC-V Specifications](https://riscv.org/specifications/)
