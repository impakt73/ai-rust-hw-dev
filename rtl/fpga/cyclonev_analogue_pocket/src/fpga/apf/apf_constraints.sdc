create_clock -name clk_74a -period 13.468 [get_ports {clk_74a}]
create_clock -name clk_74b -period 13.468 [get_ports {clk_74b}]
set_clock_groups -asynchronous -group {clk_74a} -group {clk_74b}
set_false_path -to [get_cells -hier -filter {NAME =~ *reset_bridge*/reset_sync_regs*}]
