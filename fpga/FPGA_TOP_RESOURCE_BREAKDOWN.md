# FPGA RTL Resource Breakdown (ICE40 + ECP5)

Generated: 2026-03-08 18:27 UTC

## Scope

- Re-runs synthesis/resource analysis on the latest merged branch state for **both** FPGA targets:
  - `TARGET=ice40_alchitry_cu` (iCE40 HX8K)
  - `TARGET=ecp5_icepi_zero` (ECP5-25F)
- Uses target build outputs from `rtl/fpga/build/<target>/` and hierarchical Yosys stats (`-noflatten`) for module attribution.

## Target: ice40_alchitry_cu (iCE40-HX8K)

| Resource | Used | Available | Utilization | Source |
|---|---:|---:|---:|---|
| Logic Cells | 5679 | 7680 | 73.9% | nextpnr |
| Block RAM | 30 | 32 | 93.8% | nextpnr |
| Global Buffers | 8 | 8 | 100.0% | nextpnr |
| IO Blocks | 77 | 256 | 30.1% | nextpnr |
| PLL | 1 | 2 | 50.0% | nextpnr |
| SB_LUT4 (hierarchical mapped LUTs) | 5292 | n/a | n/a | Yosys |
| Total DFF-family cells (hierarchical) | 2360 | n/a | n/a | Yosys |
| SB_CARRY (hierarchical) | 897 | n/a | n/a | Yosys |
| SB_RAM40_4K (hierarchical) | 30 | 32 | 93.8% | Yosys |

- Post-route Fmax (`pll_clk_global`): **40.32 MHz**

### ICE40 Hierarchical Breakdown: board top (`ice40_alchitry_cu_top`)

| Area | Instances | SB_LUT4 | Share | DFF cells | Carry cells | RAM blocks |
|---|---:|---:|---:|---:|---:|---:|
| fpga_common_top | 1 | 5206 | 98.4% | 2325 | 859 | 30 |
| ff_sync | 2 | 1 | 0.0% | 2 | 0 | 0 |
| local glue logic | 1 | 84 | 1.6% | 31 | 38 | 0 |

### ICE40 Hierarchical Breakdown: `fpga_common_top`

| Area | Instances | SB_LUT4 | Share | DFF cells | Carry cells | RAM blocks |
|---|---:|---:|---:|---:|---:|---:|
| top | 1 | 5082 | 97.6% | 2237 | 822 | 30 |
| uart | 1 | 124 | 2.4% | 88 | 37 | 0 |
| local glue logic | 1 | 0 | 0.0% | 0 | 0 | 0 |

### ICE40 Hierarchical Breakdown: `rtl/common/top.sv`

| Area | Instances | SB_LUT4 | Share | DFF cells | Carry cells | RAM blocks |
|---|---:|---:|---:|---:|---:|---:|
| cpu | 1 | 2712 | 53.4% | 874 | 402 | 6 |
| host_bus_interface | 1 | 650 | 12.8% | 561 | 137 | 0 |
| registered_bus | 1 | 620 | 12.2% | 111 | 3 | 0 |
| sram_peripheral | 1 | 544 | 10.7% | 165 | 10 | 24 |
| clock_peripheral | 1 | 211 | 4.2% | 154 | 131 | 0 |
| sys_led_controller | 1 | 173 | 3.4% | 124 | 98 | 0 |
| system_controller | 1 | 64 | 1.3% | 102 | 0 | 0 |
| host_bus_mux | 1 | 51 | 1.0% | 103 | 0 | 0 |
| reset_controller | 1 | 37 | 0.7% | 26 | 41 | 0 |
| led_controller_peripheral | 1 | 15 | 0.3% | 17 | 0 | 0 |
| local glue logic | 1 | 5 | 0.1% | 0 | 0 | 0 |

### ICE40 Hierarchical Breakdown: `cpu`

| Area | Instances | SB_LUT4 | Share | DFF cells | Carry cells | RAM blocks |
|---|---:|---:|---:|---:|---:|---:|
| alu | 1 | 814 | 30.0% | 0 | 95 | 0 |
| csr_file | 1 | 317 | 11.7% | 97 | 60 | 2 |
| writeback_mux | 1 | 263 | 9.7% | 0 | 59 | 0 |
| decompress | 1 | 243 | 9.0% | 0 | 0 | 0 |
| mem_interface | 1 | 93 | 3.4% | 0 | 0 | 0 |
| branch_unit | 1 | 62 | 2.3% | 0 | 64 | 0 |
| decoder | 1 | 52 | 1.9% | 0 | 0 | 0 |
| fetch_buffer | 1 | 40 | 1.5% | 18 | 0 | 0 |
| regfile | 1 | 2 | 0.1% | 64 | 0 | 4 |
| local glue logic | 1 | 826 | 30.5% | 695 | 124 | 0 |

### ICE40 ALU Detailed Breakdown

| Area | Instances | SB_LUT4 | Share | DFF cells | Carry cells | RAM blocks |
|---|---:|---:|---:|---:|---:|---:|
| local glue logic | 1 | 814 | 100.0% | 0 | 95 | 0 |

## Target: ecp5_icepi_zero (ECP5-25F)

