# Timing exceptions for asynchronous external I/O on the Alchitry Cu target.
#
# Best practice:
# - Cut asynchronous input-originated timing paths broadly.
# - Cut asynchronous outputs that have no board-level capture clock requirement.

# 100 MHz on-board oscillator drives the full design directly.
create_clock -name clk_100mhz -period 10.000 [get_ports clk]

# Active-low pushbutton reset is asynchronous to the design clocks.
set_false_path -from [get_ports rst_n_btn]

# UART RX is asynchronous to sys_clk.
set_false_path -from [get_ports usb_rx]

# Front-panel buttons are asynchronous human inputs.
set_false_path -from [get_ports {io_button[*]}]

# These outputs are asynchronous off-chip endpoints with no synchronous capture clock.
set_false_path -to [get_ports {usb_tx led[*] io_led[*] io_sel[*] io_seg[*]}]
