set_clock_groups -asynchronous \
    -group {clk_74a} \
    -group {clk_74b}

set_false_path -to [get_cells -hier -filter {NAME =~ *reset_bridge*/reset_sync_regs*}]
