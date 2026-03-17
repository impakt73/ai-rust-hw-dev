proc generateBuildID_MIF {} {
    set buildDate [clock format [clock seconds] -format %Y%m%d]
    set buildTime [clock format [clock seconds] -format %H%M%S]
    set buildUnique [expr {int(rand()*(4294967295))}]

    set outputFileName "apf/build_id.mif"
    set outputFile [open $outputFileName "w"]

    puts $outputFile "-- Build ID Memory Initialization File"
    puts $outputFile ""
    puts $outputFile "DEPTH = 256;"
    puts $outputFile "WIDTH = 32;"
    puts $outputFile "ADDRESS_RADIX = HEX;"
    puts $outputFile "DATA_RADIX = HEX;"
    puts $outputFile ""
    puts $outputFile "CONTENT"
    puts $outputFile "BEGIN"
    puts $outputFile ""
    puts $outputFile "   0E0 : $buildDate;"
    puts $outputFile "   0E1 : $buildTime;"
    puts $outputFile [format "   0E2 : %08x;" $buildUnique]
    puts $outputFile ""
    puts $outputFile "END;"
    close $outputFile
}

generateBuildID_MIF
