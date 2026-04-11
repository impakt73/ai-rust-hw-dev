# Makefile for RISC-V FPGA Synthesis
# Supports the open-source ECP5 flow plus vendor-tool bring-up flows.

# Project configuration
PROJECT = riscv_fpga
TARGET ?= ecp5_icepi_zero

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
	$(RTL_DIR)/primitives/debouncer.sv \
	$(RTL_DIR)/primitives/ff_sync.sv \
	$(RTL_DIR)/primitives/phase_accumulator.sv \
	$(RTL_DIR)/primitives/activity_indicator.sv \
	$(RTL_DIR)/primitives/bitmap_text_renderer.sv \
	$(RTL_DIR)/primitives/bus_cdc_bridge.sv \
	$(RTL_DIR)/primitives/cdc_handshake.sv \
	$(RTL_DIR)/primitives/sine_table.sv \
	$(RTL_DIR)/primitives/square_wave_generator.sv \
	$(RTL_DIR)/primitives/tone_generator.sv \
	$(RTL_DIR)/primitives/video_sync.sv \
	$(RTL_DIR)/io/apf_bus_bridge.sv \
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
	$(RTL_DIR)/io/i2s_serializer.sv \
	$(RTL_DIR)/io/sys_led_controller.sv \
	$(RTL_DIR)/cpu/mem_interface.sv \
	$(RTL_DIR)/cpu/mul_unit.sv \
	$(RTL_DIR)/memory/registered_bus.sv \
	$(RTL_DIR)/memory/sync_dpram.sv \
	$(RTL_DIR)/memory/sync_sprom.sv \
	$(RTL_DIR)/cpu/regfile.sv \
	$(RTL_DIR)/primitives/sync_fifo.sv \
	$(RTL_DIR)/primitives/reset_controller.sv \
	$(RTL_DIR)/io/uart.sv \
	$(RTL_DIR)/memory/sram.sv \
	$(RTL_DIR)/cpu/writeback_mux.sv \
	$(RTL_DIR)/cpu/cpu.sv \
	$(RTL_DIR)/peripherals/sdram_peripheral.sv \
	$(RTL_DIR)/peripherals/sram_peripheral.sv \
	$(RTL_DIR)/peripherals/gfx2d_peripheral.sv \
	$(RTL_DIR)/peripherals/audiosys_peripheral.sv \
	$(RTL_DIR)/peripherals/system_controller_peripheral.sv \
	$(RTL_DIR)/peripherals/gamepad_peripheral.sv \
	$(RTL_DIR)/top.sv

ifeq ($(TARGET),ecp5_icepi_zero)
DEVICE = 25k
PACKAGE = CABGA256
FPGA_DIR = ecp5_icepi_zero
TOP_MODULE = ecp5_icepi_zero_top
DEFAULT_PROGRAM_MODE = sram
CONSTRAINT = $(FPGA_DIR)/ecp5_icepi_zero.lpf
SDC_CONSTRAINT = $(FPGA_DIR)/timing_async.sdc
SYNTH_CMD = synth_ecp5 -top $(TOP_MODULE) -json $(NETLIST)
PNR_OUT = $(BUILD_DIR)/$(PROJECT).config
PNR_CMD = $(NEXTPNR) --$(DEVICE) --package $(PACKAGE) \
		--json $(NETLIST) \
		--lpf $(CONSTRAINT) \
		--textcfg $(PNR_OUT) \
		--freq 50
PACK_CMD = $(ECPPACK) $(PNR_OUT) $(BIN)
OPENFPGALOADER_BOARD = icepi-zero
OUTPUT_EXT = bit
endif

ifeq ($(TARGET),artix7_alchitry_au)
PART = xc7a35tftg256-1
FPGA_DIR = artix7_alchitry_au
TOP_MODULE = artix7_alchitry_au_top
DEFAULT_PROGRAM_MODE = sram
CONSTRAINT = $(FPGA_DIR)/alchitry_au.xdc
VIVADO_TCL = $(FPGA_DIR)/vivado_build.tcl
OPENFPGALOADER_BOARD = alchitry_au
OUTPUT_EXT = bit
endif

