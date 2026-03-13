# FPGA Multi-Target Synthesis Workflow Research: Artix-7, ECP5, and Cyclone V

**Research Document**
**Context:** Historical research into extending the existing iCE40-HX8K Yosys/nextpnr synthesis flow to also target Xilinx Artix-7, Lattice ECP5, and Intel/Altera Cyclone V FPGAs
**Date:** 2026-03-04

**Status Update (2026-03-07):** The open-source Xilinx Artix-7 flow described in this document is no longer supported by this repository. The supported Alchitry Au build flow now uses Vivado batch/TCL under `rtl/fpga/`.

---

## Executive Summary

The project currently targets the Lattice iCE40-HX8K (Alchitry Cu v1) using an all-open-source toolchain: Yosys (synthesis), nextpnr-ice40 (place and route), and IceStorm utilities (bitstream packing and programming). This document researches how to elegantly extend that workflow to three additional FPGA families—**Lattice ECP5**, **Xilinx Artix-7**, and **Intel/Altera Cyclone V**—while continuing to rely exclusively on open source tooling.

**Key Findings:**

1. **Lattice ECP5** is the most natural extension. The toolchain mirrors the iCE40 flow almost exactly: `synth_ecp5` in Yosys, `nextpnr-ecp5` for place and route, and Project Trellis (`ecppack`) for bitstream generation. The target board is the **iCE Pi Zero** (ECP5-25F), an OSHW-certified Raspberry Pi Zero form factor board with a built-in USB-JTAG/UART converter and 50 MHz MEMS oscillator.

2. **Xilinx Artix-7** support was researched via two parallel open source efforts, but that path is now historical-only for this repository:
   - **nextpnr-xilinx** (from the openXC7 project): Extends nextpnr to Xilinx 7-Series using Project X-Ray device databases. It was the most architecturally consistent open-source option considered during this research.
   - **F4PGA** (formerly SymbiFlow): A more comprehensive umbrella project that uses VPR for placement and routing. It is more mature but relies on a larger software stack.
   - The target board is the **Alchitry Au** (XC7A35T), which shares the same form factor, connector standard, and USB-C programming interface as the existing Alchitry Cu v1.

3. **Intel/Altera Cyclone V** support exists through Yosys (`synth_intel_alm`) and the nextpnr **Mistral** backend, but is currently **experimental and research-grade**. The target device is the **Analogue Pocket** (Cyclone V SE A5, ~49K ALMs), a gaming handheld that exposes its FPGA via the openFPGA framework. Most active Pocket development still relies on Quartus Prime Lite (free but proprietary); a fully open-source Mistral-based flow is viable for exploratory work.

4. The main adaptation required for all new targets is **vendor-primitive replacement**: the iCE40-specific `SB_PLL40_CORE` PLL must be replaced with `EHXPLLL` (ECP5), `PLLE2_ADV` (Artix-7), or `PLL_CYCLONEV`/`ALTPLL` (Cyclone V) in new per-target FPGA top wrappers. The RTL core (`top.sv` and all common modules) is vendor-agnostic and requires no changes.

5. **Constraint file formats differ** by family: PCF for iCE40, LPF for ECP5, XDC for Artix-7, and QSF-derived constraints for Cyclone V.

6. A clean **multi-target Makefile** using a `TARGET` variable can drive the appropriate synthesis backend, constraint file, and bitstream tool with minimal duplication.

**Recommendation:** Pursue ECP5 (iCE Pi Zero) first—high maturity, close tool parity, near-identical board form factor to the Alchitry Cu. Add Artix-7 (Alchitry Au) second; it shares the same Alchitry ecosystem, making board-level integration very clean. Cyclone V (Analogue Pocket) is a third-phase effort, suitable as a research/experimental target until the Mistral toolchain matures.

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
   openFPGALoader
```

**Key iCE40-specific artefacts:**

| Element | iCE40 Primitive / File |
|---------|----------------------|
| Clock generation | `SB_PLL40_CORE` (in `fpga_top.sv`) |
| Constraint format | `.pcf` (Physical Constraint File) |
| Synthesis command | `synth_ice40` |
| P&R tool | `nextpnr-ice40 --hx8k` |
| Bitstream pack | `icepack` → `.bin` |
| Programmer | `openFPGALoader` |
| Timing analysis | `nextpnr-ice40` |

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
# iCE Pi Zero has a 50 MHz MEMS oscillator; generate a PLL to 50 MHz (pass-through) or scale up
ecppll -f rtl/fpga/ecp5/pll.sv -n pll -i 50 -o 50
```

This generates a `pll` module with the correct `EHXPLLL` instantiation and computed divider values. For a 50 MHz input to 50 MHz output (pass-through, or modify `-o` for a different target frequency), the generated module would look like:

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
# Clock input (iCE Pi Zero: 50 MHz MEMS oscillator)
LOCATE COMP "clk" SITE "P3";
IOBUF PORT "clk" IO_TYPE=LVCMOS33;
FREQUENCY PORT "clk" 50.000000 MHZ;

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

### 2.7 Recommended ECP5 Board: iCE Pi Zero

| Board | FPGA | Resources | Clock | UART/JTAG | Form Factor |
|-------|------|-----------|-------|-----------|-------------|
| **iCE Pi Zero** ⭐ | ECP5U-25F (LFE5U-25F) | 24K LUT4, 112 KiB BRAM | 50 MHz MEMS | On-board USB-JTAG + UART | Pi Zero (65×30 mm) |
| **ULX3S** (85F) | ECP5-85F (LFE5U-85F) | 84K LUT4, 208 DSP | 25 MHz | FTDI USB-serial | 100×80 mm |
| **OrangeCrab** (85F) | ECP5-85F (LFE5U-85F) | 84K LUT4 | 48 MHz | USB-C (USB CDC) | Feather form factor |
| **Colorlight 5A-75B** | ECP5-25F (LFE5U-25F) | 24K LUT4 | 25 MHz | Via JTAG only | LED controller board |

