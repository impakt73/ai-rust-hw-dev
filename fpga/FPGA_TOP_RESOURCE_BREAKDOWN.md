# FPGA Top RTL Resource Breakdown

Generated: 2026-03-02 15:57 UTC

## Scope

- Analyzes the current FPGA synthesis path rooted at `rtl/fpga/fpga_top.sv` plus the instantiated RTL under `rtl/common/top.sv`.
- Resource attribution is taken from hierarchical Yosys synthesis (`synth_ice40 -noflatten`) and final device packing from nextpnr.

## Full-Design Utilization (current fpga build)

| Resource | Used | Available | Utilization | Source |
|---|---:|---:|---:|---|
| Logic Cells | 6966 | 7680 | 90.7% | nextpnr |
| Block RAM | 30 | 32 | 93.8% | nextpnr |
| Global Buffers | 8 | 8 | 100.0% | nextpnr |
| IO Blocks | 77 | 256 | 30.1% | nextpnr |
| PLL | 1 | 2 | 50.0% | nextpnr |
| SB_LUT4 (hierarchical mapped LUTs) | 6441 | n/a | n/a | Yosys |
| Total DFF-family cells (hierarchical) | 2317 | n/a | n/a | Yosys |
| SB_CARRY (hierarchical) | 1577 | n/a | n/a | Yosys |
| SB_RAM40_4K (hierarchical) | 30 | 32 | 93.8% | Yosys |

- Achieved `pll_clk_global` Fmax: **37.58 MHz** (target: 25 MHz)

## fpga_top-Level Breakdown (hierarchical)

| Area under `fpga_top` | Instances | SB_LUT4 | LUT Share of fpga_top | DFF cells | SB_CARRY | SB_RAM40_4K |
|---|---:|---:|---:|---:|---:|---:|
| top | 1 | 6242 | 96.9% | 2194 | 1502 | 30 |
| uart | 1 | 112 | 1.7% | 88 | 37 | 0 |
| ff_sync | 2 | 1 | 0.0% | 2 | 0 | 0 |
| fpga_top local glue (PLL/reset/IO) | 1 | 85 | 1.3% | 31 | 38 | 0 |

## rtl/common/top.sv Breakdown (hierarchical)

| Area under `top` | Instances | SB_LUT4 | LUT Share of `top` | DFF cells | SB_CARRY | SB_RAM40_4K |
|---|---:|---:|---:|---:|---:|---:|
| cpu | 1 | 4101 | 65.7% | 1024 | 775 | 6 |
| host_bus_interface | 1 | 655 | 10.5% | 556 | 137 | 0 |
| sram_peripheral | 1 | 501 | 8.0% | 163 | 10 | 24 |
| sys_led_controller | 1 | 267 | 4.3% | 124 | 98 | 0 |
| clock_peripheral | 1 | 207 | 3.3% | 121 | 131 | 0 |
| bus | 1 | 172 | 2.8% | 0 | 248 | 0 |
| bus_arbiter | 1 | 138 | 2.2% | 2 | 0 | 0 |
| host_bus_mux | 1 | 81 | 1.3% | 0 | 62 | 0 |
| system_controller | 1 | 59 | 0.9% | 69 | 0 | 0 |
| reset_controller | 1 | 37 | 0.6% | 26 | 41 | 0 |
| led_controller_peripheral | 1 | 11 | 0.2% | 8 | 0 | 0 |
| bus_bridge | 1 | 8 | 0.1% | 101 | 0 | 0 |
| top local glue logic | 1 | 5 | 0.1% | 0 | 0 | 0 |

## CPU Core Breakdown (hierarchical)

| Area under `cpu` | Instances | SB_LUT4 | LUT Share of `cpu` | DFF cells | SB_CARRY | SB_RAM40_4K |
|---|---:|---:|---:|---:|---:|---:|
| alu | 1 | 2205 | 53.8% | 246 | 468 | 0 |
| csr_file | 1 | 316 | 7.7% | 65 | 60 | 2 |
| writeback_mux | 1 | 262 | 6.4% | 0 | 59 | 0 |
| decompress | 1 | 243 | 5.9% | 0 | 0 | 0 |
| mem_interface | 1 | 93 | 2.3% | 0 | 0 | 0 |
| branch_unit | 1 | 62 | 1.5% | 0 | 64 | 0 |
| decoder | 1 | 59 | 1.4% | 0 | 0 | 0 |
| fetch_buffer | 1 | 40 | 1.0% | 18 | 0 | 0 |
| regfile | 1 | 2 | 0.0% | 0 | 0 | 4 |
| cpu local control/FSM/staging | 1 | 819 | 20.0% | 695 | 124 | 0 |

## ALU Module Detailed Breakdown (hierarchical)

| Area under `alu` | Instances | SB_LUT4 | LUT Share of `alu` | DFF cells | SB_CARRY | SB_RAM40_4K |
|---|---:|---:|---:|---:|---:|---:|
| div_unit | 1 | 779 | 35.3% | 140 | 216 | 0 |
| mul_unit | 1 | 538 | 24.4% | 106 | 157 | 0 |
| alu local arithmetic/control glue | 1 | 888 | 40.3% | 0 | 95 | 0 |

### ALU Primitive Mix (total, including descendants)

| Primitive | Count | Share within alu primitive count |
|---|---:|---:|
| SB_LUT4 | 2205 | 75.5% |
| SB_CARRY | 468 | 16.0% |
| SB_DFFER | 246 | 8.4% |

## Highest LUT Consumers (module view)

| Module | SB_LUT4 | Share of fpga_top LUTs | DFF cells | SB_CARRY | SB_RAM40_4K |
|---|---:|---:|---:|---:|---:|
| fpga_top | 6441 | 100.0% | 2317 | 1577 | 30 |
| top | 6242 | 96.9% | 2194 | 1502 | 30 |
| cpu | 4101 | 63.7% | 1024 | 775 | 6 |
| alu | 2205 | 34.2% | 246 | 468 | 0 |
| div_unit | 779 | 12.1% | 140 | 216 | 0 |
| host_bus_interface | 655 | 10.2% | 556 | 137 | 0 |
| mul_unit | 538 | 8.4% | 106 | 157 | 0 |
| sram_peripheral | 501 | 7.8% | 163 | 10 | 24 |
| csr_file | 316 | 4.9% | 65 | 60 | 2 |
| sys_led_controller | 267 | 4.1% | 124 | 98 | 0 |
| writeback_mux | 262 | 4.1% | 0 | 59 | 0 |
| decompress | 243 | 3.8% | 0 | 0 | 0 |

## Notes

- `SB_LUT4` is the primary logic-area proxy for module attribution; nextpnr packed LC usage is listed separately.
- Hierarchical counts include all descendants under each module/area.
- Parameter-specialized variants are normalized to their base module names in tables.
