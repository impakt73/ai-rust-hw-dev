#
# user core constraints
#
# put your clock groups in here as well as any net assignments
#

set_clock_groups -asynchronous \
 -group { bridge_spiclk } \
 -group { clk_74a } \
 -group { clk_74b } \
 -group { ic|mp1|mf_pllbase_inst|altera_pll_i|general[0].gpll~PLL_OUTPUT_COUNTER|divclk } \
 -group { ic|mp1|mf_pllbase_inst|altera_pll_i|general[1].gpll~PLL_OUTPUT_COUNTER|divclk } \
 -group { ic|mp1|mf_pllbase_inst|altera_pll_i|general[2].gpll~PLL_OUTPUT_COUNTER|divclk } \
 -group { ic|mp2|mf_pllbase2_inst|altera_pll_i|general[0].gpll~PLL_OUTPUT_COUNTER|divclk } \
 -group { ic|mp2|mf_pllbase2_inst|altera_pll_i|general[1].gpll~PLL_OUTPUT_COUNTER|divclk }

set dram_cont_clk "ic|mp2|mf_pllbase2_inst|altera_pll_i|general[0].gpll~PLL_OUTPUT_COUNTER|divclk"
set dram_chip_clk "ic|mp2|mf_pllbase2_inst|altera_pll_i|general[1].gpll~PLL_OUTPUT_COUNTER|divclk"

# The OpenFPGA basicassets example constrains a different PLL clock pair
# (mp1 general[2]/[3]). Pocket SDRAM is wired to the dedicated 133 MHz mp2
# outputs instead, so keep the constraints bound to those clocks.
#
# The example repo also uses broader 5.9/0.9 ns read and 2.0/-1.0 ns write
# windows that are not the raw AS4C32M16MSA-6BIN AC table entries. Here we keep
# the values directly traceable to the Pocket SDRAM datasheet limits for this
# device/speed grade and let Quartus account for FPGA-side routing/clock skew.
# Alliance Memory AS4C32M16MSA-6BIN (-6 speed grade) AC timing:
#   tAC(max)=5.5 ns, tOH(min)=1.5 ns, tSU(min)=1.5 ns, tIH(min)=0.8 ns.
set_input_delay -clock $dram_chip_clk -reference_pin [get_ports {dram_clk}] -max 5.5 [get_ports {dram_dq[*]}]
set_input_delay -clock $dram_chip_clk -reference_pin [get_ports {dram_clk}] -min 1.5 [get_ports {dram_dq[*]}]

# Unlike the example repo, Pocket writes also drive the mask pins and write data
# from this interface, so constrain dram_dqm[*] and the write-side dram_dq[*]
# against the SDRAM setup/hold requirements too.
set_output_delay -clock $dram_chip_clk -reference_pin [get_ports {dram_clk}] -max 1.5 \
    [get_ports {dram_cke dram_a* dram_ba* dram_cas_n dram_ras_n dram_we_n dram_dqm[*] dram_dq[*]}]
set_output_delay -clock $dram_chip_clk -reference_pin [get_ports {dram_clk}] -min -0.8 \
    [get_ports {dram_cke dram_a* dram_ba* dram_cas_n dram_ras_n dram_we_n dram_dqm[*] dram_dq[*]}]

# The controller and chip clocks come from the same PLL, but the SDRAM chip clock
# is phase-shifted late enough that read data is captured against the following
# controller edge rather than the immediately preceding one.
set_multicycle_path -from $dram_chip_clk -to $dram_cont_clk -setup -end 2
set_multicycle_path -from $dram_chip_clk -to $dram_cont_clk -hold -end 1