The **iCE Pi Zero** is the primary recommended target for this project:

- **50 MHz on-board MEMS oscillator**: Higher precision and frequency than the 25 MHz iCE40 oscillator. The design easily meets timing at 50 MHz.
- **On-board USB-JTAG + UART converter**: No external programmer required—matches the Alchitry Cu's USB-based programming model. The UART channel directly supports the host bus protocol.
- **ECP5U-25F (24K LUT4)**: ~3× the logic resources of the iCE40-HX8K. The existing design (4,688 LUTs) fits at ~19% utilization, leaving meaningful headroom for adding pipeline stages or enabling the F extension with a reduced FPU.
- **Open Source Hardware (OSHWA FR000026)**: All board schematics and KiCad files are open source (https://github.com/cheyao/icepi-zero).
- **Raspberry Pi Zero form factor**: 65×30 mm; compatible with Pi Zero HATs and easy to integrate as a co-processor.

**Note on F extension:** The ECP5-25F has 24,288 LUT4s. The full RV32IMACF design with the hardware FPU enabled needs approximately 8,000–10,000 LUTs (estimated). This may be tight. If F extension is a priority, a ULX3S with the ECP5-85F (84K LUT4) provides ample headroom.

### 2.8 Resource Headroom

Compared to the current iCE40-HX8K target (61% utilization, 4,688/7,680 LUTs):

| Resource | iCE40-HX8K | ECP5-25F (iCE Pi Zero) | ECP5-45F | ECP5-85F |
|----------|-----------|----------------------|----------|----------|
| LUT4s | 7,680 | 24,288 | 43,848 | 83,640 |
| BRAM (kbit) | 128 | 194 | 351 | 352 |
| DSP18s | N/A | 28 | 56 | 156 |
| Current design headroom | 39% free | ~81% free | >89% free | >94% free |

On an ECP5-25F (iCE Pi Zero), the RV32IMAC design (F extension disabled) would use roughly **19% of available LUTs**, providing comfortable headroom for further development. Enabling the hardware FPU would increase utilization to an estimated 35–45% on the ECP5-25F.

---

## 3. Xilinx Artix-7 Target (Historical Research Only)

### 3.1 Tool Overview

Xilinx 7-Series (which includes Artix-7) was evaluated with two open source flows during research for this repository. These flows are not supported by the current project build; the supported Alchitry Au flow uses Vivado batch/TCL.

#### Option A: nextpnr-xilinx / openXC7 (Historical evaluation)

| Stage | Tool | Notes |
|-------|------|-------|
| Synthesis | Yosys (`synth_xilinx -family xc7`) | Mature |
| Place & Route | nextpnr-xilinx | Uses Project X-Ray databases |
| Bitstream | fasm2frames + xc7frames2bit | Part of Project X-Ray / openXC7 |
| Device database | Project X-Ray (prjxray) | Reverse-engineered |
| Programming | `openFPGALoader -b alchitry_au` | Apache-2.0 |
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

**For this project, nextpnr-xilinx / openXC7 is no longer supported.** This section is retained only as historical research notes from the earlier open-source evaluation.

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
    --device xc7a35tftg256-1 \
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
fasm2frames --part xc7a35tftg256-1 build/$(PROJECT).fasm build/$(PROJECT).frames

# Convert frames to bitstream
xc7frames2bit --part-file $(XRAY_DB)/xc7a35tftg256-1/part.yaml \
              --frm-file build/$(PROJECT).frames \
              --output-file build/$(PROJECT).bit

# Program via openFPGALoader (SRAM - volatile):
openFPGALoader -b alchitry_au build/$(PROJECT).bit

# Persistent flash programming:
openFPGALoader -b alchitry_au -f build/$(PROJECT).bit
```

### 3.5 PLL Replacement: `SB_PLL40_CORE` → `PLLE2_ADV` or `MMCME2_ADV`

Artix-7 provides MMCM (Mixed-Mode Clock Manager) and PLL primitives. Both are usable in the open source flow as Yosys passes them through un-modified to nextpnr-xilinx.

The **`PLLE2_ADV`** is simpler and preferred for straightforward frequency synthesis.

The Alchitry Au provides a **100 MHz** on-board oscillator (pin N14, IO_L12P_T1_MRCC_14). The following example targets 100 MHz input → 50 MHz system clock:

```systemverilog
// Alchitry Au: 100 MHz input → 50 MHz system clock
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
# Clock: 100 MHz on-board oscillator (Alchitry Au, pin N14)
create_clock -name sys_clk -period 10.0 [get_ports clk]
set_property PACKAGE_PIN N14     [get_ports clk]
set_property IOSTANDARD  LVCMOS33 [get_ports clk]

# Reset button (active low, pull-up; Alchitry Au reset pin)
set_property PACKAGE_PIN P6      [get_ports rst_n_btn]
set_property IOSTANDARD  LVCMOS33 [get_ports rst_n_btn]
set_property PULLUP true          [get_ports rst_n_btn]

# LED outputs (Alchitry Au - 8 on-board LEDs)
set_property PACKAGE_PIN K13     [get_ports {led[0]}]
set_property IOSTANDARD  LVCMOS33 [get_ports {led[0]}]
# ... additional LEDs; see official Alchitry Au XDC for full list

# UART (USB-C via FTDI FT2232HQ on Alchitry Au)
set_property PACKAGE_PIN P16     [get_ports usb_rx]
set_property IOSTANDARD  LVCMOS33 [get_ports usb_rx]
set_property PACKAGE_PIN P15     [get_ports usb_tx]
set_property IOSTANDARD  LVCMOS33 [get_ports usb_tx]
```

**Note:** The exact Alchitry Au pin assignments above are representative; always verify against the official Alchitry Au XDC constraint file or the `alchitry-au-utils` repository before use.

### 3.7 Recommended Artix-7 Board: Alchitry Au

The original research draft incorrectly listed the Alchitry Au package as CSG324. The actual Alchitry Au board uses the XC7A35T-1 in the FTG256 package, which is reflected below.

| Board | FPGA | Resources | Clock | UART | Form Factor |
|-------|------|-----------|-------|------|-------------|
| **Alchitry Au** ⭐ | XC7A35T-1 (FTG256) | 33K LUT6, 90 DSP48 | 100 MHz (N14) | USB-C (FTDI FT2232HQ) | 65×45 mm (same as Cu) |
| **Alchitry Au+** (v2) | XC7A35T-1C (CSG324) | 33K LUT6, 90 DSP48 | 100 MHz | USB-C (FTDI FT2232HQ) | 65×45 mm |
| **Arty A7-35T** (Digilent) | XC7A35T (CSG324) | 33K LUT6, 90 DSP48 | 100 MHz | USB-UART (FTDI) | 101×76 mm |
| **Arty A7-100T** (Digilent) | XC7A100T | 101K LUT6, 240 DSP48 | 100 MHz | USB-UART (FTDI) | 101×76 mm |

The **Alchitry Au** is the primary recommended target for this project:

- **Same Alchitry ecosystem as the Alchitry Cu v1**: The Au uses the same 65×45 mm board form factor, the same USB-C programming interface, and supports the same Alchitry Element shields (IO Shield, Br Shield, etc.). Transitioning from Cu to Au requires only FPGA-side changes (XDC instead of PCF, Artix-7 primitives).
- **XC7A35T-1C (33,280 LUT6)**: ~4× the equivalent logic resources of the iCE40-HX8K. The existing design fits at approximately 15% utilization.
- **FTDI FT2232HQ**: Provides both JTAG programming and USB-UART in a single USB-C cable—directly compatible with the host bus protocol used by `fpga-host`.
- **256 MB DDR3 RAM on-board**: Enables onboard DRAM instead of forwarding DRAM accesses to the host over UART, significantly improving performance.
- **100 MHz oscillator on pin N14**: Directly usable with a `PLLE2_ADV` to generate 50 MHz system clock.

### 3.8 Resource Headroom

| Resource | iCE40-HX8K | Artix-7 A35T (Alchitry Au) | Artix-7 A100T |
|----------|-----------|---------------------------|--------------|
| LUT6 equivalents | 7,680 | ~33,280 | ~101,440 |
| BRAM (kbit) | 128 | 1,800 | 4,860 |
| DSP slices | N/A | 90 (DSP48E1) | 240 |
| Current design fit | 61% | ~15% est. | ~5% est. |

The Alchitry Au (XC7A35T) is more than sufficient for the full RV32IMACF design with considerable headroom.

---

## 4. Intel/Altera Cyclone V Target (Analogue Pocket)

### 4.1 Overview and Status

> ⚠️ **Experimental / Research-Grade:** The fully open source toolchain for Cyclone V is significantly less mature than for iCE40, ECP5, or Artix-7. Most active Analogue Pocket development uses **Quartus Prime Lite** (free-to-download but proprietary). The Yosys + Mistral path described here is suitable for exploratory work and is expected to mature over time.

The **Analogue Pocket** is a handheld FPGA-based retro gaming device powered by an **Intel Cyclone V SE A5** FPGA (part number `5CSEBA6U23I7`):

- **~49,000 ALMs** (Adaptive Logic Modules; each ALM ≈ 2 LUT6-equivalent cells → ~49K LUT6 equivalents)
- **2,540 Kbit BRAM** (embedded block memory)
- **6 DSP blocks** (18×18 multipliers)
- Secondary FPGA: Altera Cyclone 10 for system management
- 4× independently addressable RAM chips; cartridge bus, link port, IR, stereo audio
- JTAG header for development

The **openFPGA** framework (https://www.analogue.co/developer) allows third-party developers to load FPGA "cores" (bitstream + JSON metadata) from SD card. This is the primary delivery mechanism for custom designs on the Pocket.

### 4.2 Tool Overview

| Stage | Tool | Status |
|-------|------|--------|
| Synthesis | Yosys (`synth_intel_alm`) | Experimental |
| Place & Route | nextpnr **Mistral** backend | Experimental |
| Bitstream | Mistral (reverse-engineered) | Partial / experimental |
| FPGA loading | SD card via openFPGA | Supported (Analogue OS 1.1+) |
| JTAG programmer | openFPGALoader or Quartus Programmer | openFPGALoader: best-effort |
| Typical commercial path | Quartus Prime Lite (free, proprietary) | Stable and recommended for production |

The **Mistral** project (https://github.com/Ravenslofty/mistral) reverse-engineers the Cyclone V bitstream format and is integrated into nextpnr as the `mistral` architecture backend. It enables a fully open pipeline, but with significant caveats.

### 4.3 Synthesis Command

```bash
yosys -p "read_verilog -sv $(SOURCES); \
          synth_intel_alm -top $(TOP_MODULE) -json $(JSON)" 2>&1 | tee build/yosys.log
```

The `synth_intel_alm` pass handles:
- ALM (Adaptive Logic Module) technology mapping for Cyclone V / Arria 10 / Stratix 10
- BRAM inference targeting `ALTSYNCRAM` primitives
- DSP inference targeting `ALTMULT_ADD` primitives
- PLL primitives passed through unchanged

**Note:** `synth_intel_alm` is the correct pass for Cyclone V. The older `synth_intel` pass targets Cyclone IV / MAX10 and should not be used for Cyclone V.

### 4.4 Place and Route Command (Mistral / nextpnr)

```bash
nextpnr-mistral \
    --device 5CSEBA6U23I7 \
    --json $(JSON) \
    --qsf $(QSF) \
    --out-of-context \
    2>&1 | tee build/nextpnr.log
```

**Building nextpnr with Mistral support:**

```bash
# Mistral device database
git clone https://github.com/Ravenslofty/mistral.git
cd mistral
cmake -DCMAKE_INSTALL_PREFIX=/usr/local .
make -j$(nproc) && sudo make install && cd ..

# nextpnr with Mistral architecture
git clone https://github.com/YosysHQ/nextpnr.git
cd nextpnr
cmake -DARCH=mistral \
      -DMISTRAL_INSTALL_PREFIX=/usr/local \
      -DCMAKE_INSTALL_PREFIX=/usr/local .
make -j$(nproc) && sudo make install && cd ..
```

### 4.5 PLL Replacement: `SB_PLL40_CORE` → `PLL_CYCLONEV` / `ALTPLL`

The Cyclone V uses its own PLL primitive. The vendor-neutral approach is to use `ALTPLL` (which Quartus and open source tools recognize), or for the Cyclone V specifically, the `PLL_CYCLONEV` black-box primitive:

```systemverilog
// Analogue Pocket: clock source is provided by the openFPGA framework
// The Pocket's openFPGA bridge provides a configurable clock to the core
// Typical core clock is 74.25 MHz (video) or a user-selected frequency
// No external PLL needed if using the openFPGA clock bridge

// If using a standalone PLL (for custom clock generation):
ALTPLL #(
    .intended_device_family("Cyclone V"),
    .inclk0_input_frequency(20000),    // 50 MHz = 20000 ps period
    .clk0_multiply_by(2),              // 50 * 2 / 2 = 50 MHz (example)
    .clk0_divide_by(2)
) pll_inst (
    .inclk  ({1'b0, clk_in}),
    .clk    ({clk_out}),
    .locked (locked)
);
```

**Important Analogue Pocket-specific note:** When targeting the Pocket via openFPGA, the core receives its clocks through the openFPGA clock bridge (`clk_74a`, `clk_74b`) rather than direct oscillator access. The FPGA top-level port list is defined by the openFPGA APF (Analogue Platform Framework) specification, not a custom pin constraint file. PLL instantiation is optional in this context.

### 4.6 Constraint Format: QSF (Quartus Settings File)

Cyclone V designs traditionally use **QSF (Quartus Settings File)** for pin assignments. In the open source Mistral flow, constraints are passed via a simplified QSF subset or nextpnr-specific JSON. For Analogue Pocket development via openFPGA, the pin constraints are **fixed by the Analogue platform specification**—developers do not write their own pin assignments; instead, they instantiate the openFPGA APF (Analogue Platform Framework) wrapper which defines the external interface.

```tcl
# Representative QSF snippet (Cyclone V standalone use)
set_location_assignment PIN_U8  -to clk
set_location_assignment PIN_Y16 -to rst_n_btn
set_location_assignment PIN_L7  -to led[0]
# ... etc.
```

For Analogue Pocket specifically, the core's top-level ports are defined by the openFPGA APF and connect to pre-assigned platform signals (video, audio, cartridge, I/O, etc.).

### 4.7 Recommended Target: Analogue Pocket (Cyclone V SE A5)

| Aspect | Analogue Pocket |
|--------|----------------|
| **FPGA** | Intel Cyclone V SE A5 (`5CSEBA6U23I7`) |
| **Logic Elements** | ~49,000 ALMs (~49K LUT6 equivalents) |
| **BRAM** | 2,540 Kbit |
| **DSP** | 6 × 18-bit multipliers |
| **Clock delivery** | openFPGA clock bridge (74.25 MHz typical) |
| **Loading mechanism** | SD card (openFPGA `.core` package) |
| **Open source toolchain** | Yosys `synth_intel_alm` + nextpnr Mistral (experimental) |
| **Typical dev toolchain** | Quartus Prime Lite (free, proprietary) |
| **JTAG** | Available via dev kit header |
| **Community support** | Active openFPGA core community |

### 4.8 Resource Headroom

| Resource | iCE40-HX8K | Cyclone V SE A5 (Pocket) |
|----------|-----------|--------------------------|
| Logic cells (LUT equiv.) | 7,680 | ~49,000 ALMs |
| BRAM (kbit) | 128 | 2,540 |
| DSP blocks | N/A | 6 (18-bit) |
| Current design fit | 61% | ~10% est. |

The Cyclone V SE A5 has sufficient resources for the full RV32IMACF design, with the F extension enabled.

### 4.9 Key Limitations of the Fully Open Source Cyclone V Flow

1. **Mistral is reverse-engineered**: Not all Cyclone V features are documented. PLLs, hard memory controllers, and some advanced routing resources may not be fully supported.
2. **No production-grade bitstream validation**: Bitstreams generated by Mistral may have correctness issues for complex designs.
3. **No timing analysis**: Mistral/nextpnr do not yet provide reliable timing closure guarantees for Cyclone V.
4. **openFPGA APF wrapper**: Using the Pocket's platform framework requires understanding the Analogue APF specification and generating a compliant `.core` package. This layer exists independently of the synthesis tool choice.
5. **Quartus Lite path is more practical**: For anyone targeting the Analogue Pocket today, Quartus Prime Lite (free to download, Windows/Linux, no license server required for Cyclone V Lite edition) is the practical choice. The open source Mistral path is best treated as a research/future investment.

---

## 5. Required Project Changes

### 5.1 Directory Structure

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
    │   ├── icepi_zero_25f.lpf # Pin constraints for iCE Pi Zero ECP5-25F (LPF)
    │   └── stub_fpu.sv        # FPU stub (shared or symlinked)
    ├── artix7/
    │   ├── fpga_top.sv        # Artix-7 specific top (PLLE2_ADV + BUFG)
    │   ├── alchitry_au.xdc    # Pin constraints for Alchitry Au (XDC)
    │   └── stub_fpu.sv        # FPU stub (shared or symlinked)
    └── cyclonev/
        ├── fpga_top.sv        # Cyclone V specific top (ALTPLL / openFPGA APF)
        ├── analogue_pocket.qsf # Platform constraints (openFPGA APF defined)
        └── stub_fpu.sv        # FPU stub (shared or symlinked)
```

### 5.2 Multi-Target Makefile Design

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
  PROGRAM_CMD   = openFPGALoader -b ice40_generic -f $(BIN)
  TARGET_FREQ   = 25
  OUTPUT_EXT    = bin
endif

ifeq ($(TARGET), ecp5_icepi_zero)
  DEVICE        = 25k
  PACKAGE       = CABGA256
  FPGA_DIR      = ecp5
  CONSTRAINT    = $(FPGA_DIR)/icepi_zero_25f.lpf
  SYNTH_CMD     = synth_ecp5 -top $(TOP_MODULE) -json $(JSON)
  PNR_CMD       = nextpnr-ecp5 --$(DEVICE) --package $(PACKAGE) \
                    --json $(JSON) --lpf $(CONSTRAINT) --textcfg $(CONFIG) --freq $(TARGET_FREQ)
  PACK_CMD      = ecppack $(CONFIG) $(BIN)
  PROGRAM_CMD   = openFPGALoader -b icepi_zero $(BIN)
  TARGET_FREQ   = 50
  OUTPUT_EXT    = bit
endif

ifeq ($(TARGET), artix7_alchitry_au)
  DEVICE        = xc7a35tftg256-1
  FPGA_DIR      = artix7
  CONSTRAINT    = $(FPGA_DIR)/alchitry_au.xdc
  SYNTH_CMD     = synth_xilinx -family xc7 -top $(TOP_MODULE) -edif $(EDIF)
  PNR_CMD       = nextpnr-xilinx --chipdb $(CHIPDB) \
                    --xdc $(CONSTRAINT) --edif $(EDIF) --fasm $(FASM) --freq $(TARGET_FREQ)
  PACK_CMD      = fasm2frames --part $(DEVICE) $(FASM) $(FRAMES) && \
                  xc7frames2bit --part-file $(XRAY_DB)/$(DEVICE)/part.yaml \
                                --frm-file $(FRAMES) --output-file $(BIN)
  PROGRAM_CMD   = openFPGALoader -b alchitry_au $(BIN)
  TARGET_FREQ   = 50
  OUTPUT_EXT    = bit
endif

ifeq ($(TARGET), cyclonev_analogue_pocket)
  DEVICE        = 5CSEBA6U23I7
  FPGA_DIR      = cyclonev
  CONSTRAINT    = $(FPGA_DIR)/analogue_pocket.qsf
  SYNTH_CMD     = synth_intel_alm -top $(TOP_MODULE) -json $(JSON)
  PNR_CMD       = nextpnr-mistral --device $(DEVICE) \
                    --json $(JSON) --qsf $(CONSTRAINT) --out-of-context
  PACK_CMD      = mistral-bitgen $(PNR_OUT) $(BIN)
  PROGRAM_CMD   = @echo "Load $(BIN) via SD card using openFPGA .core package"
  TARGET_FREQ   = 50
  OUTPUT_EXT    = rbf
endif

# ============================================================
# Usage: make TARGET=ecp5_icepi_zero
#        make TARGET=artix7_alchitry_au
#        make TARGET=cyclonev_analogue_pocket
#        make            (defaults to ice40hx8k)
# ============================================================
```

### 5.3 FPGA Top-Level Wrapper Changes

Each target needs its own `fpga_top.sv` that handles only:
1. PLL instantiation (vendor-specific primitive)
2. Reset synchronization (logic using `ff_sync.sv` is already vendor-neutral)
3. Port list matching the board's pin constraint file

The instantiation of `top` (CPU core), `uart` (host bus), and peripheral wiring is **identical** across all three wrappers. This can be factored out into a shared SystemVerilog include or kept as a small repetition.

**Template structure common to all three wrappers:**

```systemverilog
// Only this block is target-specific:
// ==========================================
<VENDOR_PLL> pll_inst ( ... ); // iCE40:    SB_PLL40_CORE
                                // ECP5:     EHXPLLL
                                // Artix-7:  PLLE2_ADV + BUFG
                                // Cyclone V: ALTPLL or openFPGA clock bridge
// ==========================================

// Everything below is identical across targets:
top #( .CLK_FREQ_HZ(SYS_CLK_HZ), ... ) cpu_inst ( ... );
uart #( .CLK_FREQ_HZ(SYS_CLK_HZ), ... ) host_uart_inst ( ... );
// LED, button, and seven-segment logic ...
```

The only other change is updating `CLK_FREQ_HZ` and `BAUD_RATE` parameters to match the target clock frequency (e.g., 50 MHz for ECP5/Artix-7 vs. 25 MHz for iCE40).

---

## 6. Toolchain Installation

### 6.1 ECP5 Tools (Stable)

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

### 6.2 Artix-7 Tools via openXC7 (Historical, Unsupported)

The **openXC7 toolchain-installer** (https://github.com/openXC7/toolchain-installer) was part of the historical open-source evaluation for Artix-7. It is not part of the supported repository flow now that the Alchitry Au target uses Vivado batch mode.

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
python3 xilinx/python/bbaexport.py --device xc7a35tftg256-1 --bba bba/xc7a35t.bba
bbasm --l bba/xc7a35t.bba bba/xc7a35t.bin
make -j$(nproc) && sudo make install && cd ..
```

**Note on maturity:** The openXC7 project maintains pre-built toolchain releases. Building nextpnr-xilinx from source is considerably more involved than building nextpnr-ice40 or nextpnr-ecp5 because it requires pre-generated chipdb files derived from Project X-Ray databases.

### 6.3 Cyclone V Tools via Mistral (Experimental)

The Cyclone V Mistral flow requires building nextpnr from source with Mistral support (see Section 4.4 for build commands). There are no pre-packaged apt/brew/conda packages for nextpnr-mistral as of early 2026.

```bash
# OSS CAD Suite (bundles Yosys; Mistral not always included - check release notes)
# https://github.com/YosysHQ/oss-cad-suite-build/releases
# Download and extract the appropriate release for your platform
source <oss-cad-suite>/environment

# Verify synth_intel_alm is available:
yosys -p "help synth_intel_alm"

# Build nextpnr with Mistral from source (see Section 4.4)
```

**Alternative (practical):** For production Analogue Pocket development, install **Quartus Prime Lite** (free):

```bash
# Download from Intel FPGA Download Center:
# https://www.intel.com/content/www/us/en/software-kit/757270/intel-quartus-prime-lite-edition-design-software-version-23-1-for-linux.html
# Supports Cyclone V; no license required for Lite edition
```

---

## 7. Primitive and Vendor IP Migration

### 7.1 Summary of Vendor Primitives by Target

| Primitive Function | iCE40-HX8K | ECP5 | Artix-7 | Cyclone V |
|-------------------|-----------|------|---------|-----------|
| **PLL** | `SB_PLL40_CORE` | `EHXPLLL` | `PLLE2_ADV` or `MMCME2_ADV` | `ALTPLL` / openFPGA clock bridge |
| **Global Clock Buffer** | (implicit in PLL) | (implicit) | `BUFG` (explicit required) | (implicit) |
| **Block RAM** | `SB_RAM40_4K` (inferred) | `DP16KD` (inferred) | `RAMB36E1` / `RAMB18E1` (inferred) | `ALTSYNCRAM` (inferred) |
| **DSP** | N/A | `MULT18X18D` (inferred) | `DSP48E1` (inferred) | `ALTMULT_ADD` (inferred) |
| **I/O Buffer** | `SB_IO` (optional) | (automatic) | (automatic) | (automatic) |

For BRAM and DSP, all four synthesis commands (`synth_ice40`, `synth_ecp5`, `synth_xilinx`, `synth_intel_alm`) **infer** technology-specific primitives automatically from SystemVerilog `always_ff` BRAM patterns and `*` multiply operators. No manual changes to `regfile.sv`, `sync_dpram.sv`, or `mul_unit.sv` are needed.

The only **manual intervention** required is the PLL instance, which cannot be inferred and is already manually instantiated in `fpga_top.sv`. A new per-target top wrapper handles this.

### 7.2 Reset Synchronizer (`ff_sync.sv`)

The existing `ff_sync.sv` module uses standard `always_ff` and is fully vendor-neutral. No changes are needed. All four flows will synthesize it correctly.

### 7.3 FPU Stub (`stub_fpu.sv`)

The `stub_fpu.sv` module is already vendor-neutral SystemVerilog. The same stub can be shared across all targets (or softlinked). If the F extension is enabled, the actual FPU implementation in `rtl/common/fpu/` is also vendor-neutral.

---

## 8. Timing Considerations

### 8.1 Expected Fmax by Target

The current iCE40 design achieves **34.91 MHz Fmax** at the 25 MHz target. For ECP5, Artix-7, and Cyclone V:

| Metric | iCE40-HX8K | ECP5-25F (expected) | Artix-7 (expected) | Cyclone V (expected) |
|--------|-----------|--------------------|--------------------|----------------------|
| **LUT technology** | 4-input | 4-input | 6-input | 8-input ALM |
| **Carry chain speed** | ~1.2 ns/bit | ~0.7 ns/bit | ~0.5 ns/bit | ~0.4 ns/bit |
| **Routing fabric** | Limited | Rich | Rich | Rich |
| **Expected Fmax** | 34.91 MHz (achieved) | 60–80 MHz (est.) | 80–120 MHz (est.) | 80–120 MHz (est.) |
| **Recommended target** | 25 MHz | 50 MHz | 50–100 MHz | 50 MHz (openFPGA clock) |

The critical path identified in `rtl/fpga/SYNTHESIS_ANALYSIS.md`—through the 32-bit ALU carry chain then `host_bus_mux` and `registered_bus.sv`—will be significantly shorter on ECP5, Artix-7, and Cyclone V due to faster carry chains and better routing resources.

### 8.2 UART Baud Rate Scaling

The host bus UART (`uart.sv`) is parameterized by `CLK_FREQ_HZ` and `BAUD_RATE`. When moving to a higher system clock (e.g., 50 MHz), updating the `CLK_FREQ_HZ` parameter in the `fpga_top.sv` wrapper is sufficient. The baud rate divisor is computed at elaboration time. The host-side `fpga-host` crate also needs to match the configured baud rate.

**Note for Analogue Pocket:** The Pocket communicates with the host via the openFPGA bridge, not UART. The `host_bus_interface` UART layer would be replaced by the openFPGA APF data bridge for Pocket-specific builds.

---

## 9. CI Workflow Integration

The existing CI workflow (`ci.yml`) runs FPGA synthesis as a verification step. To support multiple targets, the CI job can be parameterised using a matrix strategy:

```yaml
jobs:
  fpga-synthesis:
    strategy:
      matrix:
        target: [ice40hx8k, ecp5_icepi_zero, artix7_alchitry_au]
      fail-fast: false
    steps:
      - name: Install tools
        run: |
          sudo apt-get install -y yosys
          # Install target-specific tools based on matrix.target
          if [ "${{ matrix.target }}" = "ice40hx8k" ]; then
            sudo apt-get install -y fpga-icestorm nextpnr-ice40
          elif [ "${{ matrix.target }}" = "ecp5_icepi_zero" ]; then
            sudo apt-get install -y prjtrellis nextpnr-ecp5 openfpgaloader
          fi
          # Note: artix7 and cyclonev targets require manual tool install
      - name: Synthesize
        run: cd rtl/fpga && make TARGET=${{ matrix.target }}
```

**Note:** The Artix-7 target is excluded from the open-source CI matrix described here because the supported repository flow now uses Vivado for Alchitry Au builds. The Cyclone V (Analogue Pocket) target is excluded entirely from CI due to its experimental status. The ECP5 and iCE40 targets can be run in CI using system packages.

---

## 10. Trade-offs and Recommendations

### 10.1 ECP5 (iCE Pi Zero) vs. iCE40 Comparison

| Aspect | iCE40-HX8K (Alchitry Cu) | ECP5-25F (iCE Pi Zero) |
|--------|--------------------------|------------------------|
| **Open source maturity** | ⭐⭐⭐⭐⭐ (Very mature) | ⭐⭐⭐⭐⭐ (Very mature) |
| **Resources** | 7,680 LUT4 | 24,288 LUT4 (~3×) |
| **F extension feasibility** | ❌ Too resource-constrained | ⚠️ Feasible but tight (~40%) |
| **Higher clock speeds** | Limited (~35 MHz) | Good (60–80 MHz est.) |
| **DSP blocks** | ❌ None | ✅ 28 (useful for mul_unit.sv) |
| **Tool complexity delta** | Baseline | Minimal – same flow |
| **Board form factor** | 65×45 mm | 65×30 mm (Pi Zero) |
| **Cost** | ~$50 | ~$30–40 |

**Verdict:** The iCE Pi Zero (ECP5-25F) is a significant step up from the iCE40 with minimal toolchain overhead and lower cost. Strongly recommended as the first new target.

### 10.2 Artix-7 (Alchitry Au) vs. ECP5 Comparison

| Aspect | ECP5-25F (iCE Pi Zero) | Artix-7 A35T (Alchitry Au) |
|--------|------------------------|---------------------------|
| **Open source P&R maturity** | ⭐⭐⭐⭐⭐ (Production) | ⭐⭐⭐ (Experimental) |
| **Resources** | 24K LUT4 | 33K LUT6 (~4× equiv.) |
| **LUT technology** | 4-input | 6-input (more efficient) |
| **DSP blocks** | 28 | 90 (DSP48E1) |
| **On-board DDR3 RAM** | ❌ | ✅ 256MB (Alchitry Au) |
| **Tool install complexity** | Low (apt packages) | High (openXC7 manual install) |
| **CI integration ease** | Easy | Harder |
| **Board ecosystem** | Pi Zero HATs | Alchitry Elements (shared with Cu) |
| **Cost** | ~$30–40 | ~$100 |

**Verdict:** The Alchitry Au's shared ecosystem with the existing Alchitry Cu makes the hardware transition seamless. The on-board DDR3 RAM is a major capability addition. Recommend pursuing after ECP5 is stable.

### 10.3 Cyclone V (Analogue Pocket) Assessment

| Aspect | Cyclone V SE A5 (Analogue Pocket) |
|--------|-----------------------------------|
| **Open source P&R maturity** | ⭐⭐ (Experimental – Mistral) |
| **Resources** | ~49K ALMs |
| **Deployment mechanism** | SD card via openFPGA |
| **Typical dev toolchain** | Quartus Prime Lite (free, proprietary) |
| **Open source toolchain** | Yosys `synth_intel_alm` + nextpnr Mistral |
| **CI integration** | Not feasible (experimental tools, no packaged install) |
| **Unique value** | FPGA gaming handheld platform; strong community |

**Verdict:** The Analogue Pocket is a compelling platform for RISC-V CPU cores targeting retro gaming use cases. The fully open source path (Mistral) is research-grade; for practical results today, Quartus Prime Lite is the practical choice. Treat as a Phase 3 / experimental target.

### 10.4 Recommended Implementation Order

1. **Phase 1: ECP5 (iCE Pi Zero)** – High value, low risk
   - Target board: iCE Pi Zero (ECP5-25F)
   - Effort: Low (tool parity with existing flow, apt-installable)
   - 3× more logic than iCE40; same synthesis/P&R toolchain philosophy

2. **Phase 2: Artix-7 (Alchitry Au)** – Historical research path
   - Target board: Alchitry Au (XC7A35T)
   - Historical effort estimate: Medium (openXC7 install, PLLE2_ADV wrapper, XDC constraints)
   - The repository no longer pursues this open-source path; the supported Au build now uses Vivado batch/TCL instead

3. **Phase 3: Cyclone V (Analogue Pocket)** – Exploratory / research
   - Target device: 5CSEBA6U23I7 (49K ALMs)
   - Effort: High (Mistral experimental, openFPGA APF integration)
   - Unique use case: RISC-V games console core on retro gaming handheld
   - Wait for Mistral/nextpnr-mistral to mature before investing heavily

---

## 11. Proof-of-Concept Makefile Snippet

Below is a self-contained proof-of-concept for how the multi-target Makefile would drive the ECP5 synthesis for the iCE Pi Zero, demonstrating the minimal changes relative to the current iCE40 flow:

```makefile
# ECP5 synthesis for iCE Pi Zero (add alongside existing ice40 targets)
.PHONY: synth-ecp5
synth-ecp5: $(BUILD_DIR)
	bash -e -o pipefail -c '$(YOSYS) -p \
	    "read_verilog -sv $(RTL_SOURCES) ecp5/fpga_top.sv ecp5/stub_fpu.sv; \
	     synth_ecp5 -top fpga_top -json $(BUILD_DIR)/$(PROJECT)_ecp5.json" \
	     2>&1 | tee $(BUILD_DIR)/yosys_ecp5.log'

.PHONY: pnr-ecp5
pnr-ecp5: synth-ecp5
	bash -e -o pipefail -c 'nextpnr-ecp5 \
	    --25k --package CABGA256 \
	    --json $(BUILD_DIR)/$(PROJECT)_ecp5.json \
	    --lpf ecp5/icepi_zero_25f.lpf \
	    --textcfg $(BUILD_DIR)/$(PROJECT)_ecp5.config \
	    --freq 50 \
	    2>&1 | tee $(BUILD_DIR)/nextpnr_ecp5.log'

.PHONY: bitstream-ecp5
bitstream-ecp5: pnr-ecp5
	ecppack $(BUILD_DIR)/$(PROJECT)_ecp5.config $(BUILD_DIR)/$(PROJECT)_ecp5.bit
```

This mirrors the existing iCE40 targets exactly, with `synth_ecp5`, `nextpnr-ecp5 --25k`, and `ecppack` substituted for their iCE40 counterparts. The only device-specific changes are the `--25k --package CABGA256` flags (for the iCE Pi Zero's LFE5U-25F) and the LPF constraint file.

---

## 12. References

### Tool Repositories

- **Yosys**: https://github.com/YosysHQ/yosys (synthesis, `synth_ice40` / `synth_ecp5` / `synth_xilinx` / `synth_intel_alm`)
- **nextpnr**: https://github.com/YosysHQ/nextpnr (P&R, supports ice40, ecp5, mistral)
- **nextpnr-xilinx**: https://github.com/gatecat/nextpnr-xilinx (P&R for Xilinx 7-Series)
- **IceStorm**: https://github.com/YosysHQ/icestorm (iCE40 bitstream tools)
- **Project Trellis**: https://github.com/YosysHQ/prjtrellis (ECP5 device database and bitstream tools)
- **Project X-Ray**: https://github.com/f4pga/prjxray (Xilinx 7-Series device database)
- **openXC7**: https://github.com/openxc7 (Xilinx open-source toolchain packaging)
- **F4PGA**: https://f4pga.org (Open FPGA umbrella, VPR-based Xilinx flow)
- **Mistral**: https://github.com/Ravenslofty/mistral (Cyclone V reverse-engineered bitstream, nextpnr backend)
- **openFPGALoader**: https://github.com/trabucayre/openFPGALoader (universal FPGA programmer)

### Board Documentation

- **iCE Pi Zero (ECP5-25F)**: https://github.com/cheyao/icepi-zero
- **ULX3S (ECP5-85F, alternative)**: https://ulx3s.github.io
- **Alchitry Au (Artix-7)**: https://alchitry.com/boards/au/
- **Analogue Pocket (Cyclone V)**: https://www.analogue.co/developer/docs/overview

### Technical References

- **ECP5 PLL Guide (Project F)**: https://projectf.io/posts/ecp5-fpga-clock/
- **ecppll usage**: https://github.com/YosysHQ/prjtrellis (tools/ecppll)
- **Yosys synth_ecp5 docs**: https://yosyshq.readthedocs.io/projects/yosys/en/stable/cmd/synth_ecp5.html
- **Yosys synth_xilinx docs**: https://yosyshq.readthedocs.io/projects/yosys/en/0.46/cmd/synth_xilinx.html
- **Yosys synth_intel_alm docs**: https://yosyshq.readthedocs.io/projects/yosys/en/stable/cmd/synth_intel_alm.html
- **Project Trellis documentation**: https://prjtrellis.readthedocs.io
- **Mistral documentation**: https://github.com/Ravenslofty/mistral/blob/master/README.md
- **openFPGA / Analogue Pocket developer docs**: https://www.analogue.co/developer/docs/overview
