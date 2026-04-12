#
# user core constraints
#
# put your clock groups in here as well as any net assignments
#

derive_pll_clocks

set dram_cont_clk "ic|mp2|mf_pllbase2_inst|altera_pll_i|general[0].gpll~PLL_OUTPUT_COUNTER|divclk"
set dram_chip_pll_clk "ic|mp2|mf_pllbase2_inst|altera_pll_i|general[1].gpll~PLL_OUTPUT_COUNTER|divclk"
# The SDRAM pin clock now comes from the DDIO clock forwarder, so constrain the
# external interface against the forwarded dram_clk port rather than the raw PLL
# leg feeding the DDIO primitive.
create_generated_clock -name dram_chip_clk \
 -source [get_pins {ic|sdram_clk_forward_inst|outclock}] \
 -master_clock $dram_chip_pll_clk \
 -divide_by 1 \
 [get_ports {dram_clk}]

set_clock_groups -asynchronous \
 -group { bridge_spiclk } \
 -group { clk_74a } \
 -group { clk_74b } \
 -group { ic|mp1|mf_pllbase_inst|altera_pll_i|general[0].gpll~PLL_OUTPUT_COUNTER|divclk } \
 -group { ic|mp1|mf_pllbase_inst|altera_pll_i|general[1].gpll~PLL_OUTPUT_COUNTER|divclk } \
 -group { ic|mp1|mf_pllbase_inst|altera_pll_i|general[2].gpll~PLL_OUTPUT_COUNTER|divclk } \
 -group { ic|mp2|mf_pllbase2_inst|altera_pll_i|general[0].gpll~PLL_OUTPUT_COUNTER|divclk } \
 -group { ic|mp2|mf_pllbase2_inst|altera_pll_i|general[1].gpll~PLL_OUTPUT_COUNTER|divclk dram_chip_clk }

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
set_input_delay -clock [get_clocks {dram_chip_clk}] -reference_pin [get_ports {dram_clk}] -max 5.9 [get_ports {dram_dq[*]}]
set_input_delay -clock [get_clocks {dram_chip_clk}] -reference_pin [get_ports {dram_clk}] -min 0.9 [get_ports {dram_dq[*]}]

# Unlike the example repo, Pocket writes also drive the mask pins and write data
# from this interface, so keep the same conservative write window but also
# constrain dram_dqm[*] and the write-side dram_dq[*] here.
set_output_delay -clock [get_clocks {dram_chip_clk}] -reference_pin [get_ports {dram_clk}] -max 2.0 \
    [get_ports {dram_cke dram_a* dram_ba* dram_cas_n dram_ras_n dram_we_n dram_dqm[*] dram_dq[*]}]
set_output_delay -clock [get_clocks {dram_chip_clk}] -reference_pin [get_ports {dram_clk}] -min -1.0 \
    [get_ports {dram_cke dram_a* dram_ba* dram_cas_n dram_ras_n dram_we_n dram_dqm[*] dram_dq[*]}]

# The controller and chip clocks come from the same PLL, but the SDRAM chip clock
# is phase-shifted late enough that read data is captured against the following
# controller edge rather than the immediately preceding one.
set_multicycle_path -from [get_clocks {dram_chip_clk}] -to [get_clocks $dram_cont_clk] -setup -end 2
set_multicycle_path -from [get_clocks {dram_chip_clk}] -to [get_clocks $dram_cont_clk] -hold -end 1
