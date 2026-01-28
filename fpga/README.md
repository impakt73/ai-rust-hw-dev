# FPGA Synthesis for Alchitry Cu v1

This directory contains files for synthesizing the RISC-V CPU to the Alchitry Cu v1 board (iCE40-HX8K-CB132) using open-source tools (Yosys + nextpnr + IceStorm).

## Status: ✅ Successfully Synthesized

The FPGA design is configured to run on the iCE40-HX8K within resource constraints:

- **Extensions disabled**: M (multiply/divide) and F (floating-point) extensions disabled
- **ISA supported**: RV32I base instruction set + C (compressed) + A (atomic) + Zicsr
- **Resource usage**: ~74% logic cells (5,698/7,680), 50% BRAM (16/32)
- **Clock frequency**: 25 MHz (via PLL), design achieves 30.56 MHz max
- **Test program**: LED rotation pattern (0xAA ↔ 0x55) that rotates every 1 second
- **Address mapping**: Proper CPU address (0x80000000+) to BRAM offset translation

## What's Included

The FPGA implementation includes:

- ✅ **RISC-V RV32IAC CPU**: Base integer + Atomic + Compressed instruction sets (M/F extensions disabled)
- ✅ **4 KB Instruction Memory**: On-chip block RAM (BRAM)
- ✅ **4 KB Data Memory**: On-chip block RAM (BRAM)
- ✅ **LED Controller Peripheral**: 8-bit LED output mapped at 0x50000000
- ✅ **PLL Clock Generation**: 100 MHz input → 25 MHz system clock for timing closure
- ✅ **Test Program**: LED rotation pattern program pre-loaded in instruction memory

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

# Run full synthesis flow (takes 2-5 minutes first time)
make

# This generates:
# - build/riscv_fpga.json  (synthesis output)
# - build/riscv_fpga.asc   (place-and-route output)
# - build/riscv_fpga.bin   (bitstream for programming)
```

### Program FPGA (Requires Hardware)

```bash
# Connect Alchitry Cu v1 board via USB
sudo make program

# Or manually using iceprog:
sudo iceprog build/riscv_fpga.bin
```

## Files

- **`fpga_top.sv`**: Top-level FPGA wrapper module
- **`bram_imem.sv`**: Block RAM instruction memory (4 KB)
- **`bram_dmem.sv`**: Block RAM data memory (4 KB)
- **`ice40hx8k.pcf`**: Pin constraint file for Alchitry Cu v1 board
- **`Makefile`**: Build automation for synthesis workflow
- **`build/`**: Generated build artifacts (created during synthesis)

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
- **Resources Used**: ~5,700 LUTs, 74% utilization, 16 BRAMs (50%)
- **Clock**: 100 MHz input → 25 MHz system clock (via PLL)
- **Peripherals**: 8 LEDs on main board
- **Programming**: USB cable for iceprog

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

The instruction memory includes an LED rotation program that displays an alternating pattern on the 8 LEDs, matching the behavior of `led_demo/led_pattern_top.sv`:

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

To change the program, edit the `initial` block in `bram_imem.sv`.

## Troubleshooting

### "Timing not met" error

The design uses a PLL to generate 25 MHz from the 100 MHz input clock, which ensures timing closure. If you need a different frequency, update:

1. **PLL parameters**: Edit `fpga_top.sv` PLL configuration (DIVR, DIVF, DIVQ)
2. **Makefile**: Change `--freq 25` to match your target frequency
3. **Test program**: Update delay loop count in `bram_imem.sv`

### "Insufficient resources" error

The design uses ~74% of HX8K logic resources. If synthesis fails:

1. **Already optimized**: M and F extensions are disabled by default
2. **Reduce memory**: Change BRAM sizes in `bram_imem.sv` and `bram_dmem.sv`

### "make program" fails

Check USB connection and permissions:

```bash
# List USB devices (should show FTDI device)
lsusb | grep 0403:6010

# Add user to dialout group for USB access
sudo usermod -a -G dialout $USER
# Log out and back in for changes to take effect
```

### Simulation works but FPGA doesn't

Common issues:

1. **Clock/Reset**: Verify reset button (P8) is not stuck
2. **Memory init**: Check instruction memory is correctly initialized
3. **Timing violations**: Run `make timing` to check for timing errors
4. **Pin mismatch**: Verify PCF matches your board

## Customization

### Changing Test Program

Edit `bram_imem.sv` and modify the `initial` block:

```systemverilog
initial begin
    // Your custom program here
    mem[0] = 32'h50000137;  // lui x15, 0x50000
    mem[1] = 32'h0AA00713;  // addi x14, x0, 0xAA
    mem[2] = 32'h00E7A023;  // sw x14, 0(x15)
    // ... more instructions
end
```

### Using External Memory File

Instead of hardcoding, use `$readmemh()`:

```systemverilog
initial begin
    $readmemh("program.hex", mem);
end
```

Then provide `program.hex` during synthesis.

### Changing Clock Frequency

To use a PLL for higher/lower frequencies:

1. Instantiate iCE40 PLL primitive in `fpga_top.sv`
2. Configure for desired frequency
3. Update `Makefile` `--freq` parameter
4. Re-run synthesis and check timing

## Next Steps

- **Add UART**: For printf-style debugging over serial
- **External SRAM**: For larger programs (>4KB)
- **Optimize Timing**: Add pipeline stages to run at higher clock speeds
- **CI Integration**: Automate synthesis checks in GitHub Actions

## References

- [Yosys Documentation](https://yosyshq.net/yosys/)
- [nextpnr Documentation](https://github.com/YosysHQ/nextpnr)
- [IceStorm Project](https://github.com/YosysHQ/icestorm)
- [iCE40 Family Handbook](https://www.latticesemi.com/ice40)
- [Alchitry Cu Documentation](https://alchitry.com/cu/)
- [RISC-V Specifications](https://riscv.org/specifications/)
