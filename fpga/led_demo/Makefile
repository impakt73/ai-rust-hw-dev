# Makefile for LED Pattern Demo FPGA Synthesis
# Targets Alchitry Cu v1 (iCE40-HX8K-CB132) using open-source tools (Yosys + nextpnr + icestorm)
# This is a simplified version that focuses on synthesis only

# Project configuration
PROJECT = led_pattern_demo
TOP_MODULE = led_pattern_top
DEVICE = hx8k
PACKAGE = cb132

# File paths
FPGA_DIR = .
BUILD_DIR = build

# Source files - minimal design only
SOURCES = $(FPGA_DIR)/led_pattern_top.sv

# Constraint file
PCF = $(FPGA_DIR)/alchitry_cu.pcf

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

# Default target - synthesis only (no place and route)
.PHONY: all
all: synth

# Create build directory
$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# Synthesis with Yosys
.PHONY: synth
synth: $(JSON)

$(JSON): $(SOURCES) | $(BUILD_DIR)
	bash -e -o pipefail -c '$(YOSYS) -p "read_verilog -sv $(SOURCES); \
	           synth_ice40 -top $(TOP_MODULE) -json $(JSON)" 2>&1 | tee $(BUILD_DIR)/yosys.log'

# Place and Route with nextpnr (optional)
$(ASC): $(JSON) $(PCF)
	bash -e -o pipefail -c '$(NEXTPNR) --$(DEVICE) --package $(PACKAGE) \
	           --json $(JSON) \
	           --pcf $(PCF) \
	           --asc $(ASC) \
	           --freq 100 \
	           2>&1 | tee $(BUILD_DIR)/nextpnr.log'

# Generate bitstream (optional)
$(BIN): $(ASC)
	bash -e -o pipefail -c '$(ICEPACK) $(ASC) $(BIN)'

# Full flow including place and route and bitstream
.PHONY: bitstream
bitstream: $(BIN)

# Timing analysis
.PHONY: timing
timing: $(ASC)
	$(ICETIME) -d $(DEVICE) -t -r $(TIMING) $(ASC)
	@echo "Timing report written to $(TIMING)"

# Resource utilization report
.PHONY: utilization
utilization: $(JSON)
	@echo "=== Resource Utilization Report ===" > $(UTILIZATION)
	@echo "" >> $(UTILIZATION)
	@grep -A 50 "Printing statistics" $(BUILD_DIR)/yosys.log >> $(UTILIZATION) || true
	@echo "" >> $(UTILIZATION)
	@echo "Report generated from synthesis log" >> $(UTILIZATION)
	@cat $(UTILIZATION)

# Program FPGA (optional)
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
	@echo "LED Pattern Demo FPGA Synthesis Makefile"
	@echo ""
	@echo "Targets:"
	@echo "  all (synth)  - Run synthesis only with Yosys (default)"
	@echo "  bitstream    - Full flow: synthesis + place/route + bitstream"
	@echo "  timing       - Generate timing report"
	@echo "  utilization  - Show resource utilization from synthesis"
	@echo "  program      - Program FPGA with bitstream"
	@echo "  clean        - Remove build artifacts"
	@echo "  help         - Show this help message"
	@echo ""
	@echo "Output files:"
	@echo "  build/$(PROJECT).json - Synthesis output (Yosys)"
	@echo "  build/$(PROJECT).asc  - Place and route output (nextpnr)"
	@echo "  build/$(PROJECT).bin  - Bitstream for programming"
	@echo ""
	@echo "Prerequisites:"
	@echo "  - yosys      : Verilog synthesis"
	@echo "  - nextpnr    : Place and route (for bitstream target)"
	@echo "  - icestorm   : Bitstream generation and programming"

.PHONY: check-tools
check-tools:
	@echo "Checking for required tools..."
	@which $(YOSYS) > /dev/null || (echo "ERROR: yosys not found"; exit 1)
	@which $(NEXTPNR) > /dev/null || (echo "WARNING: nextpnr-ice40 not found (needed for bitstream)"; exit 0)
	@which $(ICEPACK) > /dev/null || (echo "WARNING: icepack not found (needed for bitstream)"; exit 0)
	@echo "Required tools found!"
