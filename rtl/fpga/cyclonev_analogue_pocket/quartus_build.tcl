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
set qpf_file [file rootname $qsf_file]
append qpf_file ".qpf"
set project_name [file rootname [file tail $qsf_file]]
set project_copy_dir [file join $build_dir project]
set project_qsf [file join $project_copy_dir [file tail $qsf_file]]
set project_qpf [file join $project_copy_dir [file tail $qpf_file]]
set output_dir [file join $build_dir output_files]

if {![file exists $qsf_file]} {
    puts stderr "Quartus project file not found: $qsf_file"
    exit 1
}

file mkdir $build_dir
file delete -force $project_copy_dir
file copy -force $qsf_dir $project_copy_dir
file mkdir $output_dir

project_open $project_qpf -revision $project_name
set_global_assignment -name TOP_LEVEL_ENTITY $top_module
set_global_assignment -name PROJECT_OUTPUT_DIRECTORY $output_dir

foreach rtl_source $rtl_sources {
    set normalized_source [file normalize $rtl_source]
    if {[string first $qsf_dir $normalized_source] == 0} {
        if {$normalized_source ne [file normalize [file join $qsf_dir core analogue_pocket_repo_top.sv]]} {
            continue
        }
    }
    set extension [string tolower [file extension $normalized_source]]
    switch -- $extension {
        ".sv" {
            set_global_assignment -name SYSTEMVERILOG_FILE $normalized_source
        }
        default {
            set_global_assignment -name VERILOG_FILE $normalized_source
        }
    }
}

export_assignments
execute_flow -compile
project_close

foreach pair {
    {ap_core.rbf riscv_fpga.rbf}
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
