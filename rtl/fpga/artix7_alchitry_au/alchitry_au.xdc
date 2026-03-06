# Alchitry Au (Artix-7 XC7A35T) XDC constraints

# Clock: 100 MHz on-board oscillator
create_clock -name clk_100mhz -period 10.000 [get_ports clk]
set_property PACKAGE_PIN N14 [get_ports clk]
set_property IOSTANDARD LVCMOS33 [get_ports clk]

# Reset button (active low)
set_property PACKAGE_PIN P6 [get_ports rst_n_btn]
set_property IOSTANDARD LVCMOS33 [get_ports rst_n_btn]
set_property PULLUP true [get_ports rst_n_btn]

# Status LED
set_property PACKAGE_PIN K13 [get_ports led]
set_property IOSTANDARD LVCMOS33 [get_ports led]

# USB UART
set_property PACKAGE_PIN P16 [get_ports usb_rx]
set_property IOSTANDARD LVCMOS33 [get_ports usb_rx]
set_property PACKAGE_PIN P15 [get_ports usb_tx]
set_property IOSTANDARD LVCMOS33 [get_ports usb_tx]
