if {[llength $argv] < 8} {
    puts stderr "Usage: gowin_build.tcl <build_dir> <project_name> <top_module> <fpga_part> <device_version> <constraint_file> <timing_file> <source_files...>"
    exit 1
}

set build_dir [file normalize [lindex $argv 0]]
set project_name [lindex $argv 1]
set top_module [lindex $argv 2]
set fpga_part [lindex $argv 3]
set device_version [lindex $argv 4]
set constraint_file [file normalize [lindex $argv 5]]
set timing_file [file normalize [lindex $argv 6]]
set rtl_sources [lrange $argv 7 end]
set project_dir [file join $build_dir project]

proc copy_first_match {patterns destination} {
    foreach pattern $patterns {
        foreach candidate [lsort -dictionary [glob -nocomplain $pattern]] {
            if {[file isfile $candidate]} {
                file copy -force $candidate $destination
                return 1
            }
        }
    }
    return 0
}

file mkdir $build_dir
file delete -force $project_dir
file mkdir $project_dir

create_project -name $project_name -dir $project_dir -pn $fpga_part -device_version $device_version
cd $project_dir

foreach rtl_source $rtl_sources {
    set normalized_source [file normalize $rtl_source]
    set extension [string tolower [file extension $normalized_source]]
    switch -- $extension {
        ".sv" {
            add_file -type systemverilog $normalized_source
        }
        ".v" {
            add_file -type verilog $normalized_source
        }
        default {
            puts stderr "Unsupported RTL source extension for $normalized_source"
            exit 1
        }
    }
}

add_file -type cst $constraint_file
add_file -type sdc $timing_file

set_option -top_module $top_module
set_option -output_base_name $project_name

run all

set normalized_bitstream [file join $build_dir "riscv_fpga.fs"]
if {![copy_first_match [list \
        [file join $project_dir "*.fs"] \
        [file join $project_dir "impl" "*.fs"] \
        [file join $project_dir "impl" "pnr" "*.fs"] \
    ] $normalized_bitstream]} {
    puts stderr "Gowin build completed without producing a .fs bitstream under $project_dir"
    exit 1
}

copy_first_match [list \
    [file join $project_dir "*timing*.rpt"] \
    [file join $project_dir "*timing*.txt"] \
    [file join $project_dir "impl" "*timing*.rpt"] \
    [file join $project_dir "impl" "*timing*.txt"] \
    [file join $project_dir "impl" "pnr" "*timing*.rpt"] \
    [file join $project_dir "impl" "pnr" "*timing*.txt"] \
] [file join $build_dir "riscv_fpga_timing.rpt"]

copy_first_match [list \
    [file join $project_dir "*summary*.rpt"] \
    [file join $project_dir "*summary*.txt"] \
    [file join $project_dir "impl" "*summary*.rpt"] \
    [file join $project_dir "impl" "*summary*.txt"] \
    [file join $project_dir "impl" "pnr" "*summary*.rpt"] \
    [file join $project_dir "impl" "pnr" "*summary*.txt"] \
] [file join $build_dir "riscv_fpga_timing_summary.rpt"]

copy_first_match [list \
    [file join $project_dir "*util*.rpt"] \
    [file join $project_dir "*util*.txt"] \
    [file join $project_dir "*resource*.rpt"] \
    [file join $project_dir "*resource*.txt"] \
    [file join $project_dir "impl" "*util*.rpt"] \
    [file join $project_dir "impl" "*util*.txt"] \
    [file join $project_dir "impl" "*resource*.rpt"] \
    [file join $project_dir "impl" "*resource*.txt"] \
    [file join $project_dir "impl" "pnr" "*util*.rpt"] \
    [file join $project_dir "impl" "pnr" "*util*.txt"] \
    [file join $project_dir "impl" "pnr" "*resource*.rpt"] \
    [file join $project_dir "impl" "pnr" "*resource*.txt"] \
] [file join $build_dir "riscv_fpga_utilization.rpt"]
