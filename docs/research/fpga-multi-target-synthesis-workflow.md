# FPGA Multi-Target Synthesis Workflow: Artix-7 and ECP5 via Open Source Tooling

**Research Document**  
**Context:** Extending the existing iCE40-HX8K Yosys/nextpnr synthesis flow to also target Xilinx Artix-7 and Lattice ECP5 FPGAs, using only open source tools  
**Date:** 2026-03-04

---

## Executive Summary

The project currently targets the Lattice iCE40-HX8K (Alchitry Cu v1) using an all-open-source toolchain: Yosys (synthesis), nextpnr-ice40 (place and route), and IceStorm utilities (bitstream packing and programming). This document researches how to elegantly extend that workflow to two additional FPGA families—**Lattice ECP5** and **Xilinx Artix-7**—while continuing to rely exclusively on open source tooling.

**Key Findings:**

1. **Lattice ECP5** is the most natural extension. The toolchain mirrors the iCE40 flow almost exactly: `synth_ecp5` in Yosys, `nextpnr-ecp5` for place and route, and Project Trellis (`ecppack`) for bitstream generation. The ECP5 is significantly more capable than the iCE40 and can host the full RV32IMACF implementation, including the F extension.

2. **Xilinx Artix-7** support is maturing via two parallel open source efforts:
   - **nextpnr-xilinx** (from the openXC7 project): Extends nextpnr to Xilinx 7-Series using Project X-Ray device databases. It is the most architecturally consistent approach for this project as it mirrors the existing flow.
   - **F4PGA** (formerly SymbiFlow): A more comprehensive umbrella project that uses VPR for placement and routing. It is more mature but relies on a larger software stack.

3. The main adaptation required is **vendor-primitive replacement**: the iCE40-specific `SB_PLL40_CORE` PLL must be replaced with `EHXPLLL` (ECP5) or `PLLE2_ADV`/`MMCME2_ADV` (Artix-7) in new per-target FPGA top wrappers. The RTL core (`top.sv` and all common modules) is vendor-agnostic and requires no changes.

4. **Constraint file formats differ** by family: PCF for iCE40, LPF for ECP5, and XDC for Artix-7.

5. A clean **multi-target Makefile** using a `TARGET` variable can drive the appropriate synthesis backend, constraint file, and bitstream tool with minimal duplication.

**Recommendation:** Pursue ECP5 support first (high maturity, close tool parity, substantial resource improvement). Add Artix-7 support via nextpnr-xilinx/openXC7 second, with the understanding that this path is still experimental for complex designs.

---

## 1. Current Workflow (iCE40-HX8K Baseline)

The baseline synthesis pipeline in `rtl/fpga/Makefile` follows this linear flow:

```
SystemVerilog sources
        │
        ▼
   Yosys (synth_ice40)
        │  riscv_fpga.json
        ▼
   nextpnr-ice40
        │  riscv_fpga.asc
        ▼
   icepack
        │  riscv_fpga.bin
        ▼
   iceprog / icetime
```

**Key iCE40-specific artefacts:**

| Element | iCE40 Primitive / File |
|---------|----------------------|
| Clock generation | `SB_PLL40_CORE` (in `fpga_top.sv`) |
| Constraint format | `.pcf` (Physical Constraint File) |
| Synthesis command | `synth_ice40` |
| P&R tool | `nextpnr-ice40 --hx8k` |
| Bitstream pack | `icepack` → `.bin` |
| Programmer | `iceprog` or `openFPGALoader` |
| Timing analysis | `icetime` |

All RTL under `rtl/common/` is **fully vendor-neutral** SystemVerilog and requires no changes for new targets. The only iCE40-specific code is in `rtl/fpga/fpga_top.sv` (PLL instantiation) and `rtl/fpga/ice40hx8k.pcf` (pin constraints).

---

## 2. Lattice ECP5 Target

### 2.1 Tool Overview

| Stage | Tool | Package |
|-------|------|---------|
| Synthesis | Yosys (`synth_ecp5`) | `yosys` |
| Place & Route | nextpnr-ecp5 | `nextpnr-ecp5` |
| Bitstream generation | `ecppack` (Project Trellis) | `prjtrellis` |
| Device database | Project Trellis | `prjtrellis-db` or built-in |
| FPGA programming | `openFPGALoader` or `ecpdap` | `openfpgaloader` |
| PLL helper | `ecppll` (Project Trellis) | included in `prjtrellis` |