ifeq ($(TARGET),cyclonev_analogue_pocket)
PART = 5CEBA4F23C8
FPGA_DIR = cyclonev_analogue_pocket
TOP_MODULE = apf_top
DEFAULT_PROGRAM_MODE = package
POCKET_QSF = $(FPGA_DIR)/src/fpga/ap_core.qsf
QUARTUS_TCL = $(FPGA_DIR)/quartus_build.tcl
QUARTUS_SH = quartus_sh
POCKET_DEPLOY_DIR ?= /mnt/analogue-pocket
POCKET_DEPLOY_UUID ?= 0123-4567
POCKET_DEPLOY_UID ?= 1000
POCKET_DEPLOY_GID ?= 1000
POCKET_DEPLOY_UMASK ?= 022
POCKET_UMOUNT_RETRIES ?= 5
POCKET_UMOUNT_RETRY_DELAY ?= 1
OUTPUT_EXT = rbf
endif

ifeq ($(TARGET),gowin_tang_primer_25k)
PART = GW5A-LV25MG121NC1/I0
DEVICE_VERSION = B
FPGA_DIR = gowin_tang_primer_25k
TOP_MODULE = gowin_tang_primer_25k_top
DEFAULT_PROGRAM_MODE = sram
CONSTRAINT = $(FPGA_DIR)/gowin_tang_primer_25k.cst
SDC_CONSTRAINT = $(FPGA_DIR)/gowin_tang_primer_25k.sdc
GOWIN_TCL = $(FPGA_DIR)/gowin_build.tcl
GOWIN_SH = gw_sh
OPENFPGALOADER_BOARD = tangprimer25k
OUTPUT_EXT = fs
endif

PROGRAM_MODE ?= $(DEFAULT_PROGRAM_MODE)

ifdef OPENFPGALOADER_BOARD
ifeq ($(PROGRAM_MODE),sram)
OPENFPGALOADER_PROGRAM_FLAG = -m
else ifeq ($(PROGRAM_MODE),flash)
OPENFPGALOADER_PROGRAM_FLAG = -f
else
$(error Unsupported PROGRAM_MODE value '$(PROGRAM_MODE)'. Set PROGRAM_MODE=sram or PROGRAM_MODE=flash)
endif

PROGRAM_CMD = $(OPENFPGALOADER) -b $(OPENFPGALOADER_BOARD) $(OPENFPGALOADER_PROGRAM_FLAG) $(BIN)
endif

ifndef FPGA_DIR
$(error Unsupported TARGET value '$(TARGET)'. Set TARGET=ecp5_icepi_zero, TARGET=artix7_alchitry_au, TARGET=cyclonev_analogue_pocket, or TARGET=gowin_tang_primer_25k)
endif

BUILD_DIR := build/$(TARGET)
NETLIST ?= $(BUILD_DIR)/$(PROJECT).json

FPGA_SOURCES = \
	common/fpga_common_top.sv \
	$(FPGA_DIR)/$(FPGA_DIR)_top.sv

