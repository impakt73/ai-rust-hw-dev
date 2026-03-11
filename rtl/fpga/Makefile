# Makefile for RISC-V FPGA Synthesis
# Supports open-source Lattice flows plus a Vivado batch flow for the Alchitry Au.

# Project configuration
PROJECT = riscv_fpga
TARGET ?= ice40_alchitry_cu

# File paths
RTL_DIR = ../common
BUILD_DIR = build

# Source files (includes M extension support for multiply/divide)
RTL_SOURCES = \
	$(RTL_DIR)/cpu/alu.sv \
	$(RTL_DIR)/cpu/branch_unit.sv \
	$(RTL_DIR)/cpu/csr_file.sv \
	$(RTL_DIR)/cpu/decoder.sv \
	$(RTL_DIR)/cpu/decompress.sv \
	$(RTL_DIR)/cpu/div_unit.sv \
	$(RTL_DIR)/primitives/ff_sync.sv \
	$(RTL_DIR)/primitives/phase_accumulator.sv \
	$(RTL_DIR)/primitives/activity_indicator.sv \
	$(RTL_DIR)/primitives/square_wave_generator.sv \
	$(RTL_DIR)/io/host_bus_mux.sv \
	$(RTL_DIR)/cpu/fetch_buffer.sv \
	$(RTL_DIR)/fpu/fp_regfile.sv \
	$(RTL_DIR)/fpu/fpu.sv \
	$(RTL_DIR)/fpu/fpu_classifier.sv \
	$(RTL_DIR)/fpu/fpu_comparator.sv \
	$(RTL_DIR)/fpu/fpu_div_assemble.sv \
	$(RTL_DIR)/fpu/fpu_div_setup.sv \
	$(RTL_DIR)/fpu/fpu_float_to_int.sv \
	$(RTL_DIR)/fpu/fpu_fma.sv \
	$(RTL_DIR)/fpu/fpu_int_to_float.sv \
	$(RTL_DIR)/fpu/fpu_sqrt.sv \
	$(RTL_DIR)/io/host_bus_interface.sv \
	$(RTL_DIR)/io/host_bus_rx.sv \
	$(RTL_DIR)/io/host_bus_tx.sv \
	$(RTL_DIR)/io/sys_led_controller.sv \
	$(RTL_DIR)/cpu/mem_interface.sv \
	$(RTL_DIR)/cpu/mul_unit.sv \
	$(RTL_DIR)/memory/registered_bus.sv \
	$(RTL_DIR)/memory/sync_dpram.sv \
	$(RTL_DIR)/cpu/regfile.sv \
	$(RTL_DIR)/primitives/sync_fifo.sv \
	$(RTL_DIR)/primitives/reset_controller.sv \
	$(RTL_DIR)/io/uart.sv \
	$(RTL_DIR)/memory/sram.sv \
	$(RTL_DIR)/cpu/writeback_mux.sv \
	$(RTL_DIR)/cpu/cpu.sv \
	$(RTL_DIR)/peripherals/clock_peripheral.sv \
	$(RTL_DIR)/peripherals/led_controller_peripheral.sv \
	$(RTL_DIR)/peripherals/sram_peripheral.sv \
	$(RTL_DIR)/peripherals/system_controller_peripheral.sv \
	$(RTL_DIR)/top.sv

ifeq ($(TARGET),ice40_alchitry_cu)
DEVICE = hx8k
PACKAGE = cb132
FPGA_DIR = ice40_alchitry_cu
TOP_MODULE = ice40_alchitry_cu_top
CONSTRAINT = $(FPGA_DIR)/ice40_alchitry_cu.pcf
SYNTH_CMD = synth_ice40 -top $(TOP_MODULE) -json $(NETLIST)
PNR_OUT = $(BUILD_DIR)/$(PROJECT).asc
PNR_CMD = $(NEXTPNR) --$(DEVICE) --package $(PACKAGE) \
		--json $(NETLIST) \
		--pcf $(CONSTRAINT) \
		--asc $(PNR_OUT) \
		--freq 25
PACK_CMD = $(ICEPACK) $(PNR_OUT) $(BIN)
PROGRAM_CMD = $(ICEPROG) $(BIN)
OUTPUT_EXT = bin
endif

