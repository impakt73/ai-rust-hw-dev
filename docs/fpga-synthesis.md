# FPGA Synthesis Guide

This guide explains how to synthesize the RISC-V RV32IMACF CPU for the **Lattice iCE40-HX8K** FPGA using open-source tools.

## Table of Contents

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Detailed Workflow](#detailed-workflow)
- [Resource Utilization](#resource-utilization)
- [Customization](#customization)
- [Troubleshooting](#troubleshooting)
- [Next Steps](#next-steps)

## Overview

The FPGA synthesis flow converts the SystemVerilog RTL into a bitstream that can be loaded onto real FPGA hardware. This implementation uses:

- **Yosys**: Open-source synthesis tool for converting RTL to a netlist
- **nextpnr**: Open-source place-and-route tool for FPGA implementation
- **IceStorm**: Open-source toolchain for Lattice iCE40 FPGAs
- **Target Device**: Lattice iCE40-HX8K (8K LUTs, CT256 package)

### What's Included

The FPGA implementation includes:

- ✅ **Full RISC-V RV32IMACF CPU**: All 118 instructions
- ✅ **4 KB Instruction Memory**: On-chip block RAM (BRAM)
- ✅ **4 KB Data Memory**: On-chip block RAM (BRAM)
- ✅ **LED Controller Peripheral**: 8-bit LED output mapped at 0x50000000
- ✅ **Clock & Reset**: 12 MHz clock with synchronized reset
- ✅ **Test Program**: Simple LED pattern program pre-loaded in instruction memory

### What's NOT Included (Future Work)

- ❌ **UART/Serial**: No serial communication yet
- ❌ **Large Memory**: Limited to 8 KB total (4 KB instruction + 4 KB data)
- ❌ **Floating-Point Unit**: FPU is included in RTL but may not fit in HX8K
- ❌ **High Clock Frequencies**: Currently runs at 12 MHz (PLL for higher speeds is future work)

## Prerequisites

### Required Tools

You need to install the following open-source tools:

#### Option 1: Install from Package Manager (Ubuntu/Debian)

```bash
# Update package list
sudo apt-get update

# Install Yosys
sudo apt-get install -y yosys

# Install nextpnr and icestorm tools
sudo apt-get install -y fpga-icestorm nextpnr-ice40
```

#### Option 2: Build from Source

If your distribution doesn't have packages, or you want the latest version:

```bash
# Install dependencies
sudo apt-get install -y build-essential clang bison flex \
    libreadline-dev gawk tcl-dev libffi-dev git \
    graphviz xdot pkg-config python3 python3-dev \
    libboost-all-dev cmake

# Install IceStorm (bitstream tools)
git clone https://github.com/YosysHQ/icestorm.git
cd icestorm
make -j$(nproc)
sudo make install
cd ..

# Install Yosys (synthesis)
git clone https://github.com/YosysHQ/yosys.git
cd yosys
make -j$(nproc)
sudo make install
cd ..

# Install nextpnr (place and route)
git clone https://github.com/YosysHQ/nextpnr.git
cd nextpnr
cmake -DARCH=ice40 -DCMAKE_INSTALL_PREFIX=/usr/local .
make -j$(nproc)
sudo make install
cd ..
```

### Verify Installation

Check that all tools are installed correctly:

```bash
cd fpga
make check-tools
```

You should see: `All required tools found!`

### Hardware Requirements

To actually program the FPGA, you need:

- **iCE40-HX8K Breakout Board** (Lattice HX8K-B-EVN or compatible)
- **USB Cable** for programming (usually USB Mini-B)
- **Linux system** with USB access (or Windows with FTDI drivers)

**Note:** You can run synthesis and place-and-route **without** the physical board. The tools will generate a bitstream that you can program later.

## Quick Start

### Synthesize the Design

```bash
# Navigate to fpga directory
cd fpga

# Run full synthesis flow (Yosys + nextpnr + icepack)
make

# This generates:
# - build/riscv_fpga.json  (synthesis output)
# - build/riscv_fpga.asc   (place-and-route output)
# - build/riscv_fpga.bin   (bitstream for programming)
```

The first synthesis run takes **2-5 minutes** depending on your machine. Subsequent runs are faster.

### View Timing Report

```bash
make timing
cat build/riscv_fpga_timing.rpt
```

This shows the maximum clock frequency the design can run at.

### View Resource Utilization

```bash
make utilization
```

This shows how many LUTs, FFs, and BRAMs are used.

### Program the FPGA

**If you have the physical board connected:**

```bash
# Make sure board is connected via USB
# You may need sudo permissions for USB access
sudo make program
```

The LED pattern should start blinking on the board after programming.

## Detailed Workflow

### Step 1: Synthesis (Yosys)

Yosys converts the SystemVerilog RTL into a technology-mapped netlist:

```bash
make build/riscv_fpga.json
```

This reads all `.sv` files and generates a JSON netlist optimized for iCE40 primitives (LUTs, FFs, BRAMs, etc.).

**Output:** `build/riscv_fpga.json` + `build/yosys.log`

### Step 2: Place and Route (nextpnr)

nextpnr maps the netlist to physical FPGA resources and routes connections:

```bash
make build/riscv_fpga.asc
```

This performs:
- **Placement**: Assigns logic to specific LUT/FF locations
- **Routing**: Connects placed elements using FPGA routing resources
- **Timing analysis**: Checks if design meets timing constraints

**Output:** `build/riscv_fpga.asc` + `build/nextpnr.log`

### Step 3: Bitstream Generation (icepack)

icepack converts the ASCII configuration file to a binary bitstream:

```bash
make build/riscv_fpga.bin
```

**Output:** `build/riscv_fpga.bin` (ready to program)

### Step 4: Programming (iceprog)

Load the bitstream onto the FPGA:

```bash
sudo make program
```

**Note:** You may need to set up udev rules to avoid needing `sudo`. See [Troubleshooting](#troubleshooting).

## Resource Utilization

### Expected Resource Usage

The RISC-V CPU with full RV32IMACF support is **very large**. Here's what to expect:

| Resource | HX8K Available | Estimated Usage | Percentage |
|----------|----------------|-----------------|------------|
| LUTs     | 7,680          | ~6,500          | ~85%       |
| FFs      | 7,680          | ~2,500          | ~33%       |
| BRAMs    | 32 (4 KB each) | 2               | ~6%        |
| PLLs     | 2              | 0               | 0%         |

**Warning:** The full RV32IMACF design (including FPU and M-extension) is close to the resource limits of the HX8K. If synthesis fails due to resource constraints, see [Customization](#customization) for how to reduce the design size.

### Reducing Resource Usage

If you run out of resources, you can:

1. **Disable the Floating-Point Unit (FPU)**
   - The FPU is the largest module (~2000 LUTs)
   - Edit `fpga_top.sv` to instantiate `top.sv` instead of `top_with_peripherals.sv`
   - Remove FPU-related RTL files from `Makefile`

2. **Reduce Memory Size**
   - Change `ADDR_WIDTH` parameter in `bram_imem.sv` and `bram_dmem.sv`
   - Smaller memory = fewer BRAMs used

3. **Use a Larger FPGA**
   - Consider upgrading to iCE40-UP5K (5K LUTs) or ECP5 (45K+ LUTs)

## Customization

### Changing the Test Program

The default test program (LED blink) is hardcoded in `fpga/bram_imem.sv`.

To load your own program:

1. **Write a RISC-V assembly program**:

```assembly
# example.S
.section .text
.globl _start

_start:
    lui  x15, 0x50000      # LED base address
    addi x14, x0, 0xFF     # Pattern 0xFF (all LEDs on)
    sw   x14, 0(x15)       # Write to LED
loop:
    addi x13, x0, 0        # NOP
    j    loop              # Loop forever
```

2. **Compile to machine code**:

```bash
riscv32-unknown-elf-as -march=rv32i -mabi=ilp32 -o example.o example.S
riscv32-unknown-elf-objcopy -O binary example.o example.bin
hexdump -v -e '1/4 "mem[%d] = 32'\''h%08X;\n"' example.bin > program.hex
```

3. **Replace the `initial` block in `bram_imem.sv`** with the generated hex values

4. **Re-synthesize**:

```bash
make clean
make
```

### Changing Memory Sizes

Edit the `ADDR_WIDTH` parameter in the BRAM modules:

```systemverilog
// In fpga_top.sv
bram_imem #(.ADDR_WIDTH(11)) imem (...);  // 2^11 = 2048 words = 8 KB
bram_dmem #(.ADDR_WIDTH(11)) dmem (...);  // 2^11 = 2048 words = 8 KB
```

Remember to update the address masking in `fpga_top.sv`:

```systemverilog
.addr(imem_addr[12:2]),  // For 11-bit ADDR_WIDTH: bits [12:2]
```

### Targeting a Different FPGA

To target a different iCE40 device (e.g., iCE40-UP5K):

1. Edit `fpga/Makefile`:

```makefile
DEVICE = up5k
PACKAGE = sg48  # or your package type
```

2. Update `fpga/ice40hx8k.pcf` with the new pin assignments for your board

3. Re-run synthesis:

```bash
make clean
make
```

## Troubleshooting

### "ERROR: yosys not found"

**Solution:** Install Yosys using the instructions in [Prerequisites](#prerequisites).

### "ERROR: Can't claim USB device"

**Solution:** Set up udev rules to allow non-root access to FTDI devices:

```bash
# Create udev rule file
sudo tee /etc/udev/rules.d/53-lattice-ftdi.rules > /dev/null <<EOF
ATTRS{idVendor}=="0403", ATTRS{idProduct}=="6010", MODE="0660", GROUP="plugdev", TAG+="uaccess"
EOF

# Reload udev rules
sudo udevadm control --reload-rules
sudo udevadm trigger

# Add yourself to plugdev group
sudo usermod -a -G plugdev $USER

# Log out and log back in for group change to take effect
```

### "ERROR: Max frequency only 8 MHz (target was 12 MHz)"

**Solution:** The design doesn't meet timing at 12 MHz. Options:

1. **Accept the lower frequency**: Edit `Makefile` and change `--freq 12` to `--freq 8`
2. **Simplify the design**: Disable FPU or reduce complexity
3. **Add pipeline registers**: This requires RTL changes (advanced)

### "ERROR: insufficient resources"

**Solution:** The design is too large for HX8K. See [Reducing Resource Usage](#reducing-resource-usage).

### "Simulation works but FPGA doesn't"

**Common issues:**

1. **Clock/Reset problems**: Verify reset button is connected and working
2. **Memory initialization**: Check that instruction memory is correctly initialized
3. **Timing violations**: Run `make timing` and check for timing errors
4. **Pin constraints**: Verify PCF file matches your board's pinout

**Debug steps:**

1. Use a logic analyzer or scope to probe signals
2. Add test points in `fpga_top.sv` (e.g., expose FSM state on LEDs)
3. Simplify the design to isolate the problem

### "make program" hangs

**Solution:** Check USB connection:

```bash
# List USB devices
lsusb | grep 0403:6010

# If not found, reconnect board or try different USB port
```

## Next Steps

### Adding UART Communication

To add serial communication for printf-style debugging:

1. Add a UART module to `fpga/` directory
2. Instantiate UART in `fpga_top.sv`
3. Map UART to memory address range (e.g., 0x10000000)
4. Update PCF with UART TX/RX pin assignments

### Increasing Clock Frequency

To run at higher frequencies (e.g., 24-48 MHz):

1. Instantiate iCE40 PLL primitive in `fpga_top.sv`
2. Configure PLL for desired output frequency
3. Update `Makefile` with new `--freq` target
4. Re-run place-and-route and verify timing

### Using External SRAM/Flash

For larger programs, add external memory:

1. Connect SRAM/Flash to FPGA pins
2. Implement memory controller in RTL
3. Update memory interface in `fpga_top.sv`
4. Add pin constraints to PCF

### Automating Program Loading

Instead of hardcoding programs in BRAM:

1. Use `$readmemh()` in `bram_imem.sv` to load from file
2. Convert program to `.hex` format
3. Provide hex file to Yosys during synthesis

## References

- **Yosys Documentation**: https://yosyshq.net/yosys/
- **nextpnr Documentation**: https://github.com/YosysHQ/nextpnr
- **IceStorm Project**: https://github.com/YosysHQ/icestorm
- **iCE40 Family Handbook**: https://www.latticesemi.com/ice40
- **RISC-V Specifications**: https://riscv.org/specifications/

## Getting Help

If you encounter issues:

1. Check the build logs in `fpga/build/`
2. Review the [Troubleshooting](#troubleshooting) section
3. Open an issue on the GitHub repository with:
   - Tool versions (`yosys --version`, `nextpnr-ice40 --version`)
   - Full error messages
   - Build logs

---

**Last Updated:** 2026-01-27  
**Tested with:** Yosys 0.37, nextpnr-ice40 0.6, IceStorm (latest)
