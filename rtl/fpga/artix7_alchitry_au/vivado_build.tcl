if {[llength $argv] < 5} {
    puts stderr "Usage: vivado_build.tcl <build_dir> <top_module> <fpga_part> <constraint_file> <source_files...>"
    exit 1
}

set build_dir [file normalize [lindex $argv 0]]
set top_module [lindex $argv 1]
set fpga_part [lindex $argv 2]
set constraint_file [file normalize [lindex $argv 3]]
set rtl_sources [lrange $argv 4 end]

file mkdir $build_dir
cd $build_dir
create_project -in_memory -part $fpga_part

foreach rtl_source $rtl_sources {
    read_verilog -sv [file normalize $rtl_source]
}

read_xdc $constraint_file

synth_design -top $top_module
write_checkpoint -force riscv_fpga_synth.dcp
opt_design
place_design
phys_opt_design
write_checkpoint -force riscv_fpga_placed.dcp
route_design

report_timing -max_paths 10 -file riscv_fpga_timing.rpt
report_timing_summary -file riscv_fpga_timing_summary.rpt
report_utilization -file riscv_fpga_utilization.rpt
write_checkpoint -force riscv_fpga_routed.dcp
write_bitstream -force riscv_fpga.bit