ALL_SOURCES = $(RTL_SOURCES) $(FPGA_SOURCES)
POCKET_PROJECT_FILES = $(sort $(shell find $(FPGA_DIR)/src/fpga -type f)) $(QUARTUS_TCL)
POCKET_PACKAGE_INFO = $(BUILD_DIR)/pocket-package-path.txt
POCKET_PACKAGE_STAMP = $(BUILD_DIR)/pocket-package.stamp
POCKET_PACKAGE_ROOT_FILES = $(sort $(wildcard $(FPGA_DIR)/*.json) $(wildcard $(FPGA_DIR)/*.txt))
POCKET_PACKAGE_ROOT_DIRS = $(wildcard $(FPGA_DIR)/Assets $(FPGA_DIR)/Platforms $(FPGA_DIR)/Presets)
POCKET_PACKAGE_DIR_FILES = $(sort $(foreach dir,$(POCKET_PACKAGE_ROOT_DIRS),$(shell find $(dir) -type f)))
POCKET_PACKAGE_FILES = $(sort $(POCKET_PACKAGE_ROOT_FILES) $(POCKET_PACKAGE_DIR_FILES))
POCKET_TEST_PROGRAM_DIR = $(abspath ../../rust-test-program)
POCKET_TEST_PROGRAM_TARGET = riscv32imafc-unknown-none-elf
POCKET_TEST_PROGRAM_ELF = $(POCKET_TEST_PROGRAM_DIR)/target/$(POCKET_TEST_PROGRAM_TARGET)/release/test_pocket_demo
POCKET_TEST_PROGRAM_ASSET = $(FPGA_DIR)/Assets/aihwdev/common/test_pocket_demo.bin

# Output files
BIN = $(BUILD_DIR)/$(PROJECT).$(OUTPUT_EXT)
TIMING = $(BUILD_DIR)/$(PROJECT)_timing.rpt
TIMING_SUMMARY = $(BUILD_DIR)/$(PROJECT)_timing_summary.rpt
UTILIZATION = $(BUILD_DIR)/$(PROJECT)_utilization.rpt
POCKET_PACKAGE_CMD = $(CARGO) run --quiet --release --manifest-path $(abspath ../../pkt/Cargo.toml) -- --output-path-file $(abspath $(POCKET_PACKAGE_INFO)) $(abspath $(BIN)) $(abspath $(FPGA_DIR)) $(abspath $(BUILD_DIR))

# Tools
YOSYS = yosys
NEXTPNR = nextpnr-ecp5
ECPPACK = ecppack
OPENFPGALOADER = openFPGALoader
VIVADO = vivado
CARGO ?= cargo
# Required for TARGET=cyclonev_analogue_pocket; override with
# `make RISCV_OBJCOPY=/path/to/objcopy ...` if your tool name differs.
RISCV_OBJCOPY ?= riscv64-elf-objcopy
PYTHON3 = python3
UNZIP ?= unzip
STATS_SCRIPT = ./fpga_design_stats.py
STATS_FORMAT ?= text

ifneq ($(TARGET),artix7_alchitry_au)
HAS_NEXTPNR := $(shell command -v $(NEXTPNR) >/dev/null 2>&1 && echo 1 || echo 0)
ifeq ($(HAS_NEXTPNR),1)
HAS_NEXTPNR_SDC := $(shell $(NEXTPNR) --help 2>/dev/null | grep -q -- "--sdc" && echo 1 || echo 0)
HAS_NEXTPNR_REPORT := $(shell $(NEXTPNR) --help 2>/dev/null | grep -q -- "--report" && echo 1 || echo 0)
else
HAS_NEXTPNR_SDC := 0
HAS_NEXTPNR_REPORT := 0
endif
endif

# Default target
.PHONY: all FORCE
ifeq ($(TARGET),cyclonev_analogue_pocket)
all: $(POCKET_PACKAGE_STAMP) timing
else
all: $(BIN) timing
endif

FORCE:

# Create build directory
$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# Synthesis with Yosys
ifeq ($(TARGET),artix7_alchitry_au)
$(BIN): $(ALL_SOURCES) $(CONSTRAINT) $(VIVADO_TCL) | $(BUILD_DIR)
	$(VIVADO) -mode batch \
		-journal "$(abspath $(BUILD_DIR)/vivado.jou)" \
		-log "$(abspath $(BUILD_DIR)/vivado.log)" \
		-source $(abspath $(VIVADO_TCL)) -tclargs \
		"$(abspath $(BUILD_DIR))" "$(TOP_MODULE)" "$(PART)" "$(abspath $(CONSTRAINT))" \
		$(foreach src,$(ALL_SOURCES),"$(abspath $(src))")
else ifeq ($(TARGET),cyclonev_analogue_pocket)
$(POCKET_TEST_PROGRAM_ASSET): FORCE | $(BUILD_DIR)
	@$(CARGO) build --quiet --release --manifest-path "$(POCKET_TEST_PROGRAM_DIR)/Cargo.toml" \
		--target $(POCKET_TEST_PROGRAM_TARGET) --bin test_pocket_demo
	@mkdir -p "$(dir $@)"
	@set -e; \
	tmp_bin="$(BUILD_DIR)/test_pocket_demo.bin.tmp"; \
	$(RISCV_OBJCOPY) -O binary "$(POCKET_TEST_PROGRAM_ELF)" "$$tmp_bin"; \
	if [ -f "$@" ] && cmp -s "$$tmp_bin" "$@"; then \
		rm -f "$$tmp_bin"; \
	else \
		mv "$$tmp_bin" "$@"; \
	fi

$(BIN): $(ALL_SOURCES) $(POCKET_PROJECT_FILES) | $(BUILD_DIR)
	$(QUARTUS_SH) -t $(abspath $(QUARTUS_TCL)) \
		"$(abspath $(BUILD_DIR))" "$(abspath $(POCKET_QSF))" "$(TOP_MODULE)" \
		$(foreach src,$(ALL_SOURCES),"$(abspath $(src))")

$(POCKET_PACKAGE_STAMP): $(BIN) $(POCKET_PACKAGE_FILES) $(POCKET_TEST_PROGRAM_ASSET) | $(BUILD_DIR)
	$(POCKET_PACKAGE_CMD)
	@touch $@
else ifeq ($(TARGET),gowin_tang_primer_25k)
$(BIN): $(ALL_SOURCES) $(CONSTRAINT) $(SDC_CONSTRAINT) $(GOWIN_TCL) | $(BUILD_DIR)
	$(GOWIN_SH) "$(abspath $(GOWIN_TCL))" \
		"$(abspath $(BUILD_DIR))" "$(PROJECT)" "$(TOP_MODULE)" "$(PART)" "$(DEVICE_VERSION)" \
		"$(abspath $(CONSTRAINT))" "$(abspath $(SDC_CONSTRAINT))" \
		$(foreach src,$(ALL_SOURCES),"$(abspath $(src))")
else
$(NETLIST): $(ALL_SOURCES) | $(BUILD_DIR)
	bash -e -o pipefail -c '$(YOSYS) -p "read_verilog -sv $(ALL_SOURCES); $(SYNTH_CMD)" 2>&1 | tee $(BUILD_DIR)/yosys.log'
# Place and Route with nextpnr
$(PNR_OUT): $(NETLIST) $(CONSTRAINT) $(SDC_CONSTRAINT)
	bash -e -o pipefail -c 'if [ "$(HAS_NEXTPNR)" != "1" ]; then \
			echo "ERROR: $(NEXTPNR) not found. Run '\''make check-tools'\'' to verify the FPGA toolchain."; \
			exit 1; \
		fi; \
		if [ "$(HAS_NEXTPNR_SDC)" = "1" ]; then \
			sdc_arg="--sdc $(abspath $(SDC_CONSTRAINT))"; \
		else \
			echo "INFO: $(NEXTPNR) does not support --sdc; keeping $(SDC_CONSTRAINT) as timing-intent input only."; \
			sdc_arg=""; \
		fi; \
		if [ "$(HAS_NEXTPNR_REPORT)" = "1" ]; then \
			report_arg="--report $(abspath $(TIMING))"; \
		else \
			echo "INFO: $(NEXTPNR) does not support --report; timing details will remain in $(BUILD_DIR)/nextpnr.log."; \
			report_arg=""; \
		fi; \
		$(PNR_CMD) $${sdc_arg} $${report_arg} 2>&1 | tee $(BUILD_DIR)/nextpnr.log'

# Generate bitstream
$(BIN): $(PNR_OUT)
	bash -e -o pipefail -c '$(PACK_CMD)'
endif

# Timing analysis
.PHONY: timing
ifeq ($(TARGET),artix7_alchitry_au)
timing: $(BIN)
	@if test -f $(TIMING); then \
		echo "Timing report written to $(TIMING)"; \
	elif test -f $(TIMING_SUMMARY); then \
		echo "Timing summary available at $(TIMING_SUMMARY)"; \
	else \
		echo "ERROR: Expected timing report at $(TIMING) or summary at $(TIMING_SUMMARY)"; \
		exit 1; \
	fi
else ifeq ($(TARGET),cyclonev_analogue_pocket)
timing: $(BIN)
	@if test -f $(TIMING); then \
		echo "Timing report written to $(TIMING)"; \
	else \
		echo "Timing details remain in $(BUILD_DIR)/output_files/"; \
	fi
else ifeq ($(TARGET),gowin_tang_primer_25k)
timing: $(BIN)
	@if test -f $(TIMING); then \
		echo "Timing report written to $(TIMING)"; \
	elif test -f $(TIMING_SUMMARY); then \
		echo "Timing summary available at $(TIMING_SUMMARY)"; \
	else \
		echo "Timing details remain in $(BUILD_DIR)/project/"; \
	fi
else
timing: $(PNR_OUT)
	@if test -f $(TIMING); then \
		echo "Timing report written to $(TIMING)"; \
	else \
		echo "Timing details available in $(BUILD_DIR)/nextpnr.log"; \
	fi
endif

# Resource utilization report
.PHONY: utilization
ifeq ($(TARGET),artix7_alchitry_au)
utilization: $(BIN)
	@test -f $(UTILIZATION) || (echo "ERROR: Expected utilization report at $(UTILIZATION)"; exit 1)
	@cat $(UTILIZATION)
else ifeq ($(TARGET),cyclonev_analogue_pocket)
utilization: $(BIN)
	@if test -f $(UTILIZATION); then \
		cat $(UTILIZATION); \
	else \
		echo "Quartus utilization details remain in $(BUILD_DIR)/output_files/"; \
	fi
else ifeq ($(TARGET),gowin_tang_primer_25k)
utilization: $(BIN)
	@if test -f $(UTILIZATION); then \
		cat $(UTILIZATION); \
	else \
		echo "Gowin utilization details remain in $(BUILD_DIR)/project/"; \
	fi
else
utilization: $(NETLIST)
	bash -e -o pipefail -c 'echo "Generating resource utilization report..."; echo "=== Resource Utilization ===" > $(UTILIZATION); grep -A 20 "Printing statistics" $(BUILD_DIR)/yosys.log >> $(UTILIZATION) || true; cat $(UTILIZATION)'
endif

# Standardized design stats summary
.PHONY: check-python
check-python:
	@$(PYTHON3) -c 'import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)' \
		|| (echo "ERROR: $(PYTHON3) 3.10+ is required for FPGA stats generation"; exit 1)

.PHONY: stats
stats: check-python
ifeq ($(TARGET),cyclonev_analogue_pocket)
	@echo "ERROR: standardized FPGA stats are not implemented yet for TARGET=$(TARGET)"
	@exit 1
else ifeq ($(TARGET),gowin_tang_primer_25k)
	@echo "ERROR: standardized FPGA stats are not implemented yet for TARGET=$(TARGET)"
	@exit 1
else
	@$(PYTHON3) $(STATS_SCRIPT) --target $(TARGET) --build --format $(STATS_FORMAT)
endif


# Program FPGA
.PHONY: program
ifeq ($(TARGET),cyclonev_analogue_pocket)
program: $(POCKET_PACKAGE_STAMP)
	@if [ -z "$(strip $(POCKET_DEPLOY_DIR))" ]; then \
		echo "ERROR: POCKET_DEPLOY_DIR is required for TARGET=$(TARGET)"; \
		echo "Usage: make TARGET=$(TARGET) program POCKET_DEPLOY_DIR=/path/to/deploy"; \
		exit 1; \
	fi
	@mkdir -p "$(POCKET_DEPLOY_DIR)"
	@set -e; \
	mounted=0; \
	umount_pocket() { \
		sync; \
		attempt=0; \
		while ! sudo umount "$(POCKET_DEPLOY_DIR)" 2>/dev/null; do \
			attempt=$$((attempt + 1)); \
			if [ "$$attempt" -ge "$(POCKET_UMOUNT_RETRIES)" ]; then \
				sudo umount "$(POCKET_DEPLOY_DIR)"; \
				return 1; \
			fi; \
			if [ "$$attempt" -eq 1 ]; then \
				echo "Waiting for Pocket mount to go idle before unmounting..."; \
			fi; \
			sleep "$(POCKET_UMOUNT_RETRY_DELAY)"; \
		done; \
		mounted=0; \
	}; \
	cleanup() { \
		if [ "$$mounted" -eq 1 ]; then \
			umount_pocket; \
		fi; \
	}; \
	trap cleanup EXIT; \
	package_path=""; \
	if [ -f "$(POCKET_PACKAGE_INFO)" ]; then \
		package_path="$$(cat $(POCKET_PACKAGE_INFO))"; \
	fi; \
	if [ -z "$$package_path" ] || [ ! -f "$$package_path" ]; then \
		$(POCKET_PACKAGE_CMD); \
		package_path="$$(cat $(POCKET_PACKAGE_INFO))"; \
	fi; \
	if [ ! -f "$$package_path" ]; then \
		echo "ERROR: Pocket package not found at $$package_path"; \
		echo "Run 'rm -f $(POCKET_PACKAGE_INFO) && make TARGET=$(TARGET) program POCKET_DEPLOY_DIR=$(POCKET_DEPLOY_DIR)' to regenerate the package before retrying deployment."; \
		exit 1; \
	fi; \
	sudo mount -U $(POCKET_DEPLOY_UUID) "$(POCKET_DEPLOY_DIR)" -o uid=$(POCKET_DEPLOY_UID),gid=$(POCKET_DEPLOY_GID),umask=$(POCKET_DEPLOY_UMASK); \
	mounted=1; \
	$(UNZIP) -o "$$package_path" -d "$(POCKET_DEPLOY_DIR)"; \
	umount_pocket
else
program: $(BIN)
	bash -e -o pipefail -c '$(PROGRAM_CMD)'
endif

# Clean build artifacts
.PHONY: clean
clean:
	rm -rf $(BUILD_DIR)
	rm -f $(POCKET_TEST_PROGRAM_ASSET)

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
else ifeq ($(TARGET),cyclonev_analogue_pocket)
	@echo "  $(BIN)       - Run the Quartus batch flow and emit the Pocket bring-up image"
else ifeq ($(TARGET),gowin_tang_primer_25k)
	@echo "  $(BIN)       - Run the Gowin batch flow and generate the Tang Primer 25K bitstream"
else
	@echo "  $(NETLIST)   - Run synthesis only (Yosys)"
	@echo "  $(PNR_OUT)   - Run place and route (nextpnr)"
	@echo "  $(BIN)       - Generate bitstream (target-specific packer)"
endif
ifeq ($(TARGET),artix7_alchitry_au)
	@echo "  timing       - Generate timing report (written to $(TIMING))"
else ifeq ($(TARGET),cyclonev_analogue_pocket)
	@echo "  timing       - Show $(TIMING) when copied from Quartus output_files"
else ifeq ($(TARGET),gowin_tang_primer_25k)
	@echo "  timing       - Show $(TIMING) or $(TIMING_SUMMARY) copied from the Gowin project"
else
	@echo "  timing       - Show $(TIMING) when available, otherwise fall back to $(BUILD_DIR)/nextpnr.log"
endif
	@echo "  utilization  - Show resource utilization"
ifeq ($(TARGET),cyclonev_analogue_pocket)
	@echo "  stats        - Not supported yet for this Quartus target"
else ifeq ($(TARGET),gowin_tang_primer_25k)
	@echo "  stats        - Not supported yet for this Gowin target"
else
	@echo "  stats        - Build and print standardized timing/utilization stats"
endif
ifeq ($(TARGET),cyclonev_analogue_pocket)
	@echo "  program      - Mount the Pocket SD card at POCKET_DEPLOY_DIR, unzip the generated core package, then unmount it"
else
	@echo "  program      - Program FPGA with bitstream"
endif
	@echo "  clean        - Remove build artifacts"
	@echo "  help         - Show this help message"
	@echo ""
	@echo "Prerequisites:"
ifeq ($(TARGET),artix7_alchitry_au)
	@echo "  - vivado     : Xilinx Vivado batch/CLI tools"
	@echo "  - openFPGALoader : Board programming tool (required for 'make program')"
else ifeq ($(TARGET),cyclonev_analogue_pocket)
	@echo "  - quartus_sh : Intel Quartus batch/CLI tools"
	@echo "  - cargo      : Rust toolchain (required for the Pocket packaging step)"
	@echo "  - $(RISCV_OBJCOPY) : RISC-V objcopy used for test_pocket_demo.bin (override RISCV_OBJCOPY=/path/to/objcopy if needed)"
	@echo "  - cmp        : Binary comparison tool used to avoid rewriting unchanged Pocket assets"
	@echo "  - unzip      : Archive extraction tool (required for 'make program')"
	@echo "  - sudo mount/umount : Required to mount the Pocket SD card by UUID $(POCKET_DEPLOY_UUID)"
else ifeq ($(TARGET),gowin_tang_primer_25k)
	@echo "  - gw_sh      : Gowin EDA batch/CLI tool"
	@echo "  - openFPGALoader : Board programming tool (required for 'make program')"
else
	@echo "  - yosys      : Verilog synthesis"
	@echo "  - nextpnr    : Place and route"
	@echo "  - ecppack    : ECP5 bitstream generation"
	@echo "  - openFPGALoader : Board programming tool (required for 'make program')"
endif
	@echo ""
	@echo "Target selection:"
	@echo "  - make TARGET=ecp5_icepi_zero  (default: iCE Pi Zero ECP5-25F)"
	@echo "  - make TARGET=artix7_alchitry_au  (Alchitry Au Artix-7)"
	@echo "  - make TARGET=cyclonev_analogue_pocket  (Analogue Pocket Cyclone V bring-up)"
	@echo "  - make TARGET=gowin_tang_primer_25k  (Sipeed Tang Primer 25K bring-up)"
	@echo ""
	@echo "Stats output format:"
	@echo "  - make TARGET=ecp5_icepi_zero stats STATS_FORMAT=text"
	@echo "  - make TARGET=ecp5_icepi_zero stats STATS_FORMAT=json"
	@echo "  - make TARGET=ecp5_icepi_zero stats STATS_FORMAT=markdown"
	@echo ""
	@echo "Programming selection:"
ifeq ($(TARGET),cyclonev_analogue_pocket)
	@echo "  - make program POCKET_DEPLOY_DIR=/path/to/mountpoint"
	@echo "    mounts Pocket SD UUID $(POCKET_DEPLOY_UUID), deploys the packaged core, then unmounts"
	@echo "  - make TARGET=$(TARGET)           (builds $(BIN) and packages a deployable Pocket zip)"
else
	@echo "  - make program PROGRAM_MODE=$(DEFAULT_PROGRAM_MODE)   (default for TARGET=$(TARGET))"
ifneq ($(DEFAULT_PROGRAM_MODE),sram)
	@echo "  - make program PROGRAM_MODE=sram"
endif
ifneq ($(DEFAULT_PROGRAM_MODE),flash)
	@echo "  - make program PROGRAM_MODE=flash"
endif
endif

.PHONY: check-tools
check-tools: check-python
	@echo "Checking for required tools..."
ifeq ($(TARGET),artix7_alchitry_au)
	@which $(VIVADO) > /dev/null || (echo "ERROR: $(VIVADO) not found"; exit 1)
	@which $(OPENFPGALOADER) > /dev/null || (echo "WARNING: openFPGALoader not found (required for 'make program')"; exit 0)
else ifeq ($(TARGET),cyclonev_analogue_pocket)
	@which $(QUARTUS_SH) > /dev/null || (echo "ERROR: $(QUARTUS_SH) not found"; exit 1)
	@which $(CARGO) > /dev/null || (echo "ERROR: $(CARGO) not found (required for Pocket packaging)"; exit 1)
	@which $(RISCV_OBJCOPY) > /dev/null || (echo "ERROR: $(RISCV_OBJCOPY) not found (required to convert test_pocket_demo into a Pocket .bin; override RISCV_OBJCOPY=/path/to/objcopy if needed)"; exit 1)
	@which cmp > /dev/null || (echo "ERROR: cmp not found (required to avoid rewriting unchanged Pocket assets)"; exit 1)
	@which $(UNZIP) > /dev/null || (echo "ERROR: $(UNZIP) not found (required for 'make program')"; exit 1)
else ifeq ($(TARGET),gowin_tang_primer_25k)
	@which $(GOWIN_SH) > /dev/null || (echo "ERROR: $(GOWIN_SH) not found"; exit 1)
	@which $(OPENFPGALOADER) > /dev/null || (echo "WARNING: openFPGALoader not found (required for 'make program')"; exit 0)
else
	@which $(YOSYS) > /dev/null || (echo "ERROR: yosys not found"; exit 1)
	@which $(NEXTPNR) > /dev/null || (echo "ERROR: $(NEXTPNR) not found"; exit 1)
ifeq ($(TARGET),ecp5_icepi_zero)
	@which $(ECPPACK) > /dev/null || (echo "ERROR: ecppack not found"; exit 1)
	@which $(OPENFPGALOADER) > /dev/null || (echo "WARNING: openFPGALoader not found (required for 'make program')"; exit 0)
endif
endif
	@echo "All required tools found!"
