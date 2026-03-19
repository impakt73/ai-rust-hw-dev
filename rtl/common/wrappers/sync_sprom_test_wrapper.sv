`default_nettype none
module sync_sprom_test_wrapper (
    input  wire logic       clk,
    input  wire logic [2:0] addr,
    output      logic [31:0] rdata
);

    sync_sprom #(
        .DATA_WIDTH(32),
        .ADDR_WIDTH(3),
        .INIT_FILE("rtl/common/wrappers/sync_sprom_test_init.hex")
    ) u_sync_sprom (
        .clk(clk),
        .addr(addr),
        .rdata(rdata)
    );

endmodule
`default_nettype wire
