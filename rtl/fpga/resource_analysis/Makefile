# Makefile for FPGA Resource Analysis
# Synthesizes individual RTL modules to measure resource consumption

# Project configuration
DEVICE = hx8k
PACKAGE = cb132

# Paths
RTL_DIR = ../../common
FPGA_DIR = ..
BUILD_DIR = build
HARNESS = synth_harness.sv

# Tools
YOSYS = yosys

# PCF file (minimal for synthesis-only)
PCF = $(FPGA_DIR)/led_demo/alchitry_cu.pcf

# Module-specific source files
# Base sources needed by most tests
BASE_SOURCES = $(RTL_DIR)/cpu/div_unit.sv

# FPU base sources (common dependencies)
FPU_BASE_SOURCES = \
	$(RTL_DIR)/fpu/fpu_classifier.sv \
	$(RTL_DIR)/fpu/fpu_comparator.sv \
	$(RTL_DIR)/fpu/fpu_sqrt.sv \
	$(RTL_DIR)/fpu/fpu_int_to_float.sv \
	$(RTL_DIR)/fpu/fpu_float_to_int.sv \
	$(RTL_DIR)/fpu/fpu_div_setup.sv \
	$(RTL_DIR)/fpu/fpu_div_assemble.sv \
	$(RTL_DIR)/fpu/fpu_fma.sv

# Module source definitions
SOURCES_alu = $(RTL_DIR)/cpu/alu.sv $(RTL_DIR)/cpu/div_unit.sv
SOURCES_branch_unit = $(RTL_DIR)/cpu/branch_unit.sv
SOURCES_csr_file = $(RTL_DIR)/cpu/csr_file.sv $(RTL_DIR)/memory/sync_dpram.sv
SOURCES_decoder = $(RTL_DIR)/cpu/decoder.sv
SOURCES_decompress = $(RTL_DIR)/cpu/decompress.sv
SOURCES_div_unit = $(RTL_DIR)/cpu/div_unit.sv
SOURCES_fetch_buffer = $(RTL_DIR)/cpu/fetch_buffer.sv
SOURCES_fp_regfile = $(RTL_DIR)/fpu/fp_regfile.sv
SOURCES_fpu = $(RTL_DIR)/fpu/fpu.sv $(FPU_BASE_SOURCES) $(RTL_DIR)/cpu/div_unit.sv
SOURCES_fpu_classifier = $(RTL_DIR)/fpu/fpu_classifier.sv
SOURCES_fpu_comparator = $(RTL_DIR)/fpu/fpu_comparator.sv
SOURCES_fpu_div_assemble = $(RTL_DIR)/fpu/fpu_div_assemble.sv
SOURCES_fpu_div_setup = $(RTL_DIR)/fpu/fpu_div_setup.sv
SOURCES_fpu_float_to_int = $(RTL_DIR)/fpu/fpu_float_to_int.sv
SOURCES_fpu_fma = $(RTL_DIR)/fpu/fpu_fma.sv
SOURCES_fpu_int_to_float = $(RTL_DIR)/fpu/fpu_int_to_float.sv
SOURCES_fpu_sqrt = $(RTL_DIR)/fpu/fpu_sqrt.sv
SOURCES_mem_interface = $(RTL_DIR)/cpu/mem_interface.sv
SOURCES_regfile = $(RTL_DIR)/cpu/regfile.sv
SOURCES_writeback_mux = $(RTL_DIR)/cpu/writeback_mux.sv

# Define macro names for each module
DEFINE_alu = SYNTH_ALU
DEFINE_branch_unit = SYNTH_BRANCH_UNIT
DEFINE_csr_file = SYNTH_CSR_FILE
DEFINE_decoder = SYNTH_DECODER
DEFINE_decompress = SYNTH_DECOMPRESS
DEFINE_div_unit = SYNTH_DIV_UNIT
DEFINE_fetch_buffer = SYNTH_FETCH_BUFFER
DEFINE_fp_regfile = SYNTH_FP_REGFILE
DEFINE_fpu = SYNTH_FPU
DEFINE_fpu_classifier = SYNTH_FPU_CLASSIFIER
DEFINE_fpu_comparator = SYNTH_FPU_COMPARATOR
DEFINE_fpu_div_assemble = SYNTH_FPU_DIV_ASSEMBLE
DEFINE_fpu_div_setup = SYNTH_FPU_DIV_SETUP
DEFINE_fpu_float_to_int = SYNTH_FPU_FLOAT_TO_INT
DEFINE_fpu_fma = SYNTH_FPU_FMA
DEFINE_fpu_int_to_float = SYNTH_FPU_INT_TO_FLOAT
DEFINE_fpu_sqrt = SYNTH_FPU_SQRT
DEFINE_mem_interface = SYNTH_MEM_INTERFACE
DEFINE_regfile = SYNTH_REGFILE
DEFINE_writeback_mux = SYNTH_WRITEBACK_MUX

