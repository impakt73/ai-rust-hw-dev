# Timing exceptions for asynchronous external I/O on the Alchitry Cu target.
#
# Best practice:
# - Cut asynchronous inputs only to the first synchronizer stage.
# - Cut asynchronous outputs that have no board-level capture clock requirement.

# Active-low pushbutton reset enters a dedicated 2-FF synchronizer.
set_false_path \
    -from [get_ports rst_n_btn] \
    -to [get_pins -hierarchical -filter {NAME =~ *rst_n_btn_sync_inst*sync_regs_reg[[]0[]]/D}]

# UART RX is asynchronous to sys_clk and enters the UART's 3-FF input synchronizer.
set_false_path \
    -from [get_ports usb_rx] \
    -to [get_pins -hierarchical -filter {NAME =~ *host_uart_inst*rx_sync_inst*sync_regs_reg[[]0[]]/D}]

# Front-panel buttons are asynchronous human inputs and are synchronized in the top level.
set_false_path \
    -from [get_ports {io_button[*]}] \
    -to [get_pins -hierarchical -filter {NAME =~ *io_button_sync1_reg*/D}]

# These outputs are asynchronous off-chip endpoints with no synchronous capture clock.
set_false_path -to [get_ports {usb_tx led[*] io_led[*] io_sel[*] io_seg[*]}]
