# FPGA Top RTL Resource Breakdown

Generated: 2026-02-26 20:23 UTC

## Scope

- Analyzes the current FPGA synthesis path rooted at `fpga/fpga_top.sv` plus the instantiated RTL under `rtl/top.sv`.
- Resource attribution is taken from hierarchical Yosys synthesis (`synth_ice40 -noflatten`) and final device packing from nextpnr.

## Full-Design Utilization (current fpga build)

| Resource | Used | Available | Utilization | Source |
|---|---:|---:|---:|---|
| Logic Cells | 7006 | 7680 | 91.2% | nextpnr |
| Block RAM | 28 | 32 | 87.5% | nextpnr |
| Global Buffers | 7 | 8 | 87.5% | nextpnr |
| IO Blocks | 77 | 256 | 30.1% | nextpnr |
| PLL | 1 | 2 | 50.0% | nextpnr |
| SB_LUT4 (hierarchical mapped LUTs) | 6270 | n/a | n/a | Yosys |
| Total DFF-family cells (hierarchical) | 2293 | n/a | n/a | Yosys |
| SB_CARRY (hierarchical) | 1504 | n/a | n/a | Yosys |
| SB_RAM40_4K (hierarchical) | 28 | 32 | 87.5% | Yosys |

- Achieved `pll_clk_global` Fmax: **29.31 MHz** (target: 25 MHz)

## fpga_top-Level Breakdown (hierarchical)

| Area under `fpga_top` | Instances | SB_LUT4 | LUT Share of fpga_top | DFF cells | SB_CARRY | SB_RAM40_4K |
|---|---:|---:|---:|---:|---:|---:|
| top | 1 | 6075 | 96.9% | 2170 | 1429 | 28 |
| uart | 1 | 111 | 1.8% | 88 | 37 | 0 |
| ff_sync | 2 | 1 | 0.0% | 2 | 0 | 0 |
| fpga_top local glue (PLL/reset/IO) | 1 | 82 | 1.3% | 31 | 38 | 0 |

## rtl/top.sv Breakdown (hierarchical)

| Area under `top` | Instances | SB_LUT4 | LUT Share of `top` | DFF cells | SB_CARRY | SB_RAM40_4K |
|---|---:|---:|---:|---:|---:|---:|
| cpu | 1 | 4337 | 71.4% | 1406 | 839 | 4 |
| sram_peripheral | 1 | 501 | 8.2% | 163 | 10 | 24 |
| sys_led_controller | 1 | 267 | 4.4% | 124 | 98 | 0 |
| host_bus_interface | 1 | 260 | 4.3% | 251 | 0 | 0 |
| clock_peripheral | 1 | 207 | 3.4% | 121 | 131 | 0 |
| bus | 1 | 172 | 2.8% | 0 | 248 | 0 |
| bus_arbiter | 1 | 138 | 2.3% | 2 | 0 | 0 |
| host_bus_mux | 1 | 81 | 1.3% | 0 | 62 | 0 |
| system_controller | 1 | 59 | 1.0% | 69 | 0 | 0 |
| reset_controller | 1 | 37 | 0.6% | 26 | 41 | 0 |
| led_controller_peripheral | 1 | 11 | 0.2% | 8 | 0 | 0 |
| top local glue logic | 1 | 5 | 0.1% | 0 | 0 | 0 |

## CPU Core Breakdown (hierarchical)

| Area under `cpu` | Instances | SB_LUT4 | LUT Share of `cpu` | DFF cells | SB_CARRY | SB_RAM40_4K |
|---|---:|---:|---:|---:|---:|---:|
| alu | 1 | 2205 | 50.8% | 246 | 468 | 0 |
| csr_file | 1 | 568 | 13.1% | 448 | 124 | 0 |
| writeback_mux | 1 | 263 | 6.1% | 0 | 59 | 0 |
| decompress | 1 | 240 | 5.5% | 0 | 0 | 0 |
| mem_interface | 1 | 93 | 2.1% | 0 | 0 | 0 |
| branch_unit | 1 | 62 | 1.4% | 0 | 64 | 0 |
| decoder | 1 | 59 | 1.4% | 0 | 0 | 0 |
| fetch_buffer | 1 | 40 | 0.9% | 18 | 0 | 0 |
| regfile | 1 | 2 | 0.0% | 0 | 0 | 4 |
| cpu local control/FSM/staging | 1 | 805 | 18.6% | 694 | 124 | 0 |

## Highest LUT Consumers (module view)

| Module | SB_LUT4 | Share of fpga_top LUTs | DFF cells | SB_CARRY | SB_RAM40_4K |
|---|---:|---:|---:|---:|---:|
| fpga_top | 6270 | 100.0% | 2293 | 1504 | 28 |
| top | 6075 | 96.9% | 2170 | 1429 | 28 |
| cpu | 4337 | 69.2% | 1406 | 839 | 4 |
| alu | 2205 | 35.2% | 246 | 468 | 0 |
| div_unit | 779 | 12.4% | 140 | 216 | 0 |
| csr_file | 568 | 9.1% | 448 | 124 | 0 |
| mul_unit | 538 | 8.6% | 106 | 157 | 0 |
| sram_peripheral | 501 | 8.0% | 163 | 10 | 24 |
| sys_led_controller | 267 | 4.3% | 124 | 98 | 0 |
| writeback_mux | 263 | 4.2% | 0 | 59 | 0 |
| host_bus_interface | 260 | 4.1% | 251 | 0 | 0 |
| decompress | 240 | 3.8% | 0 | 0 | 0 |

## Notes

- `SB_LUT4` is the primary logic-area proxy for module attribution; nextpnr packed LC usage is listed separately.
- Hierarchical counts include all descendants under each module/area.
- Parameter-specialized variants are normalized to their base module names in tables.
