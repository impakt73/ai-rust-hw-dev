# Makefile for RISC-V FPGA Synthesis
# Targets Alchitry Cu v1 (iCE40-HX8K-CB132) using open-source tools (Yosys + nextpnr + icestorm)

# Project configuration
PROJECT = riscv_fpga
TOP_MODULE = fpga_top
DEVICE = hx8k
PACKAGE = cb132

# File paths
RTL_DIR = ../rtl
FPGA_DIR = .
BUILD_DIR = build

# Source files (includes M extension support for multiply/divide)
RTL_SOURCES = \
	$(RTL_DIR)/cpu/alu.sv \
	$(RTL_DIR)/cpu/branch_unit.sv \
	$(RTL_DIR)/memory/bus.sv \
	$(RTL_DIR)/memory/bus_arbiter.sv \
	$(RTL_DIR)/cpu/csr_file.sv \
	$(RTL_DIR)/cpu/decoder.sv \
	$(RTL_DIR)/cpu/decompress.sv \
	$(RTL_DIR)/cpu/div_unit.sv \
	$(RTL_DIR)/primitives/ff_sync.sv \
	$(RTL_DIR)/io/host_bus_mux.sv \
	$(RTL_DIR)/cpu/fetch_buffer.sv \
	$(RTL_DIR)/io/host_bus_interface.sv \
	$(RTL_DIR)/io/host_bus_rx.sv \
	$(RTL_DIR)/io/host_bus_tx.sv \
	$(RTL_DIR)/cpu/mem_interface.sv \
	$(RTL_DIR)/cpu/mul_unit.sv \
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

FPGA_SOURCES = \
	$(FPGA_DIR)/stub_fpu.sv \
	$(FPGA_DIR)/fpga_top.sv

ALL_SOURCES = $(RTL_SOURCES) $(FPGA_SOURCES)

# Constraint file
PCF = $(FPGA_DIR)/ice40hx8k.pcf

# Output files
JSON = $(BUILD_DIR)/$(PROJECT).json
ASC = $(BUILD_DIR)/$(PROJECT).asc
BIN = $(BUILD_DIR)/$(PROJECT).bin
TIMING = $(BUILD_DIR)/$(PROJECT)_timing.rpt
UTILIZATION = $(BUILD_DIR)/$(PROJECT)_utilization.rpt

# Tools
YOSYS = yosys
NEXTPNR = nextpnr-ice40
ICEPACK = icepack
ICEPROG = iceprog
ICETIME = icetime
# Fixed seed for reproducible timing closure in CI (seed 3 passes 25 MHz target)
NEXTPNR_SEED ?= 3

# Default target
.PHONY: all
all: $(BIN) timing

# Create build directory
$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# Synthesis with Yosys
$(JSON): $(ALL_SOURCES) | $(BUILD_DIR)
	bash -e -o pipefail -c '$(YOSYS) -p "read_verilog -sv $(ALL_SOURCES); \
			     synth_ice40 -top $(TOP_MODULE) -json $(JSON)" 2>&1 | tee $(BUILD_DIR)/yosys.log'
# Place and Route with nextpnr
$(ASC): $(JSON) $(PCF)
	bash -e -o pipefail -c '$(NEXTPNR) --$(DEVICE) --package $(PACKAGE) \
	           --json $(JSON) \
	           --pcf $(PCF) \
	           --asc $(ASC) \
	           --freq 25 \
	           --seed $(NEXTPNR_SEED) \
	           2>&1 | tee $(BUILD_DIR)/nextpnr.log'

# Generate bitstream
$(BIN): $(ASC)
	bash -e -o pipefail -c '$(ICEPACK) $(ASC) $(BIN)'

# Timing analysis
.PHONY: timing
timing: $(ASC)
	$(ICETIME) -d $(DEVICE) -t -r $(TIMING) $(ASC)
	@echo "Timing report written to $(TIMING)"

# Resource utilization report
.PHONY: utilization
utilization: $(JSON)
	bash -e -o pipefail -c 'echo "Generating resource utilization report..."; echo "=== Resource Utilization ===" > $(UTILIZATION); grep -A 20 "Printing statistics" $(BUILD_DIR)/yosys.log >> $(UTILIZATION) || true; cat $(UTILIZATION)'


# Program FPGA
.PHONY: program
program: $(BIN)
	bash -e -o pipefail -c '$(ICEPROG) $(BIN)'

# Clean build artifacts
.PHONY: clean
clean:
	rm -rf $(BUILD_DIR)

# Show help
.PHONY: help
help:
	@echo "RISC-V FPGA Synthesis Makefile"
	@echo ""
	@echo "Targets:"
	@echo "  all          - Synthesize and generate bitstream (default)"
	@echo "  $(JSON)      - Run synthesis only (Yosys)"
	@echo "  $(ASC)       - Run place and route (nextpnr)"
	@echo "  $(BIN)       - Generate bitstream (icepack)"
	@echo "  timing       - Generate timing report"
	@echo "  utilization  - Show resource utilization"
	@echo "  program      - Program FPGA with bitstream"
	@echo "  clean        - Remove build artifacts"
	@echo "  help         - Show this help message"
	@echo ""
	@echo "Prerequisites:"
	@echo "  - yosys      : Verilog synthesis"
	@echo "  - nextpnr    : Place and route"
	@echo "  - icestorm   : Bitstream generation and programming"

.PHONY: check-tools
check-tools:
	@echo "Checking for required tools..."
	@which $(YOSYS) > /dev/null || (echo "ERROR: yosys not found"; exit 1)
	@which $(NEXTPNR) > /dev/null || (echo "ERROR: nextpnr-ice40 not found"; exit 1)
	@which $(ICEPACK) > /dev/null || (echo "ERROR: icepack not found"; exit 1)
	@which $(ICEPROG) > /dev/null || (echo "ERROR: iceprog not found (optional for programming)"; exit 0)
	@echo "All required tools found!"
