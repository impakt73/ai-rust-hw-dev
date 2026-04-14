#
# user core constraints
#
# put your clock groups in here as well as any net assignments
#

derive_pll_clocks

set dram_cont_clk_name "ic|mp2|mf_pllbase2_inst|altera_pll_i|general[0].gpll~PLL_OUTPUT_COUNTER|divclk"
set dram_cont_clk [get_clocks $dram_cont_clk_name]
set dram_chip_pll_source_clk_name "ic|mp2|mf_pllbase2_inst|altera_pll_i|general[1].gpll~PLL_OUTPUT_COUNTER|divclk"
set dram_chip_pll_source_clk [get_clocks $dram_chip_pll_source_clk_name]
set dram_samp_clk_name "ic|mp2|mf_pllbase2_inst|altera_pll_i|general[2].gpll~PLL_OUTPUT_COUNTER|divclk"
set dram_samp_clk [get_clocks $dram_samp_clk_name]
# The SDRAM pin clock now comes from the DDIO clock forwarder, so constrain the
# external interface against the forwarded dram_clk port rather than the raw PLL
# leg feeding the DDIO primitive.
create_generated_clock -name dram_chip_clk \
 -source [get_pins {ic|sdram_clk_forward_inst|outclock}] \
 -master_clock $dram_chip_pll_source_clk \
 -divide_by 1 \
 [get_ports {dram_clk}]
set dram_chip_clk [get_clocks {dram_chip_clk}]
set sdram_clock_group [get_clocks [list $dram_cont_clk_name $dram_chip_pll_source_clk_name $dram_samp_clk_name dram_chip_clk]]

set_clock_groups -asynchronous \
 -group { bridge_spiclk } \
 -group { clk_74a } \
 -group { clk_74b } \
 -group { ic|mp1|mf_pllbase_inst|altera_pll_i|general[0].gpll~PLL_OUTPUT_COUNTER|divclk } \
 -group { ic|mp1|mf_pllbase_inst|altera_pll_i|general[1].gpll~PLL_OUTPUT_COUNTER|divclk } \
 -group { ic|mp1|mf_pllbase_inst|altera_pll_i|general[2].gpll~PLL_OUTPUT_COUNTER|divclk } \
 -group $sdram_clock_group

# The OpenFPGA basicassets example constrains a different PLL clock pair
# (mp1 general[2]/[3]). Pocket SDRAM is wired to the dedicated 133 MHz mp2
# outputs instead, so keep the constraints bound to those clocks.
#
# Public Pocket/openFPGA SDRAM examples use a wider external timing envelope
# than the raw AS4C32M16MSA-6BIN (-6) AC table to leave room for board/package
# skew. Both basicassets and Mazamars312/Analogue_Pocket_Neogeo use 5.9/0.9 ns
# for read capture and 2.0/-1.0 ns for write launch at 133 MHz, so use that
# conservative window here too instead of the bare tAC/tOH/tSU/tIH numbers
# (5.5/1.5/1.5/0.8 ns).
set_input_delay -clock $dram_chip_clk -reference_pin [get_ports {dram_clk}] -max 5.9 [get_ports {dram_dq[*]}]
set_input_delay -clock $dram_chip_clk -reference_pin [get_ports {dram_clk}] -min 0.9 [get_ports {dram_dq[*]}]

# Unlike the example repo, Pocket writes also drive the mask pins and write data
# from this interface, so keep the same conservative write window but also
# constrain dram_dqm[*] and the write-side dram_dq[*] here.
set_output_delay -clock $dram_chip_clk -reference_pin [get_ports {dram_clk}] -max 2.0 \
    [get_ports {dram_cke dram_a* dram_ba* dram_cas_n dram_ras_n dram_we_n dram_dqm[*] dram_dq[*]}]
set_output_delay -clock $dram_chip_clk -reference_pin [get_ports {dram_clk}] -min -1.0 \
    [get_ports {dram_cke dram_a* dram_ba* dram_cas_n dram_ras_n dram_we_n dram_dqm[*] dram_dq[*]}]

# The SDRAM controller, forwarded chip clock, and sample clock now remain in the
# same synchronous clock family so TimeQuest can analyze the quarter-cycle launch
# and capture paths directly.
