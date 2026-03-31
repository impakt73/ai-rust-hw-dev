# Timing exceptions for asynchronous external I/O on the Tang Primer 25K target.
#
# Best practice:
# - Cut asynchronous input-originated timing paths broadly.
# - Cut asynchronous outputs that have no board-level capture clock requirement.

# 50 MHz on-board oscillator.
create_clock -name clk_50mhz -period 20.000 [get_ports {clk}]

# Active-low pushbutton reset is asynchronous to the design clocks.
set_false_path -from [get_ports {rst_n_btn}]

# UART RX is asynchronous to sys_clk.
set_false_path -from [get_ports {usb_rx}]

# These outputs are asynchronous off-chip endpoints with no synchronous capture clock.
set_false_path -to [get_ports {usb_tx led led_done led_ready}]
