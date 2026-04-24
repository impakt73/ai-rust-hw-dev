`default_nettype none
// CSR (Control and Status Register) File Module
// Implements RISC-V CSR register file and trap-critical machine state.

module csr_file #(
    parameter bit ENABLE_M_EXT = 1'b1,
    parameter bit ENABLE_F_EXT = 1'b1
) (
    input wire logic        clk,
    input wire logic        rst,
    input wire logic        is_csr,
    input wire logic        instr_complete,
    input wire logic [2:0]  funct3,
    input wire logic [4:0]  rs1,
    input wire logic [11:0] csr_addr,
    input wire logic [31:0] rs1_data,
    input wire logic        trap_entry,
    input wire logic [31:0] trap_mepc_in,
    input wire logic [31:0] trap_mcause_in,
    input wire logic [31:0] trap_mtval_in,
    input wire logic        trap_return,
    input wire logic        msip,
    input wire logic        mtip,
    input wire logic        meip,
    input wire logic [4:0]  fp_fflags_in,
    input wire logic        fp_fflags_we,
    output logic [31:0] csr_rdata,
    output logic [31:0] csr_mtvec_out,
    output logic [31:0] csr_mepc_out,
    output logic [31:0] csr_mstatus_out,
    output logic [31:0] csr_mie_out,
    output logic [31:0] csr_mip_out,
    output logic [31:0] csr_fcsr_out
);

    localparam logic [11:0] CSR_FFLAGS    = 12'h001;
    localparam logic [11:0] CSR_FRM       = 12'h002;
    localparam logic [11:0] CSR_FCSR      = 12'h003;
    localparam logic [11:0] CSR_CYCLE     = 12'hC00;
    localparam logic [11:0] CSR_TIME      = 12'hC01;
    localparam logic [11:0] CSR_INSTRET   = 12'hC02;
    localparam logic [11:0] CSR_CYCLEH    = 12'hC80;
    localparam logic [11:0] CSR_TIMEH     = 12'hC81;
    localparam logic [11:0] CSR_INSTRETH  = 12'hC82;

    localparam logic [11:0] CSR_MSTATUS   = 12'h300;
    localparam logic [11:0] CSR_MISA      = 12'h301;
    localparam logic [11:0] CSR_MEDELEG   = 12'h302;
    localparam logic [11:0] CSR_MIDELEG   = 12'h303;
    localparam logic [11:0] CSR_MIE       = 12'h304;
    localparam logic [11:0] CSR_MTVEC     = 12'h305;
    localparam logic [11:0] CSR_MSCRATCH  = 12'h340;
    localparam logic [11:0] CSR_MEPC      = 12'h341;
    localparam logic [11:0] CSR_MCAUSE    = 12'h342;
    localparam logic [11:0] CSR_MTVAL     = 12'h343;
    localparam logic [11:0] CSR_MIP       = 12'h344;

    localparam logic [11:0] CSR_MVENDORID = 12'hF11;
    localparam logic [11:0] CSR_MARCHID   = 12'hF12;
    localparam logic [11:0] CSR_MIMPID    = 12'hF13;
    localparam logic [11:0] CSR_MHARTID   = 12'hF14;

    localparam int MSTATUS_MIE_BIT  = 3;
    localparam int MSTATUS_MPIE_BIT = 7;
    localparam int MIP_MSIP_BIT = 3;
    localparam int MIP_MTIP_BIT = 7;
    localparam int MIP_MEIP_BIT = 11;
    localparam logic [31:0] MACHINE_INTERRUPT_MASK =
        (32'h1 << MIP_MSIP_BIT) |
        (32'h1 << MIP_MTIP_BIT) |
        (32'h1 << MIP_MEIP_BIT);

    localparam logic [31:0] CSR_MISA_CONST = 32'h4000_0105 |
                                             (ENABLE_M_EXT ? 32'h0000_1000 : 32'h0) |
                                             (ENABLE_F_EXT ? 32'h0000_0020 : 32'h0);

    logic [31:0] csr_cycle;
    logic [31:0] csr_instret;
    logic [31:0] csr_mip_next;
    logic [31:0] csr_rdata_next;

    logic [31:0] csr_mstatus_reg;
    logic [31:0] csr_medeleg_reg;
    logic [31:0] csr_mideleg_reg;
    logic [31:0] csr_mie_reg;
    logic [31:0] csr_mtvec_reg;
    logic [31:0] csr_mscratch_reg;
    logic [31:0] csr_mepc_reg;
    logic [31:0] csr_mcause_reg;
    logic [31:0] csr_mtval_reg;
    logic [31:0] csr_fcsr_reg;

    logic [31:0] csr_wdata;
    logic        csr_write_en;

    function automatic logic [31:0] sanitize_mtvec(input logic [31:0] value);
        sanitize_mtvec = {value[31:2], 2'b00};
    endfunction

    function automatic logic [31:0] sanitize_mepc(input logic [31:0] value);
        sanitize_mepc = value & ~32'h1;
    endfunction

    function automatic logic [31:0] sanitize_mie(input logic [31:0] value);
        sanitize_mie = value & MACHINE_INTERRUPT_MASK;
    endfunction

    function automatic logic is_software_writable(input logic [11:0] addr);
        case (addr)
            CSR_FFLAGS, CSR_FRM, CSR_FCSR,
            CSR_MSTATUS, CSR_MEDELEG, CSR_MIDELEG, CSR_MIE,
            CSR_MTVEC, CSR_MSCRATCH, CSR_MEPC, CSR_MCAUSE, CSR_MTVAL:
                is_software_writable = 1'b1;
            default:
                is_software_writable = 1'b0;
        endcase
    endfunction

    // Trap-critical architectural CSR outputs stay direct on their dedicated ports,
    // but software-initiated CSR reads return through a registered csr_rdata path.
    assign csr_mip_next = {
        20'h0,
        meip,
        3'b000,
        mtip,
        3'b000,
        msip,
        3'b000
    };

    always_comb begin
        case (csr_addr)
            CSR_FFLAGS:    csr_rdata_next = ENABLE_F_EXT ? {27'h0, csr_fcsr_reg[4:0]} : 32'h0;
            CSR_FRM:       csr_rdata_next = ENABLE_F_EXT ? {29'h0, csr_fcsr_reg[7:5]} : 32'h0;
            CSR_FCSR:      csr_rdata_next = ENABLE_F_EXT ? {24'h0, csr_fcsr_reg[7:0]} : 32'h0;
            CSR_MSTATUS:   csr_rdata_next = csr_mstatus_reg;
            CSR_MISA:      csr_rdata_next = CSR_MISA_CONST;
            CSR_MEDELEG:   csr_rdata_next = csr_medeleg_reg;
            CSR_MIDELEG:   csr_rdata_next = csr_mideleg_reg;
            CSR_MIE:       csr_rdata_next = csr_mie_reg;
            CSR_MTVEC:     csr_rdata_next = csr_mtvec_reg;
            CSR_MSCRATCH:  csr_rdata_next = csr_mscratch_reg;
            CSR_MEPC:      csr_rdata_next = csr_mepc_reg;
            CSR_MCAUSE:    csr_rdata_next = csr_mcause_reg;
            CSR_MTVAL:     csr_rdata_next = csr_mtval_reg;
            CSR_MIP:       csr_rdata_next = csr_mip_next;
            CSR_CYCLE:     csr_rdata_next = csr_cycle;
            CSR_TIME:      csr_rdata_next = csr_cycle;
            CSR_INSTRET:   csr_rdata_next = csr_instret;
            CSR_CYCLEH:    csr_rdata_next = 32'h0;
            CSR_TIMEH:     csr_rdata_next = 32'h0;
            CSR_INSTRETH:  csr_rdata_next = 32'h0;
            CSR_MVENDORID: csr_rdata_next = 32'h0;
            CSR_MARCHID:   csr_rdata_next = 32'h0;
            CSR_MIMPID:    csr_rdata_next = 32'h0001_0000;
            CSR_MHARTID:   csr_rdata_next = 32'h0;
            default:       csr_rdata_next = 32'h0;
        endcase
    end

    always_comb begin
        case (funct3)
            3'b001: csr_wdata = rs1_data;
            3'b010: csr_wdata = csr_rdata | rs1_data;
            3'b011: csr_wdata = csr_rdata & ~rs1_data;
            3'b101: csr_wdata = {27'b0, rs1};
            3'b110: csr_wdata = csr_rdata | {27'b0, rs1};
            3'b111: csr_wdata = csr_rdata & ~{27'b0, rs1};
            default: csr_wdata = 32'h0;
        endcase
    end

    always_comb begin
        case (funct3)
            3'b001, 3'b101: csr_write_en = 1'b1;
            3'b010, 3'b011, 3'b110, 3'b111: csr_write_en = (rs1 != 5'b0);
            default: csr_write_en = 1'b0;
        endcase
    end

    always_ff @(posedge clk) begin
        if (rst) begin
            csr_rdata        <= 32'h0;
            csr_cycle        <= 32'h0;
            csr_instret      <= 32'h0;
            csr_mstatus_reg  <= 32'h0;
            csr_medeleg_reg  <= 32'h0;
            csr_mideleg_reg  <= 32'h0;
            csr_mie_reg      <= 32'h0;
            csr_mtvec_reg    <= 32'h0;
            csr_mscratch_reg <= 32'h0;
            csr_mepc_reg     <= 32'h0;
            csr_mcause_reg   <= 32'h0;
            csr_mtval_reg    <= 32'h0;
            csr_fcsr_reg     <= 32'h0;
        end else begin
            csr_rdata <= csr_rdata_next;
            csr_cycle <= csr_cycle + 32'd1;
            if (instr_complete)
                csr_instret <= csr_instret + 32'd1;

            if (trap_entry) begin
                csr_mepc_reg <= sanitize_mepc(trap_mepc_in);
                csr_mcause_reg <= trap_mcause_in;
                csr_mtval_reg <= trap_mtval_in;
                csr_mstatus_reg[MSTATUS_MPIE_BIT] <= csr_mstatus_reg[MSTATUS_MIE_BIT];
                csr_mstatus_reg[MSTATUS_MIE_BIT] <= 1'b0;
            end else if (trap_return) begin
                csr_mstatus_reg[MSTATUS_MIE_BIT] <= csr_mstatus_reg[MSTATUS_MPIE_BIT];
                csr_mstatus_reg[MSTATUS_MPIE_BIT] <= 1'b1;
            end else if (fp_fflags_we && ENABLE_F_EXT) begin
                csr_fcsr_reg[4:0] <= csr_fcsr_reg[4:0] | fp_fflags_in;
            end else if (is_csr && csr_write_en && is_software_writable(csr_addr)) begin
                case (csr_addr)
                    CSR_FFLAGS:   if (ENABLE_F_EXT) csr_fcsr_reg[4:0] <= csr_wdata[4:0];
                    CSR_FRM:      if (ENABLE_F_EXT) csr_fcsr_reg[7:5] <= csr_wdata[2:0];
                    CSR_FCSR:     if (ENABLE_F_EXT) csr_fcsr_reg <= {24'h0, csr_wdata[7:0]};
                    CSR_MSTATUS:  csr_mstatus_reg <= csr_wdata;
                    CSR_MEDELEG:  csr_medeleg_reg <= csr_wdata;
                    CSR_MIDELEG:  csr_mideleg_reg <= csr_wdata;
                    CSR_MIE:      csr_mie_reg <= sanitize_mie(csr_wdata);
                    CSR_MTVEC:    csr_mtvec_reg <= sanitize_mtvec(csr_wdata);
                    CSR_MSCRATCH: csr_mscratch_reg <= csr_wdata;
                    CSR_MEPC:     csr_mepc_reg <= sanitize_mepc(csr_wdata);
                    CSR_MCAUSE:   csr_mcause_reg <= csr_wdata;
                    CSR_MTVAL:    csr_mtval_reg <= csr_wdata;
                    default: ;
                endcase
            end
        end
    end

    assign csr_mtvec_out = csr_mtvec_reg;
    assign csr_mepc_out = csr_mepc_reg;
    assign csr_mstatus_out = csr_mstatus_reg;
    assign csr_mie_out = csr_mie_reg;
    assign csr_mip_out = csr_mip_next;
    assign csr_fcsr_out = ENABLE_F_EXT ? {24'h0, csr_fcsr_reg[7:0]} : 32'h0;

endmodule
`default_nettype wire
