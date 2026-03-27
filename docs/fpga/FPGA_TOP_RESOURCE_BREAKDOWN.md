# FPGA RTL Resource Breakdown (ECP5)

Generated: 2026-03-08 18:27 UTC

> **Note:** This report predates the consolidation of the standalone LED and clock
> peripherals into `system_controller`. The rows for `clock_peripheral` and
> `led_controller_peripheral` are therefore historical and now correspond to
> logic integrated into `system_controller` in the current design.

## Scope

- Re-runs synthesis/resource analysis on the default FPGA target:
  - `TARGET=ecp5_icepi_zero` (ECP5-25F)
- Uses target build outputs from `rtl/fpga/build/ecp5_icepi_zero/` and hierarchical Yosys stats (`-noflatten`) for module attribution.

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

- The ECP5 open-source flow reports packed utilization as `TRELLIS_COMB` / `TRELLIS_FF` / `DP16KD`.
- The hierarchical Yosys proxy sums mapped logic primitives `LUT4 + PFUMX + L6MUX21 + CCU2C`.
- Hierarchical rows include descendants; local glue rows represent primitives instantiated directly in the named module.