# List of all modules to analyze
MODULES = \
	alu \
	branch_unit \
	csr_file \
	decoder \
	decompress \
	div_unit \
	fetch_buffer \
	fp_regfile \
	fpu \
	fpu_classifier \
	fpu_comparator \
	fpu_div_assemble \
	fpu_div_setup \
	fpu_float_to_int \
	fpu_fma \
	fpu_int_to_float \
	fpu_sqrt \
	mem_interface \
	regfile \
	writeback_mux

# Default target: analyze all modules
.PHONY: all
all: report

# Create build directory
$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# Generic synthesis rule for any module
# Usage: make synth-<module_name>
define synth_template
.PHONY: synth-$(1)
synth-$(1): $(BUILD_DIR)
	@echo "============================================================"
	@echo "Synthesizing module: $(1)"
	@echo "============================================================"
	$(YOSYS) -p "read_verilog -sv -D$(DEFINE_$(1)) $(HARNESS) $(SOURCES_$(1)); \
	             synth_ice40 -top synth_harness -json $(BUILD_DIR)/$(1).json" \
	         2>&1 | tee $(BUILD_DIR)/$(1).log
	@echo ""
	@echo "=== Resource Summary for $(1) ==="
	@grep -A 20 "Printing statistics" $(BUILD_DIR)/$(1).log | head -25
	@echo ""
endef

# Generate rules for all modules
$(foreach mod,$(MODULES),$(eval $(call synth_template,$(mod))))

# Synthesize all modules
.PHONY: synth-all
synth-all: $(addprefix synth-,$(MODULES))

# Generate report
.PHONY: report
report: synth-all
	@echo "============================================================"
	@echo "Generating Resource Analysis Report"
	@echo "============================================================"
	@./generate_report.sh > $(BUILD_DIR)/resource_report.md
	@cat $(BUILD_DIR)/resource_report.md
	@echo ""
	@echo "Report saved to $(BUILD_DIR)/resource_report.md"

# Quick synthesis of a single module (for testing)
# Usage: make quick MOD=alu
.PHONY: quick
quick: $(BUILD_DIR)
	@if [ -z "$(MOD)" ]; then echo "Usage: make quick MOD=<module_name>"; exit 1; fi
	@echo "Quick synthesis of module: $(MOD)"
	$(YOSYS) -p "read_verilog -sv -D$(DEFINE_$(MOD)) $(HARNESS) $(SOURCES_$(MOD)); \
	             synth_ice40 -top synth_harness" 2>&1 | tee $(BUILD_DIR)/$(MOD).log
	@echo ""
	@grep -A 20 "Printing statistics" $(BUILD_DIR)/$(MOD).log

# Synthesize the full CPU for baseline comparison
.PHONY: synth-full-cpu
synth-full-cpu: $(BUILD_DIR)
	@echo "============================================================"
	@echo "Synthesizing FULL CPU (baseline)"
	@echo "============================================================"
	$(YOSYS) -p "read_verilog -sv \
		$(RTL_DIR)/cpu/alu.sv \
		$(RTL_DIR)/cpu/branch_unit.sv \
		$(RTL_DIR)/cpu/csr_file.sv \
		$(RTL_DIR)/cpu/decoder.sv \
		$(RTL_DIR)/cpu/decompress.sv \
		$(RTL_DIR)/cpu/div_unit.sv \
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
		$(RTL_DIR)/cpu/mem_interface.sv \
		$(RTL_DIR)/cpu/regfile.sv \
		$(RTL_DIR)/cpu/writeback_mux.sv \
		$(RTL_DIR)/cpu/cpu.sv \
		$(RTL_DIR)/peripherals/led_controller_peripheral.sv \
		$(RTL_DIR)/top.sv \
		$(FPGA_DIR)/common/fpga_common_top.sv \
		$(FPGA_DIR)/ice40_alchitry_cu/ice40_alchitry_cu_top.sv; \
		synth_ice40 -top ice40_alchitry_cu_top -json $(BUILD_DIR)/full_cpu.json" \
		2>&1 | tee $(BUILD_DIR)/full_cpu.log
	@echo ""
	@echo "=== Full CPU Resource Summary ==="
	@grep -A 20 "Printing statistics" $(BUILD_DIR)/full_cpu.log

# Clean build artifacts
.PHONY: clean
clean:
	rm -rf $(BUILD_DIR)

# Help
.PHONY: help
help:
	@echo "FPGA Resource Analysis Makefile"
	@echo ""
	@echo "Targets:"
	@echo "  all          - Synthesize all modules and generate report"
	@echo "  synth-all    - Synthesize all modules"
	@echo "  synth-<mod>  - Synthesize specific module (e.g., synth-alu)"
	@echo "  quick MOD=x  - Quick synthesis of module x"
	@echo "  synth-full-cpu - Synthesize full CPU for baseline"
	@echo "  report       - Generate resource analysis report"
	@echo "  clean        - Remove build artifacts"
	@echo "  help         - Show this help"
	@echo ""
	@echo "Available modules:"
	@echo "  $(MODULES)"