All tools are maintained under the **YosysHQ** umbrella (https://github.com/YosysHQ) and are in active development. The ECP5 open-source flow is considered **production-grade** for most designs.

### 2.2 Synthesis Command

```bash
yosys -p "read_verilog -sv $(SOURCES); \
          synth_ecp5 -top $(TOP_MODULE) -json $(JSON)" 2>&1 | tee build/yosys.log
```

The `synth_ecp5` pass handles ECP5-specific technology mapping, including LUT4s, carry chains, distributed RAM, and BRAM inference. It respects manually-instantiated ECP5 primitives such as `EHXPLLL` and passes them through unchanged.

### 2.3 Place and Route Command

```bash
nextpnr-ecp5 \
    --$(ECP5_DEVICE) \
    --package $(ECP5_PACKAGE) \
    --json $(JSON) \
    --lpf $(LPF) \
    --textcfg $(CONFIG) \
    --freq $(TARGET_FREQ_MHZ) \
    2>&1 | tee build/nextpnr.log
```

Where `--$(ECP5_DEVICE)` is one of: `--um5g-25k`, `--um5g-45k`, `--um5g-85k`, `--um-25k`, `--um-45k`, `--um-85k`.

### 2.4 Bitstream Generation and Programming

```bash
ecppack build/$(PROJECT).config build/$(PROJECT).bit

# Program via openFPGALoader (SRAM - volatile):
openFPGALoader -b $(BOARD) build/$(PROJECT).bit

# Program to flash (persistent):
openFPGALoader -b $(BOARD) -f build/$(PROJECT).bit
```

### 2.5 PLL Replacement: `SB_PLL40_CORE` → `EHXPLLL`

The ECP5 uses the `EHXPLLL` primitive instead of `SB_PLL40_CORE`. The easiest approach is to use the `ecppll` tool from Project Trellis to auto-generate the correct wrapper:

```bash
ecppll -f rtl/fpga/ecp5/pll.sv -n pll -i 25 -o 50
```

This generates a `pll` module with the correct `EHXPLLL` instantiation and computed divider values. For a 25 MHz input to 50 MHz output (example), the generated module would look like:

```systemverilog
module pll (
    input  logic clki,
    output logic clko,
    output logic locked
);
    EHXPLLL #(
        .PLLRST_ENA("DISABLED"),
        .INTFB_WAKE("DISABLED"),
        .STDBY_ENABLE("DISABLED"),
        .DPHASE_SOURCE("DISABLED"),
        .OUTDIVIDER_MUXA("DIVA"),
        .OUTDIVIDER_MUXB("DIVB"),
        .OUTDIVIDER_MUXC("DIVC"),
        .OUTDIVIDER_MUXD("DIVD"),
        .CLKI_DIV(1),
        .CLKOP_ENABLE("ENABLED"),
        .CLKOP_DIV(12),
        .CLKOP_CPHASE(5),
        .CLKOP_FPHASE(0),
        .CLKFB_SEL("INTERNAL"),
        .CLKOP_TRIM_TRIM("0b0000"),
        .CLKOP_TRIM_DELAY(0),
        .FEEDBK_PATH("CLKOP"),
        .CLKFB_DIV(2)
    ) pll_inst (
        .RST(1'b0),
        .STDBY(1'b0),
        .CLKI(clki),
        .CLKOP(clko),
        .CLKFB(clko),
        .PHASESEL0(1'b0),
        .PHASESEL1(1'b0),
        .PHASEDIR(1'b1),
        .PHASESTEP(1'b1),
        .PHASELOADREG(1'b1),
        .PLLWAKESYNC(1'b0),
        .ENCLKOP(1'b0),
        .LOCK(locked)
    );
endmodule
```

The `fpga_top.sv` wrapper for ECP5 would swap the `SB_PLL40_CORE` block for this generated `pll` module. All other instantiation (CPU, UART, peripherals) remains unchanged.

### 2.6 Constraint File Format: LPF

ECP5 uses **LPF (Lattice Preference File)** for pin assignments and timing constraints, which is read by `nextpnr-ecp5`. The syntax is similar to PCF but with some differences:

```lpf
# Clock input
LOCATE COMP "clk" SITE "P3";
IOBUF PORT "clk" IO_TYPE=LVCMOS33;
FREQUENCY PORT "clk" 25.000000 MHZ;

# Reset button (active low)
LOCATE COMP "rst_n_btn" SITE "T1";
IOBUF PORT "rst_n_btn" IO_TYPE=LVCMOS33;

# LED outputs
LOCATE COMP "led[0]" SITE "E3";
IOBUF PORT "led[0]" IO_TYPE=LVCMOS33;
# ... etc.

# UART
LOCATE COMP "usb_rx" SITE "C11";
IOBUF PORT "usb_rx" IO_TYPE=LVCMOS33;
LOCATE COMP "usb_tx" SITE "A11";
IOBUF PORT "usb_tx" IO_TYPE=LVCMOS33;
```

The exact pin names depend on the target ECP5 board.

### 2.7 Recommended ECP5 Boards

| Board | FPGA | Resources | UART | Notes |
|-------|------|-----------|------|-------|
| **ULX3S** (85F) | ECP5-85F (LFE5U-85F) | 84K LUT4, 208 DSP | FTDI USB-serial | Most popular; strong open-source support |
| **OrangeCrab** (85F) | ECP5-85F (LFE5U-85F) | 84K LUT4 | USB-C (USB CDC) | Compact; Feather form factor |
| **Colorlight 5A-75B** | ECP5-25F (LFE5U-25F) | 24K LUT4 | Via JTAG only | Very low cost; no on-board USB-serial |
| **iCEBreaker v2** (if ECP5) | ECP5-12F | 12K LUT4 | FTDI USB-serial | Smallest ECP5 |

The **ULX3S with 85F** is the most recommended: it provides 84K LUT4 cells (vs. 7,680 on iCE40-HX8K), room for the full RV32IMACF implementation including floating point, and has dedicated FTDI USB-serial UART matching the existing host bus communication model.

### 2.8 Resource Headroom

Compared to the current iCE40-HX8K target (61% utilization, 4,688/7,680 LUTs):

| Resource | iCE40-HX8K | ECP5-25F | ECP5-45F | ECP5-85F |
|----------|-----------|----------|----------|----------|
| LUT4s | 7,680 | 24,288 | 43,848 | 83,640 |
| BRAM (kbit) | 128 | 194 | 351 | 352 |
| DSP18s | N/A | 28 | 56 | 156 |
| Current design headroom | 39% free | >85% free | >89% free | >94% free |

On an ECP5-85F, the design would use roughly **5–6% of available LUTs**, leaving ample room to enable the full F extension, add pipeline stages, or increase SRAM size.

---

## 3. Xilinx Artix-7 Target

### 3.1 Tool Overview

Xilinx 7-Series (which includes Artix-7) is supported by two open source flows:

#### Option A: nextpnr-xilinx / openXC7 (Recommended for this project)

| Stage | Tool | Notes |
|-------|------|-------|
| Synthesis | Yosys (`synth_xilinx -family xc7`) | Mature |
| Place & Route | nextpnr-xilinx | Uses Project X-Ray databases |
| Bitstream | fasm2frames + xc7frames2bit | Part of Project X-Ray / openXC7 |
| Device database | Project X-Ray (prjxray) | Reverse-engineered |
| Programming | `openFPGALoader -b arty` | Apache-2.0 |
| Distribution | openXC7 toolchain-installer | Nix / manual install |

**Status:** Actively maintained, functional for moderate complexity designs. The nextpnr-xilinx backend is developed by **@gatecat** (also the main nextpnr developer) at https://github.com/gatecat/nextpnr-xilinx. The openXC7 project (https://github.com/openxc7) packages the full toolchain with install scripts updated as recently as mid-2024.

#### Option B: F4PGA (formerly SymbiFlow)

| Stage | Tool | Notes |
|-------|------|-------|
| Synthesis | Yosys with F4PGA plugins | Enhanced BRAM/DSP inference |
| Place & Route | VPR | Academic-origin, different P&R engine |
| Bitstream | FASM + Project X-Ray | Same bitstream tooling |
| Distribution | Conda/Pip packages | Larger software dependency |

**Status:** More comprehensive device support but larger dependency chain. Best suited when F4PGA's architecture-aware VPR provides better results than nextpnr-xilinx.

**For this project, nextpnr-xilinx / openXC7 is recommended** as it maintains architectural consistency with the existing iCE40 flow (same nextpnr P&R tool, same JSON netlist handoff).

### 3.2 Synthesis Command

```bash
yosys -p "read_verilog -sv $(SOURCES); \
          synth_xilinx -family xc7 -top $(TOP_MODULE) -edif $(EDIF)" 2>&1 | tee build/yosys.log
```

Note: nextpnr-xilinx currently consumes EDIF (`.edf`) rather than JSON for the Xilinx target.

The `synth_xilinx -family xc7` pass handles:
- LUT6 technology mapping (Artix-7 uses 6-input LUTs, not 4-input)
- BRAM36/BRAM18 inference
- DSP48E1 inference (for multipliers)
- CARRY4 chain inference
- Buffer and flip-flop mapping

### 3.3 Place and Route Command

```bash
# Generate nextpnr-xilinx database for the target device first:
python3 nextpnr-xilinx/xilinx/python/bbaexport.py \
    --device xc7a35tcsg324-1 \
    --bba build/xc7a35t.bba

# Run P&R:
nextpnr-xilinx \
    --chipdb build/xc7a35t.bba \
    --xdc $(XDC) \
    --edif $(EDIF) \
    --fasm $(FASM) \
    --freq $(TARGET_FREQ_MHZ) \
    2>&1 | tee build/nextpnr.log
```

### 3.4 Bitstream Generation and Programming

```bash
# Convert FASM to frames
fasm2frames --part xc7a35tcsg324-1 build/$(PROJECT).fasm build/$(PROJECT).frames

# Convert frames to bitstream
xc7frames2bit --part-file $(XRAY_DB)/xc7a35tcsg324-1/part.yaml \
              --frm-file build/$(PROJECT).frames \
              --output-file build/$(PROJECT).bit

# Program via openFPGALoader:
openFPGALoader -b arty build/$(PROJECT).bit

# Persistent flash programming:
openFPGALoader -b arty -f build/$(PROJECT).bit
```

### 3.5 PLL Replacement: `SB_PLL40_CORE` → `PLLE2_ADV` or `MMCME2_ADV`

Artix-7 provides MMCM (Mixed-Mode Clock Manager) and PLL primitives. Both are usable in the open source flow as Yosys passes them through un-modified to nextpnr-xilinx.

The **`PLLE2_ADV`** is simpler and preferred for straightforward frequency synthesis.

The Arty A7 provides a **100 MHz** on-board oscillator. The following example targets 100 MHz input → 50 MHz system clock:

```systemverilog
// Arty A7: 100 MHz input → 50 MHz system clock
// VCO = 100 * 10 = 1000 MHz, output = 1000 / 20 = 50 MHz

logic pll_clk_fb;
logic pll_clk_out;
logic pll_locked;

PLLE2_ADV #(
    .BANDWIDTH         ("OPTIMIZED"),
    .CLKFBOUT_MULT     (10),        // VCO = 100 * 10 = 1000 MHz
    .CLKFBOUT_PHASE    (0.0),
    .CLKIN1_PERIOD     (10.0),      // 100 MHz = 10 ns period
    .CLKOUT0_DIVIDE    (20),        // 1000 / 20 = 50 MHz
    .CLKOUT0_DUTY_CYCLE(0.5),
    .CLKOUT0_PHASE     (0.0),
    .DIVCLK_DIVIDE     (1),
    .REF_JITTER1       (0.010),
    .STARTUP_WAIT      ("FALSE")
) pll_inst (
    .CLKFBIN    (pll_clk_fb),
    .CLKIN1     (clk),
    .CLKIN2     (1'b0),
    .CLKINSEL   (1'b1),
    .DADDR      (7'b0),
    .DCLK       (1'b0),
    .DEN        (1'b0),
    .DI         (16'b0),
    .DWE        (1'b0),
    .PWRDWN     (1'b0),
    .RST        (1'b0),
    .CLKFBOUT   (pll_clk_fb),
    .CLKOUT0    (pll_clk_out),
    .LOCKED     (pll_locked)
);

// Route through global clock buffer
BUFG sys_clk_bufg (
    .I(pll_clk_out),
    .O(sys_clk)
);
```

**Key difference from iCE40:** Artix-7 requires explicit `BUFG` (Global Clock Buffer) instantiation to route the PLL output onto the global clock network with low skew. The iCE40 PLL drives the global network directly via `PLLOUTGLOBAL`.

### 3.6 Constraint File Format: XDC

Artix-7 uses **XDC (Xilinx Design Constraints)**, a Tcl-based constraint format accepted by both Vivado and nextpnr-xilinx:

```tcl
# Clock: 100 MHz on-board oscillator (Arty A7)
create_clock -name sys_clk -period 10.0 [get_ports clk]
set_property PACKAGE_PIN E3      [get_ports clk]
set_property IOSTANDARD  LVCMOS33 [get_ports clk]

# Reset button (active low, pull-up)
set_property PACKAGE_PIN C2      [get_ports rst_n_btn]
set_property IOSTANDARD  LVCMOS33 [get_ports rst_n_btn]
set_property PULLUP true          [get_ports rst_n_btn]

# LED outputs (Arty A7 on-board LEDs)
set_property PACKAGE_PIN H5      [get_ports {led[0]}]
set_property IOSTANDARD  LVCMOS33 [get_ports {led[0]}]
# ... additional LEDs H6, J5, J1 for bits 1-3

# UART (USB-UART via FTDI, Arty A7)
set_property PACKAGE_PIN A9      [get_ports usb_rx]
set_property IOSTANDARD  LVCMOS33 [get_ports usb_rx]
set_property PACKAGE_PIN D10     [get_ports usb_tx]
set_property IOSTANDARD  LVCMOS33 [get_ports usb_tx]
```

### 3.7 Recommended Artix-7 Boards

| Board | FPGA | Resources | Clock | UART | JTAG |
|-------|------|-----------|-------|------|------|
| **Arty A7-35T** (Digilent) | XC7A35T (CSG324) | 33K LUT6, 50 DSP48 | 100 MHz | USB-UART (FTDI) | USB-JTAG |
| **Arty A7-100T** (Digilent) | XC7A100T | 101K LUT6, 240 DSP48 | 100 MHz | USB-UART (FTDI) | USB-JTAG |
| **Basys 3** (Digilent) | XC7A35T (CPG236) | 33K LUT6 | 100 MHz | USB-UART | USB-JTAG |
| **Nexys A7** (Digilent) | XC7A100T | 101K LUT6 | 100 MHz | USB-UART | USB-JTAG |

The **Arty A7-35T** is the standard reference board for Xilinx open-source development and is directly supported by the openXC7 toolchain. The **Arty A7-100T** provides considerably more resources for the full RV32IMACF design.

### 3.8 Resource Headroom

| Resource | iCE40-HX8K | Artix-7 A35T | Artix-7 A100T |
|----------|-----------|-------------|--------------|
| LUT6 equivalents | 7,680 | ~33,280 | ~101,440 |
| BRAM (kbit) | 128 | 1,800 | 4,860 |
| DSP slices | N/A | 90 (DSP48E1) | 240 |
| Current design fit | 61% | ~15% est. | ~5% est. |

The Artix-7 A35T is sufficient for the full RV32IMACF design with considerable headroom.

---

## 4. Required Project Changes

### 4.1 Directory Structure

The recommended directory structure introduces per-target subdirectories under `rtl/fpga/` while keeping the common RTL untouched:

```
rtl/
├── common/                    # Unchanged - vendor-neutral RTL
│   └── ...
└── fpga/
    ├── Makefile               # Updated multi-target Makefile
    ├── ice40hx8k/             # (existing, reorganised from fpga root)
    │   ├── fpga_top.sv        # iCE40-specific top (SB_PLL40_CORE)
    │   ├── ice40hx8k.pcf      # Existing pin constraints (PCF)
    │   └── stub_fpu.sv        # Existing FPU stub
    ├── ecp5/
    │   ├── fpga_top.sv        # ECP5-specific top (EHXPLLL)
    │   ├── ulx3s_85f.lpf      # Pin constraints for ULX3S 85F (LPF)
    │   └── stub_fpu.sv        # FPU stub (shared or symlinked)
    └── artix7/
        ├── fpga_top.sv        # Artix-7 specific top (PLLE2_ADV + BUFG)
        ├── arty_a7_35t.xdc    # Pin constraints for Arty A7-35T (XDC)
        ├── arty_a7_100t.xdc   # Pin constraints for Arty A7-100T (XDC)
        └── stub_fpu.sv        # FPU stub (shared or symlinked)
```

### 4.2 Multi-Target Makefile Design

The Makefile should accept a `TARGET` variable that selects the backend and adjusts all tool invocations accordingly:

```makefile
# Default target: existing iCE40-HX8K
TARGET ?= ice40hx8k

# ============================================================
# Target-specific configuration
# ============================================================

ifeq ($(TARGET), ice40hx8k)
  DEVICE        = hx8k
  PACKAGE       = cb132
  FPGA_DIR      = ice40hx8k
  CONSTRAINT    = $(FPGA_DIR)/ice40hx8k.pcf
  SYNTH_CMD     = synth_ice40 -top $(TOP_MODULE) -json $(JSON)
  PNR_CMD       = nextpnr-ice40 --$(DEVICE) --package $(PACKAGE) \
                    --json $(JSON) --pcf $(CONSTRAINT) --asc $(ASC) --freq $(TARGET_FREQ)
  PACK_CMD      = icepack $(ASC) $(BIN)
  PROGRAM_CMD   = iceprog $(BIN)
  TARGET_FREQ   = 25
  OUTPUT_EXT    = bin
endif

ifeq ($(TARGET), ecp5_ulx3s)
  DEVICE        = um5g-85k
  PACKAGE       = CABGA381
  FPGA_DIR      = ecp5
  CONSTRAINT    = $(FPGA_DIR)/ulx3s_85f.lpf
  SYNTH_CMD     = synth_ecp5 -top $(TOP_MODULE) -json $(JSON)
  PNR_CMD       = nextpnr-ecp5 --$(DEVICE) --package $(PACKAGE) \
                    --json $(JSON) --lpf $(CONSTRAINT) --textcfg $(CONFIG) --freq $(TARGET_FREQ)
  PACK_CMD      = ecppack $(CONFIG) $(BIN)
  PROGRAM_CMD   = openFPGALoader -b ulx3s $(BIN)
  TARGET_FREQ   = 50
  OUTPUT_EXT    = bit
endif

ifeq ($(TARGET), artix7_arty_a35t)
  DEVICE        = xc7a35tcsg324-1
  FPGA_DIR      = artix7
  CONSTRAINT    = $(FPGA_DIR)/arty_a7_35t.xdc
  SYNTH_CMD     = synth_xilinx -family xc7 -top $(TOP_MODULE) -edif $(EDIF)
  PNR_CMD       = nextpnr-xilinx --chipdb $(CHIPDB) \
                    --xdc $(CONSTRAINT) --edif $(EDIF) --fasm $(FASM) --freq $(TARGET_FREQ)
  PACK_CMD      = fasm2frames --part $(DEVICE) $(FASM) $(FRAMES) && \
                  xc7frames2bit --part-file $(XRAY_DB)/$(DEVICE)/part.yaml \
                                --frm-file $(FRAMES) --output-file $(BIN)
  PROGRAM_CMD   = openFPGALoader -b arty $(BIN)
  TARGET_FREQ   = 50
  OUTPUT_EXT    = bit
endif

# ============================================================
# Usage: make TARGET=ecp5_ulx3s
#        make TARGET=artix7_arty_a35t
#        make            (defaults to ice40hx8k)
# ============================================================
```

### 4.3 FPGA Top-Level Wrapper Changes

Each target needs its own `fpga_top.sv` that handles only:
1. PLL instantiation (vendor-specific primitive)
2. Reset synchronization (logic using `ff_sync.sv` is already vendor-neutral)
3. Port list matching the board's pin constraint file

The instantiation of `top` (CPU core), `uart` (host bus), and peripheral wiring is **identical** across all three wrappers. This can be factored out into a shared SystemVerilog include or kept as a small repetition.

**Template structure common to all three wrappers:**

```systemverilog
// Only this block is target-specific:
// ==========================================
<VENDOR_PLL> pll_inst ( ... ); // iCE40: SB_PLL40_CORE
                                // ECP5:  EHXPLLL
                                // Artix: PLLE2_ADV + BUFG
// ==========================================

// Everything below is identical across targets:
top #( .CLK_FREQ_HZ(SYS_CLK_HZ), ... ) cpu_inst ( ... );
uart #( .CLK_FREQ_HZ(SYS_CLK_HZ), ... ) host_uart_inst ( ... );
// LED, button, and seven-segment logic ...
```

The only other change is updating `CLK_FREQ_HZ` and `BAUD_RATE` parameters to match the target clock frequency (e.g., 50 MHz for ECP5/Artix-7 vs. 25 MHz for iCE40).

---

## 5. Toolchain Installation

### 5.1 ECP5 Tools (Stable)

**Option 1: System packages (Ubuntu/Debian)**

```bash
sudo apt-get update
sudo apt-get install -y yosys nextpnr-ecp5 prjtrellis openfpgaloader
```

**Option 2: Build from source (latest features)**

```bash
# Project Trellis (device database and ecppack/ecppll tools)
git clone --recurse-submodules https://github.com/YosysHQ/prjtrellis.git
cd prjtrellis/libtrellis
cmake -DCMAKE_INSTALL_PREFIX=/usr/local .
make -j$(nproc) && sudo make install && cd ../..

# nextpnr with ECP5 support
git clone https://github.com/YosysHQ/nextpnr.git
cd nextpnr
cmake -DARCH=ecp5 \
      -DTRELLIS_INSTALL_PREFIX=/usr/local \
      -DCMAKE_INSTALL_PREFIX=/usr/local .
make -j$(nproc) && sudo make install && cd ..

# openFPGALoader
git clone https://github.com/trabucayre/openFPGALoader.git
cd openFPGALoader
cmake -B build . && cmake --build build -j$(nproc)
sudo cmake --install build && cd ..
```

### 5.2 Artix-7 Tools via openXC7 (Recommended)

The **openXC7 toolchain-installer** (https://github.com/openXC7/toolchain-installer) provides a script that installs all required tools from pre-built binaries or Nix packages:

```bash
# Clone openXC7 toolchain installer
git clone https://github.com/openXC7/toolchain-installer.git
cd toolchain-installer

# Install to /opt/openxc7 (adjust prefix as needed)
./install.sh

# Add to PATH
export PATH=/opt/openxc7/bin:$PATH
```

This installs:
- Yosys with Xilinx plugins
- nextpnr-xilinx with xc7 chipdb files
- Project X-Ray tools (`fasm2frames`, `xc7frames2bit`)
- openFPGALoader

**Manual / source build (advanced):**

```bash
# Project X-Ray (Xilinx device database)
git clone https://github.com/f4pga/prjxray.git
cd prjxray
# Download pre-built device databases (building from scratch requires Vivado)
make download-latest-db
sudo make install && cd ..

# nextpnr-xilinx
git clone https://github.com/gatecat/nextpnr-xilinx.git
cd nextpnr-xilinx
cmake -DARCH=xilinx -DCMAKE_INSTALL_PREFIX=/usr/local .
# Generate chipdb for target device
python3 xilinx/python/bbaexport.py --device xc7a35tcsg324-1 --bba bba/xc7a35t.bba
bbasm --l bba/xc7a35t.bba bba/xc7a35t.bin
make -j$(nproc) && sudo make install && cd ..
```

**Note on maturity:** The openXC7 project maintains pre-built toolchain releases. Building nextpnr-xilinx from source is considerably more involved than building nextpnr-ice40 or nextpnr-ecp5 because it requires pre-generated chipdb files derived from Project X-Ray databases.

---

## 6. Primitive and Vendor IP Migration

### 6.1 Summary of Vendor Primitives by Target

| Primitive Function | iCE40-HX8K | ECP5 | Artix-7 |
|-------------------|-----------|------|---------|
| **PLL** | `SB_PLL40_CORE` | `EHXPLLL` | `PLLE2_ADV` or `MMCME2_ADV` |
| **Global Clock Buffer** | (implicit in PLL) | (implicit) | `BUFG` (explicit required) |
| **Block RAM** | `SB_RAM40_4K` (inferred) | `DP16KD` (inferred) | `RAMB36E1` / `RAMB18E1` (inferred) |
| **DSP** | N/A | `MULT18X18D` (inferred) | `DSP48E1` (inferred) |
| **I/O Buffer** | `SB_IO` (optional) | (automatic) | (automatic) |

For BRAM and DSP, all three synthesis commands (`synth_ice40`, `synth_ecp5`, `synth_xilinx`) **infer** technology-specific primitives automatically from SystemVerilog `always_ff` BRAM patterns and `*` multiply operators. No manual changes to `regfile.sv`, `sync_dpram.sv`, or `mul_unit.sv` are needed.

The only **manual intervention** required is the PLL instance, which cannot be inferred and is already manually instantiated in `fpga_top.sv`. A new per-target top wrapper handles this.

### 6.2 Reset Synchronizer (`ff_sync.sv`)

The existing `ff_sync.sv` module uses standard `always_ff` and is fully vendor-neutral. No changes are needed. All three flows will synthesize it correctly.

### 6.3 FPU Stub (`stub_fpu.sv`)

The `stub_fpu.sv` module is already vendor-neutral SystemVerilog. The same stub can be shared across all three targets (or softlinked). If the F extension is enabled, the actual FPU implementation in `rtl/common/fpu/` is also vendor-neutral.

---

## 7. Timing Considerations

### 7.1 Expected Fmax by Target

The current iCE40 design achieves **34.91 MHz Fmax** at the 25 MHz target. For ECP5 and Artix-7:

| Metric | iCE40-HX8K | ECP5 (expected) | Artix-7 (expected) |
|--------|-----------|----------------|-------------------|
| **LUT technology** | 4-input | 4-input | 6-input |
| **Carry chain speed** | ~1.2 ns/bit | ~0.7 ns/bit | ~0.5 ns/bit |
| **Routing fabric** | Limited | Rich | Rich |
| **Expected Fmax** | 34.91 MHz (achieved) | 60–80 MHz (est.) | 80–120 MHz (est.) |
| **Recommended target** | 25 MHz | 50 MHz | 50–100 MHz |

The critical path identified in `rtl/fpga/SYNTHESIS_ANALYSIS.md`—through the 32-bit ALU carry chain then `host_bus_mux` and `bus.sv`—will be significantly shorter on ECP5 and Artix-7 due to faster carry chains and better routing resources.

### 7.2 UART Baud Rate Scaling

The host bus UART (`uart.sv`) is parameterized by `CLK_FREQ_HZ` and `BAUD_RATE`. When moving to a higher system clock (e.g., 50 MHz), updating the `CLK_FREQ_HZ` parameter in the `fpga_top.sv` wrapper is sufficient. The baud rate divisor is computed at elaboration time. The host-side `fpga-host` crate also needs to match the configured baud rate.

---

## 8. CI Workflow Integration

The existing CI workflow (`ci.yml`) runs FPGA synthesis as a verification step. To support multiple targets, the CI job can be parameterised using a matrix strategy:

```yaml
jobs:
  fpga-synthesis:
    strategy:
      matrix:
        target: [ice40hx8k, ecp5_ulx3s, artix7_arty_a35t]
      fail-fast: false
    steps:
      - name: Install tools
        run: |
          sudo apt-get install -y yosys
          # Install target-specific tools based on matrix.target
          if [ "${{ matrix.target }}" = "ice40hx8k" ]; then
            sudo apt-get install -y fpga-icestorm nextpnr-ice40
          elif [ "${{ matrix.target }}" = "ecp5_ulx3s" ]; then
            sudo apt-get install -y prjtrellis nextpnr-ecp5 openfpgaloader
          fi
          # Note: artix7 target requires openXC7 manual install
      - name: Synthesize
        run: cd rtl/fpga && make TARGET=${{ matrix.target }}
```

**Note:** The Artix-7 target is excluded from automated CI initially due to the more complex tool installation. The ECP5 and iCE40 targets can be run in CI using system packages.

---

## 9. Trade-offs and Recommendations

### 9.1 ECP5 vs. iCE40 Comparison

| Aspect | iCE40-HX8K | ECP5-85F |
|--------|-----------|---------|
| **Open source maturity** | ⭐⭐⭐⭐⭐ (Very mature) | ⭐⭐⭐⭐⭐ (Very mature) |
| **Resources** | Limited (7,680 LUT4) | Large (83,640 LUT4) |
| **F extension feasibility** | ❌ Too resource-constrained | ✅ Comfortable |
| **Higher clock speeds** | Limited (~35 MHz) | Good (60–80 MHz est.) |
| **DSP blocks** | ❌ None | ✅ 156 (useful for mul_unit.sv) |
| **Tool complexity delta** | Baseline | Minimal – same flow |
| **Cost of typical board** | ~$50 (Alchitry Cu) | ~$70–90 (ULX3S) |

**Verdict:** ECP5 is a clear improvement with minimal toolchain overhead. Strongly recommended as the first new target.

### 9.2 Artix-7 vs. ECP5 Comparison

| Aspect | ECP5-85F | Artix-7 A35T | Artix-7 A100T |
|--------|---------|-------------|--------------|
| **Open source P&R maturity** | ⭐⭐⭐⭐⭐ (Production) | ⭐⭐⭐ (Experimental) | ⭐⭐⭐ (Experimental) |
| **Resources** | Large | Medium | Large |
| **LUT technology** | 4-input | 6-input (more efficient) | 6-input |
| **DSP blocks** | 156 | 90 | 240 |
| **Tool install complexity** | Low | High (openXC7 manual install) | High |
| **CI integration ease** | Easy (apt packages) | Harder | Harder |
| **Typical board** | ULX3S ~$80 | Arty A7 ~$130 | Arty A7-100T ~$250 |
| **Vendor ecosystem** | Lattice Diamond (FOSS alternative) | Vivado (FOSS alternative) | Vivado (FOSS alternative) |

**Verdict:** Artix-7 offers a wider DSP ecosystem and the most popular commercial board family, but the open source toolchain is more complex to install and less mature for P&R. Recommend pursuing it after ECP5 is stable.

### 9.3 Recommended Implementation Order

1. **Phase 1: ECP5 (Lattice)** – High value, low risk
   - Target board: ULX3S 85F
   - Effort: Low (tool parity with existing flow)
   - Enables full RV32IMACF including F extension

2. **Phase 2: Artix-7 (Xilinx)** – Medium value, medium risk
   - Target board: Arty A7-35T
   - Effort: Medium (nextpnr-xilinx install, PLLE2_ADV wrapper, XDC constraints)
   - Enables targeting the most widely-used FPGA family in academia/industry
   - Monitor openXC7 maturity before committing to CI integration

---

## 10. Proof-of-Concept Makefile Snippet

Below is a self-contained proof-of-concept for how the multi-target Makefile would drive the ECP5 synthesis, demonstrating the minimal changes relative to the current iCE40 flow:

```makefile
# ECP5 synthesis (add alongside existing ice40 targets)
.PHONY: synth-ecp5
synth-ecp5: $(BUILD_DIR)
	bash -e -o pipefail -c '$(YOSYS) -p \
	    "read_verilog -sv $(RTL_SOURCES) ecp5/fpga_top.sv ecp5/stub_fpu.sv; \
	     synth_ecp5 -top fpga_top -json $(BUILD_DIR)/$(PROJECT)_ecp5.json" \
	     2>&1 | tee $(BUILD_DIR)/yosys_ecp5.log'

.PHONY: pnr-ecp5
pnr-ecp5: synth-ecp5
	bash -e -o pipefail -c 'nextpnr-ecp5 \
	    --um5g-85k --package CABGA381 \
	    --json $(BUILD_DIR)/$(PROJECT)_ecp5.json \
	    --lpf ecp5/ulx3s_85f.lpf \
	    --textcfg $(BUILD_DIR)/$(PROJECT)_ecp5.config \
	    --freq 50 \
	    2>&1 | tee $(BUILD_DIR)/nextpnr_ecp5.log'

.PHONY: bitstream-ecp5
bitstream-ecp5: pnr-ecp5
	ecppack $(BUILD_DIR)/$(PROJECT)_ecp5.config $(BUILD_DIR)/$(PROJECT)_ecp5.bit
```

This mirrors the existing iCE40 targets exactly, with `synth_ecp5`, `nextpnr-ecp5`, and `ecppack` substituted for their iCE40 counterparts.

---

## 11. References

### Tool Repositories

- **Yosys**: https://github.com/YosysHQ/yosys (synthesis, `synth_ice40` / `synth_ecp5` / `synth_xilinx`)
- **nextpnr**: https://github.com/YosysHQ/nextpnr (P&R, supports ice40, ecp5)
- **nextpnr-xilinx**: https://github.com/gatecat/nextpnr-xilinx (P&R for Xilinx 7-Series)
- **IceStorm**: https://github.com/YosysHQ/icestorm (iCE40 bitstream tools)
- **Project Trellis**: https://github.com/YosysHQ/prjtrellis (ECP5 device database and bitstream tools)
- **Project X-Ray**: https://github.com/f4pga/prjxray (Xilinx 7-Series device database)
- **openXC7**: https://github.com/openxc7 (Xilinx open-source toolchain packaging)
- **F4PGA**: https://f4pga.org (Open FPGA umbrella, VPR-based Xilinx flow)
- **openFPGALoader**: https://github.com/trabucayre/openFPGALoader (universal FPGA programmer)

### Board Documentation

- **ULX3S (ECP5)**: https://ulx3s.github.io
- **OrangeCrab (ECP5)**: https://github.com/gregdavill/OrangeCrab
- **Arty A7 (Artix-7)**: https://digilent.com/reference/programmable-logic/arty-a7/start

### Technical References

- **ECP5 PLL Guide (Project F)**: https://projectf.io/posts/ecp5-fpga-clock/
- **ecppll usage**: https://github.com/YosysHQ/prjtrellis (tools/ecppll)
- **Yosys synth_ecp5 docs**: https://yosyshq.readthedocs.io/projects/yosys/en/stable/cmd/synth_ecp5.html
- **Yosys synth_xilinx docs**: https://yosyshq.readthedocs.io/projects/yosys/en/0.46/cmd/synth_xilinx.html
- **Project Trellis documentation**: https://prjtrellis.readthedocs.io
