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

# Alliance Memory AS4C32M16MSA-6BIN (-6 speed grade) AC timing:
#   tAC(max)=5.5 ns, tOH(min)=1.5 ns, tSU(min)=1.5 ns, tIH(min)=0.8 ns.
set_input_delay -clock $dram_chip_clk -reference_pin [get_ports {dram_clk}] -max 5.5 [get_ports {dram_dq[*]}]
set_input_delay -clock $dram_chip_clk -reference_pin [get_ports {dram_clk}] -min 1.5 [get_ports {dram_dq[*]}]

set_output_delay -clock $dram_chip_clk -reference_pin [get_ports {dram_clk}] -max 1.5 \
    [get_ports {dram_cke dram_a* dram_ba* dram_cas_n dram_ras_n dram_we_n dram_dqm[*] dram_dq[*]}]
set_output_delay -clock $dram_chip_clk -reference_pin [get_ports {dram_clk}] -min -0.8 \
    [get_ports {dram_cke dram_a* dram_ba* dram_cas_n dram_ras_n dram_we_n dram_dqm[*] dram_dq[*]}]

# The controller and chip clocks come from the same PLL, but the SDRAM chip clock
# is phase-shifted late enough that read data is captured against the following
# controller edge rather than the immediately preceding one.
set_multicycle_path -from $dram_chip_clk -to $dram_cont_clk -setup -end 2
set_multicycle_path -from $dram_chip_clk -to $dram_cont_clk -hold -end 1