ifeq ($(TARGET),ecp5_icepi_zero)
DEVICE = 25k
PACKAGE = CABGA256
FPGA_DIR = ecp5_icepi_zero
TOP_MODULE = ecp5_icepi_zero_top
CONSTRAINT = $(FPGA_DIR)/ecp5_icepi_zero.lpf
SYNTH_CMD = synth_ecp5 -top $(TOP_MODULE) -json $(NETLIST)
PNR_OUT = $(BUILD_DIR)/$(PROJECT).config
PNR_CMD = $(NEXTPNR) --$(DEVICE) --package $(PACKAGE) \
		--json $(NETLIST) \
		--lpf $(CONSTRAINT) \
		--textcfg $(PNR_OUT) \
		--freq 50
PACK_CMD = $(ECPPACK) $(PNR_OUT) $(BIN)
PROGRAM_CMD = $(OPENFPGALOADER) -b icepi_zero $(BIN)
OUTPUT_EXT = bit
endif

ifeq ($(TARGET),artix7_alchitry_au)
PART = xc7a35tftg256-1
FPGA_DIR = artix7_alchitry_au
TOP_MODULE = artix7_alchitry_au_top
CONSTRAINT = $(FPGA_DIR)/alchitry_au.xdc
VIVADO_TCL = $(FPGA_DIR)/vivado_build.tcl
PROGRAM_CMD = $(OPENFPGALOADER) -b alchitry_au $(BIN)
OUTPUT_EXT = bit
endif

ifndef FPGA_DIR
$(error Unsupported TARGET value '$(TARGET)'. Set TARGET=ice40_alchitry_cu, TARGET=ecp5_icepi_zero, or TARGET=artix7_alchitry_au)
endif

BUILD_DIR := build/$(TARGET)
NETLIST ?= $(BUILD_DIR)/$(PROJECT).json

FPGA_SOURCES = \
	common/fpga_common_top.sv \
	$(FPGA_DIR)/$(TOP_MODULE).sv

ALL_SOURCES = $(RTL_SOURCES) $(FPGA_SOURCES)

# Output files
BIN = $(BUILD_DIR)/$(PROJECT).$(OUTPUT_EXT)
TIMING = $(BUILD_DIR)/$(PROJECT)_timing.rpt
UTILIZATION = $(BUILD_DIR)/$(PROJECT)_utilization.rpt

# Tools
YOSYS = yosys
NEXTPNR = nextpnr-ice40
ICEPACK = icepack
ICEPROG = iceprog
ICETIME = icetime
ECPPACK = ecppack
OPENFPGALOADER = openFPGALoader
VIVADO = vivado
PYTHON3 = python3
STATS_SCRIPT = ./fpga_design_stats.py
STATS_FORMAT ?= text

ifeq ($(TARGET),ecp5_icepi_zero)
NEXTPNR = nextpnr-ecp5
endif

# Default target
.PHONY: all
all: $(BIN) timing

# Create build directory
$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# Synthesis with Yosys
ifeq ($(TARGET),artix7_alchitry_au)
$(BIN): $(ALL_SOURCES) $(CONSTRAINT) $(VIVADO_TCL) | $(BUILD_DIR)
	$(VIVADO) -mode batch -source $(abspath $(VIVADO_TCL)) -tclargs \
		"$(abspath $(BUILD_DIR))" "$(TOP_MODULE)" "$(PART)" "$(abspath $(CONSTRAINT))" \
		$(foreach src,$(ALL_SOURCES),"$(abspath $(src))") 2>&1 | tee $(BUILD_DIR)/vivado.log
else
$(NETLIST): $(ALL_SOURCES) | $(BUILD_DIR)
	bash -e -o pipefail -c '$(YOSYS) -p "read_verilog -sv $(ALL_SOURCES); \
			     $(SYNTH_CMD)" 2>&1 | tee $(BUILD_DIR)/yosys.log'
# Place and Route with nextpnr
$(PNR_OUT): $(NETLIST) $(CONSTRAINT)
	bash -e -o pipefail -c '$(PNR_CMD) 2>&1 | tee $(BUILD_DIR)/nextpnr.log'

# Generate bitstream
$(BIN): $(PNR_OUT)
	bash -e -o pipefail -c '$(PACK_CMD)'
endif

# Timing analysis
.PHONY: timing
ifeq ($(TARGET),artix7_alchitry_au)
timing: $(BIN)
	@test -f $(TIMING) || (echo "ERROR: Expected timing report at $(TIMING)"; exit 1)
	@echo "Timing report written to $(TIMING)"
else
timing: $(PNR_OUT)
ifeq ($(TARGET),ice40_alchitry_cu)
	$(ICETIME) -d $(DEVICE) -t -r $(TIMING) $(PNR_OUT)
	@echo "Timing report written to $(TIMING)"
