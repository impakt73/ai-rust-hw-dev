#!/bin/bash
# Generate resource analysis report from synthesis logs

BUILD_DIR="build"

echo "# FPGA Resource Analysis Report"
echo ""
echo "This report analyzes the resource consumption of each RTL module"
echo "in the RISC-V CPU design when synthesized for the iCE40-HX8K FPGA."
echo ""
echo "## Target Device: Lattice iCE40-HX8K"
echo ""
echo "### Available Resources:"
echo "- **Logic Cells (LUTs):** 7,680"
echo "- **Flip-Flops (FFs):** 7,680"
echo "- **Block RAM (BRAM):** 32 (4Kbit each = 16KB total)"
echo ""
echo "---"
echo ""
echo "## Module Resource Summary"
echo ""
echo "| Module | LCs | FFs | BRAM | % of Device | Notes |"
echo "|--------|-----|-----|------|-------------|-------|"

# List of modules to process
MODULES=(
    "alu"
    "branch_unit"
    "csr_file"
    "decoder"
    "decompress"
    "div_unit"
    "fetch_buffer"
    "fp_regfile"
    "fpu"
    "fpu_adder"
    "fpu_classifier"
    "fpu_comparator"
    "fpu_div_assemble"
    "fpu_div_setup"
    "fpu_float_to_int"
    "fpu_fma"
    "fpu_int_to_float"
    "fpu_multiplier"
    "fpu_sqrt"
    "mem_interface"
    "regfile"
    "writeback_mux"
)

# iCE40-HX8K has 7680 LUTs
TOTAL_LUTS=7680

# Function to extract stats from log file
extract_stats() {
    local log_file="$1"
    local module="$2"
    
    if [ ! -f "$log_file" ]; then
        echo "| $module | N/A | N/A | N/A | N/A | Synthesis failed |"
        return
    fi
    
    # Extract cell counts from statistics section
    local cells=$(grep -A 30 "Printing statistics" "$log_file" | grep "Number of cells:" | head -1 | awk '{print $NF}')
    local sbs=$(grep -A 30 "Printing statistics" "$log_file" | grep "SB_LUT4" | head -1 | awk '{print $NF}')
    local dffs=$(grep -A 30 "Printing statistics" "$log_file" | grep -E "SB_DFF" | head -1 | awk '{print $NF}')
    local rams=$(grep -A 30 "Printing statistics" "$log_file" | grep "SB_RAM" | head -1 | awk '{print $NF}')
    
    # Default to 0 if not found
    [ -z "$sbs" ] && sbs=0
    [ -z "$dffs" ] && dffs=0
    [ -z "$rams" ] && rams=0
    
    # Calculate percentage
    local pct="0"
    if [ "$sbs" -gt 0 ]; then
        pct=$(echo "scale=1; $sbs * 100 / $TOTAL_LUTS" | bc)
    fi
    
    # Determine notes based on resource usage
    local notes=""
    if [ "$sbs" -gt 2000 ]; then
        notes="⚠️ HIGH"
    elif [ "$sbs" -gt 1000 ]; then
        notes="⚡ Medium"
    elif [ "$sbs" -gt 0 ]; then
        notes="✅ Low"
    else
        notes="❓ Check"
    fi
    
    # Special notes for specific modules
    if [ "$module" == "csr_file" ] && [ "$rams" -gt 0 ]; then
        notes="$notes (BRAM)"
    fi
    
    echo "| $module | $sbs | $dffs | $rams | ${pct}% | $notes |"
}

# Process each module
for mod in "${MODULES[@]}"; do
    extract_stats "$BUILD_DIR/${mod}.log" "$mod"
done

# Full CPU if available
if [ -f "$BUILD_DIR/full_cpu.log" ]; then
    echo ""
    echo "### Full CPU (Baseline)"
    echo ""
    extract_stats "$BUILD_DIR/full_cpu.log" "**FULL CPU**"
fi

echo ""
echo "---"
echo ""
echo "## Detailed Analysis"
echo ""

# Print detailed stats for high-resource modules
echo "### High Resource Modules (>1000 LUTs)"
echo ""

for mod in "${MODULES[@]}"; do
    log_file="$BUILD_DIR/${mod}.log"
    if [ -f "$log_file" ]; then
        sbs=$(grep -A 30 "Printing statistics" "$log_file" | grep "SB_LUT4" | head -1 | awk '{print $NF}')
        [ -z "$sbs" ] && sbs=0
        
        if [ "$sbs" -gt 1000 ]; then
            echo "#### $mod ($sbs LUTs)"
            echo ""
            echo "\`\`\`"
            grep -A 25 "Printing statistics" "$log_file" | head -30
            echo "\`\`\`"
            echo ""
        fi
    fi
done

echo "---"
echo ""
echo "## Recommendations"
echo ""
echo "Based on the analysis above, the following modules may need optimization:"
echo ""

# Identify problematic modules
for mod in "${MODULES[@]}"; do
    log_file="$BUILD_DIR/${mod}.log"
    if [ -f "$log_file" ]; then
        sbs=$(grep -A 30 "Printing statistics" "$log_file" | grep "SB_LUT4" | head -1 | awk '{print $NF}')
        [ -z "$sbs" ] && sbs=0
        
        if [ "$sbs" -gt 2000 ]; then
            echo "- **$mod** ($sbs LUTs): Consider optimization or making optional"
        fi
    fi
done

echo ""
echo "---"
echo ""
echo "*Report generated on $(date)*"