| Resource | Used | Available | Utilization | Source |
|---|---:|---:|---:|---|
| Combinational cells (packed) | 10320 | 24288 | 42.5% | nextpnr |
| Flip-flops (packed) | 2377 | 24288 | 9.8% | nextpnr |
| Block RAM (DP16KD) | 9 | 56 | 16.1% | nextpnr |
| IO | 5 | 197 | 2.5% | nextpnr |
| Clock distribution | 1 | 56 | 1.8% | nextpnr |
| PLL | 0 | 2 | 0.0% | nextpnr |
| Hierarchical logic proxy (`LUT4 + PFUMX + L6MUX21 + CCU2C`) | 14835 | n/a | n/a | Yosys |
| TRELLIS_FF (hierarchical mapped) | 2583 | n/a | n/a | Yosys |
| DP16KD (hierarchical mapped) | 9 | 56 | 16.1% | Yosys |

- Post-route Fmax (`$glbnet$clk$TRELLIS_IO_IN`): **53.98 MHz**

### ECP5 Hierarchical Breakdown: board top (`ecp5_icepi_zero_top`)

| Area | Instances | Logic proxy (`LUT4 + PFUMX + L6MUX21 + CCU2C`) | Share | TRELLIS_FF | CCU2C | DP16KD |
|---|---:|---:|---:|---:|---:|---:|
| fpga_common_top | 1 | 14833 | 100.0% | 2581 | 741 | 9 |
| ff_sync | 1 | 2 | 0.0% | 2 | 0 | 0 |
| local glue logic | 1 | 0 | 0.0% | 0 | 0 | 0 |

### ECP5 Hierarchical Breakdown: `fpga_common_top`

| Area | Instances | Logic proxy (`LUT4 + PFUMX + L6MUX21 + CCU2C`) | Share | TRELLIS_FF | CCU2C | DP16KD |
|---|---:|---:|---:|---:|---:|---:|
| top | 1 | 14597 | 98.4% | 2493 | 724 | 9 |
| uart | 1 | 236 | 1.6% | 88 | 17 | 0 |
| local glue logic | 1 | 0 | 0.0% | 0 | 0 | 0 |

### ECP5 Hierarchical Breakdown: `rtl/common/top.sv`

| Area | Instances | Logic proxy (`LUT4 + PFUMX + L6MUX21 + CCU2C`) | Share | TRELLIS_FF | CCU2C | DP16KD |
|---|---:|---:|---:|---:|---:|---:|
| cpu | 1 | 9901 | 67.8% | 1124 | 406 | 3 |
| host_bus_interface | 1 | 1265 | 8.7% | 561 | 79 | 0 |
| registered_bus | 1 | 1245 | 8.5% | 111 | 81 | 0 |
| sram_peripheral | 1 | 761 | 5.2% | 164 | 6 | 6 |
| clock_peripheral | 1 | 609 | 4.2% | 155 | 73 | 0 |
| host_bus_mux | 1 | 346 | 2.4% | 103 | 0 | 0 |
| sys_led_controller | 1 | 181 | 1.2% | 129 | 57 | 0 |
| system_controller | 1 | 164 | 1.1% | 102 | 0 | 0 |
| reset_controller | 1 | 77 | 0.5% | 27 | 22 | 0 |
| led_controller_peripheral | 1 | 43 | 0.3% | 17 | 0 | 0 |
| local glue logic | 1 | 5 | 0.0% | 0 | 0 | 0 |

### ECP5 Hierarchical Breakdown: `cpu`

| Area | Instances | Logic proxy (`LUT4 + PFUMX + L6MUX21 + CCU2C`) | Share | TRELLIS_FF | CCU2C | DP16KD |
|---|---:|---:|---:|---:|---:|---:|
| alu | 1 | 4734 | 47.8% | 250 | 247 | 0 |
| writeback_mux | 1 | 937 | 9.5% | 0 | 31 | 0 |
| csr_file | 1 | 765 | 7.7% | 97 | 32 | 1 |
| decompress | 1 | 664 | 6.7% | 0 | 0 | 0 |
| mem_interface | 1 | 235 | 2.4% | 0 | 0 | 0 |
| decoder | 1 | 120 | 1.2% | 0 | 0 | 0 |
| branch_unit | 1 | 91 | 0.9% | 0 | 32 | 0 |
| fetch_buffer | 1 | 57 | 0.6% | 18 | 0 | 0 |
| regfile | 1 | 0 | 0.0% | 64 | 0 | 2 |
| local glue logic | 1 | 2298 | 23.2% | 695 | 64 | 0 |

### ECP5 ALU Detailed Breakdown

| Area | Instances | Logic proxy (`LUT4 + PFUMX + L6MUX21 + CCU2C`) | Share | TRELLIS_FF | CCU2C | DP16KD |
|---|---:|---:|---:|---:|---:|---:|
| div_unit | 1 | 984 | 20.8% | 142 | 115 | 0 |
| mul_unit | 1 | 965 | 20.4% | 108 | 84 | 0 |
| local glue logic | 1 | 2785 | 58.8% | 0 | 48 | 0 |

## Notes

- iCE40 and ECP5 use different primitive vocabularies (`SB_*` vs `LUT4/PFUMX/L6MUX21/CCU2C` + `TRELLIS_FF/DP16KD`), so area columns are target-specific.
- ECP5 nextpnr reports packed utilization as `TRELLIS_COMB`; the hierarchical Yosys proxy sums mapped logic primitives `LUT4 + PFUMX + L6MUX21 + CCU2C`.
- Hierarchical rows include descendants; local glue rows represent primitives instantiated directly in the named module.
