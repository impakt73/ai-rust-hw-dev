if {[llength $argv] < 5} {
    puts stderr "Usage: vivado_build.tcl <build_dir> <top_module> <part> <constraint> <sources...>"
    exit 1
}

set build_dir [file normalize [lindex $argv 0]]
set top_module [lindex $argv 1]
set fpga_part [lindex $argv 2]
set constraint_file [file normalize [lindex $argv 3]]
set rtl_sources [lrange $argv 4 end]

file mkdir $build_dir

foreach rtl_source $rtl_sources {
    read_verilog -sv [file normalize $rtl_source]
}

read_xdc $constraint_file

synth_design -top $top_module -part $fpga_part
opt_design
place_design
phys_opt_design
route_design

report_timing_summary -file [file join $build_dir riscv_fpga_timing.rpt]
report_utilization -file [file join $build_dir riscv_fpga_utilization.rpt]
write_checkpoint -force [file join $build_dir riscv_fpga_routed.dcp]
write_bitstream -force [file join $build_dir riscv_fpga.bit]
