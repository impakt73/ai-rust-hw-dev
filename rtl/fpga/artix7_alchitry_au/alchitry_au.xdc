# Alchitry Au (Artix-7 XC7A35T) XDC constraints

# Clock: 100 MHz on-board oscillator
create_clock -name clk_100mhz -period 10.000 [get_ports clk]
set_property PACKAGE_PIN N14 [get_ports clk]
set_property IOSTANDARD LVCMOS33 [get_ports clk]

# Reset button (active low)
set_property PACKAGE_PIN P6 [get_ports rst_n_btn]
set_property IOSTANDARD LVCMOS33 [get_ports rst_n_btn]
set_property PULLUP true [get_ports rst_n_btn]

# Status LEDs
set_property PACKAGE_PIN K13 [get_ports {led[0]}]
set_property IOSTANDARD LVCMOS33 [get_ports {led[0]}]

set_property PACKAGE_PIN K12 [get_ports {led[1]}]
set_property IOSTANDARD LVCMOS33 [get_ports {led[1]}]

set_property PACKAGE_PIN L14 [get_ports {led[2]}]
set_property IOSTANDARD LVCMOS33 [get_ports {led[2]}]

set_property PACKAGE_PIN L13 [get_ports {led[3]}]
set_property IOSTANDARD LVCMOS33 [get_ports {led[3]}]

set_property PACKAGE_PIN M16 [get_ports {led[4]}]
set_property IOSTANDARD LVCMOS33 [get_ports {led[4]}]

set_property PACKAGE_PIN M14 [get_ports {led[5]}]
set_property IOSTANDARD LVCMOS33 [get_ports {led[5]}]

set_property PACKAGE_PIN M12 [get_ports {led[6]}]
set_property IOSTANDARD LVCMOS33 [get_ports {led[6]}]

set_property PACKAGE_PIN N16 [get_ports {led[7]}]
set_property IOSTANDARD LVCMOS33 [get_ports {led[7]}]

# USB UART
set_property PACKAGE_PIN P15 [get_ports usb_rx]
set_property IOSTANDARD LVCMOS33 [get_ports usb_rx]
set_property PACKAGE_PIN P16 [get_ports usb_tx]
set_property IOSTANDARD LVCMOS33 [get_ports usb_tx]

# Timing exceptions for asynchronous external I/O.
# Cut async inputs only to the first synchronizer stage, and cut async outputs
# that have no synchronous board-level capture requirement.
set_false_path \
    -from [get_ports rst_n_btn] \
    -to [get_pins -hierarchical -filter {NAME =~ *rst_n_btn_sync_inst*sync_regs_reg[[]0[]]/D}]

set_false_path \
    -from [get_ports usb_rx] \
    -to [get_pins -hierarchical -filter {NAME =~ *host_uart_inst*rx_sync_inst*sync_regs_reg[[]0[]]/D}]

set_false_path -to [get_ports {usb_tx led[*]}]