else
	@echo "Timing report generation is only implemented for TARGET=ice40_alchitry_cu"
endif
endif

# Resource utilization report
.PHONY: utilization
ifeq ($(TARGET),artix7_alchitry_au)
utilization: $(BIN)
	@test -f $(UTILIZATION) || (echo "ERROR: Expected utilization report at $(UTILIZATION)"; exit 1)
	@cat $(UTILIZATION)
else
utilization: $(NETLIST)
	bash -e -o pipefail -c 'echo "Generating resource utilization report..."; echo "=== Resource Utilization ===" > $(UTILIZATION); grep -A 20 "Printing statistics" $(BUILD_DIR)/yosys.log >> $(UTILIZATION) || true; cat $(UTILIZATION)'
endif

# Standardized design stats summary
.PHONY: stats
stats:
	@$(PYTHON3) $(STATS_SCRIPT) --target $(TARGET) --build --format $(STATS_FORMAT)


# Program FPGA
.PHONY: program
program: $(BIN)
	bash -e -o pipefail -c '$(PROGRAM_CMD)'

# Clean build artifacts
.PHONY: clean
clean:
	rm -rf $(BUILD_DIR)

# Show help
.PHONY: help
help:
	@echo "RISC-V FPGA Synthesis Makefile"
	@echo "  Target board: $(TARGET)"
	@echo ""
	@echo "Targets:"
	@echo "  all          - Synthesize and generate bitstream (default)"
ifeq ($(TARGET),artix7_alchitry_au)
	@echo "  $(BIN)       - Run the Vivado batch flow and generate the bitstream"
else
	@echo "  $(NETLIST)   - Run synthesis only (Yosys)"
	@echo "  $(PNR_OUT)   - Run place and route (nextpnr)"
	@echo "  $(BIN)       - Generate bitstream (target-specific packer)"
endif
	@echo "  timing       - Generate timing report"
	@echo "  utilization  - Show resource utilization"
	@echo "  stats        - Build and print standardized timing/utilization stats"
	@echo "  program      - Program FPGA with bitstream"
	@echo "  clean        - Remove build artifacts"
	@echo "  help         - Show this help message"
	@echo ""
	@echo "Prerequisites:"
ifeq ($(TARGET),artix7_alchitry_au)
	@echo "  - vivado     : Xilinx Vivado batch/CLI tools"
	@echo "  - openFPGALoader : Optional board programming tool"
else
	@echo "  - yosys      : Verilog synthesis"
	@echo "  - nextpnr    : Place and route"
	@echo "  - icestorm   : Bitstream generation and programming"
endif
	@echo ""
	@echo "Target selection:"
	@echo "  - make TARGET=ice40_alchitry_cu  (default: Alchitry Cu v1)"
	@echo "  - make TARGET=ecp5_icepi_zero  (iCE Pi Zero ECP5-25F)"
	@echo "  - make TARGET=artix7_alchitry_au  (Alchitry Au Artix-7)"
	@echo ""
	@echo "Stats output format:"
	@echo "  - make TARGET=ice40_alchitry_cu stats STATS_FORMAT=text"
	@echo "  - make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json"
	@echo "  - make TARGET=ice40_alchitry_cu stats STATS_FORMAT=markdown"

.PHONY: check-tools
check-tools:
	@echo "Checking for required tools..."
ifeq ($(TARGET),artix7_alchitry_au)
	@which $(VIVADO) > /dev/null || (echo "ERROR: $(VIVADO) not found"; exit 1)
	@which $(OPENFPGALOADER) > /dev/null || (echo "WARNING: openFPGALoader not found (optional for programming)"; exit 0)
else
	@which $(YOSYS) > /dev/null || (echo "ERROR: yosys not found"; exit 1)
	@which $(NEXTPNR) > /dev/null || (echo "ERROR: $(NEXTPNR) not found"; exit 1)
ifeq ($(TARGET),ice40_alchitry_cu)
	@which $(ICEPACK) > /dev/null || (echo "ERROR: icepack not found"; exit 1)
	@which $(ICEPROG) > /dev/null || (echo "ERROR: iceprog not found (optional for programming)"; exit 0)
endif
ifeq ($(TARGET),ecp5_icepi_zero)
	@which $(ECPPACK) > /dev/null || (echo "ERROR: ecppack not found"; exit 1)
	@which $(OPENFPGALOADER) > /dev/null || (echo "ERROR: openFPGALoader not found (optional for programming)"; exit 0)
endif
endif
	@echo "All required tools found!"
