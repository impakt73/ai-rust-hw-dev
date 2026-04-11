set_global_assignment -entity "mf_pllbase2" -library "lib_mf_pllbase2" -name IP_TOOL_NAME "altera_pll"
set_global_assignment -entity "mf_pllbase2" -library "lib_mf_pllbase2" -name IP_TOOL_VERSION "25.1"
set_global_assignment -entity "mf_pllbase2" -library "lib_mf_pllbase2" -name IP_TOOL_ENV "mwpim"
set_global_assignment -library "lib_mf_pllbase2" -name SPD_FILE [file join $::quartus(sip_path) "mf_pllbase2.spd"]

set_global_assignment -library "lib_mf_pllbase2" -name MISC_FILE [file join $::quartus(sip_path) "mf_pllbase2_sim/mf_pllbase2.vo"]
