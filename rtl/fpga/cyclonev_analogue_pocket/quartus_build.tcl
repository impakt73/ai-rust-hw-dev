package require ::quartus::project
package require ::quartus::flow

if {[llength $argv] < 3} {
    puts stderr "Usage: quartus_build.tcl <build_dir> <qsf_file> <top_module> <source_files...>"
    exit 1
}

set build_dir [file normalize [lindex $argv 0]]
set qsf_file [file normalize [lindex $argv 1]]
set top_module [lindex $argv 2]
set rtl_sources [lrange $argv 3 end]
set qsf_dir [file dirname $qsf_file]
set output_dir [file join $build_dir output_files]

file mkdir $build_dir
file mkdir $output_dir
file mkdir [file join $build_dir apf]
if {[file exists $qsf_file]} {
    file copy -force $qsf_file [file join $build_dir [file tail $qsf_file]]
}
set qpf_file [file rootname $qsf_file]
append qpf_file ".qpf"
if {[file exists $qpf_file]} {
    file copy -force $qpf_file [file join $build_dir [file tail $qpf_file]]
}
cd $build_dir

project_new -overwrite ap_core -revision ap_core
set_global_assignment -name FAMILY "Cyclone V"
set_global_assignment -name DEVICE 5CEBA4F23C8
set_global_assignment -name TOP_LEVEL_ENTITY $top_module
set_global_assignment -name PROJECT_OUTPUT_DIRECTORY $output_dir
set_global_assignment -name PRE_FLOW_SCRIPT_FILE "quartus_sh:[file join $qsf_dir apf build_id_gen.tcl]"
set_global_assignment -name SDC_FILE [file join $qsf_dir apf apf_constraints.sdc]
set_global_assignment -name SDC_FILE [file join $qsf_dir core core_constraints.sdc]
set_global_assignment -name GENERATE_RBF_FILE ON
set_global_assignment -name ON_CHIP_BITSTREAM_DECOMPRESSION ON

foreach rtl_source $rtl_sources {
    set normalized_source [file normalize $rtl_source]
    set_global_assignment -name SYSTEMVERILOG_FILE $normalized_source
}

export_assignments
execute_flow -compile
project_close

foreach pair {
    {ap_core.sof riscv_fpga.sof}
    {ap_core.fit.rpt riscv_fpga_utilization.rpt}
    {ap_core.sta.rpt riscv_fpga_timing.rpt}
} {
    lassign $pair src_name dst_name
    set src_path [file join $output_dir $src_name]
    if {[file exists $src_path]} {
        file copy -force $src_path [file join $build_dir $dst_name]
    }
}

foreach candidate [glob -nocomplain -directory $output_dir ap_core*.summary ap_core*.rpt] {
    file copy -force $candidate [file join $build_dir [file tail $candidate]]
}
