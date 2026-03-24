`default_nettype none
// CSR (Control and Status Register) File Module
// Implements RISC-V CSR register file and operations
// Includes support for F extension FCSR/FRM/FFLAGS
//
// OPTIMIZED: Uses BRAM-backed sparse CSR storage (via sync_dpram) instead of
// discrete flip-flop registers for writable machine-level CSRs.

module csr_file #(
    parameter bit ENABLE_M_EXT = 1'b1,
    parameter bit ENABLE_F_EXT = 1'b1
) (
    input wire logic        clk,
    input wire logic        rst,
    
    // Control signals
    input wire logic        is_csr,
    input wire logic        instr_complete,  // Signal when instruction completes
    input wire logic [2:0]  funct3,
    input wire logic [4:0]  rs1,
    
    // Data signals
    input wire logic [11:0] csr_addr,
    input wire logic [31:0] rs1_data,
    input wire logic [31:0] fcsr,        // F extension: FCSR value from top module
    
    // Output
    output logic [31:0] csr_rdata
);

    // ============================================================
    // CSR Address Definitions (RISC-V Privileged Spec)
    // ============================================================
    
    // User-level CSRs (0x000-0x0FF)
    localparam CSR_FFLAGS    = 12'h001;  // FP exception flags
    localparam CSR_FRM       = 12'h002;  // FP rounding mode
    localparam CSR_FCSR      = 12'h003;  // FP control/status
    localparam CSR_CYCLE     = 12'hC00;  // Cycle counter (read-only)
    localparam CSR_TIME      = 12'hC01;  // Timer (read-only)
    localparam CSR_INSTRET   = 12'hC02;  // Instructions retired (read-only)
    localparam CSR_CYCLEH    = 12'hC80;  // Upper 32 bits of cycle
    localparam CSR_TIMEH     = 12'hC81;  // Upper 32 bits of time
    localparam CSR_INSTRETH  = 12'hC82;  // Upper 32 bits of instret
    
    // Machine-level CSRs (0x300-0x3FF)
    localparam CSR_MSTATUS   = 12'h300;  // Machine status
    localparam CSR_MISA      = 12'h301;  // Machine ISA (read-only, derived from enabled extensions)
    localparam CSR_MEDELEG   = 12'h302;  // Machine exception delegation
    localparam CSR_MIDELEG   = 12'h303;  // Machine interrupt delegation
    localparam CSR_MIE       = 12'h304;  // Machine interrupt enable
    localparam CSR_MTVEC     = 12'h305;  // Machine trap vector
    localparam CSR_MSCRATCH  = 12'h340;  // Machine scratch
    localparam CSR_MEPC      = 12'h341;  // Machine exception PC
    localparam CSR_MCAUSE    = 12'h342;  // Machine trap cause
    localparam CSR_MTVAL     = 12'h343;  // Machine trap value
    localparam CSR_MIP       = 12'h344;  // Machine interrupt pending
    
    // Machine information CSRs (0xF11-0xF14, read-only)
    localparam CSR_MVENDORID = 12'hF11;  // Vendor ID
    localparam CSR_MARCHID   = 12'hF12;  // Architecture ID
    localparam CSR_MIMPID    = 12'hF13;  // Implementation ID
    localparam CSR_MHARTID   = 12'hF14;  // Hardware thread ID
    
    // ============================================================
    // BRAM-backed CSR Storage (writable machine-level CSRs)
    // ============================================================
    localparam int CSR_MEM_ADDR_WIDTH = 8;  // 256 entries (matches sync_dpram BRAM-friendly config)

    localparam logic [CSR_MEM_ADDR_WIDTH-1:0] CSR_IDX_MSTATUS  = 8'd0;
    localparam logic [CSR_MEM_ADDR_WIDTH-1:0] CSR_IDX_MEDELEG  = 8'd1;
    localparam logic [CSR_MEM_ADDR_WIDTH-1:0] CSR_IDX_MIDELEG  = 8'd2;
    localparam logic [CSR_MEM_ADDR_WIDTH-1:0] CSR_IDX_MIE      = 8'd3;
    localparam logic [CSR_MEM_ADDR_WIDTH-1:0] CSR_IDX_MTVEC    = 8'd4;
    localparam logic [CSR_MEM_ADDR_WIDTH-1:0] CSR_IDX_MSCRATCH = 8'd5;
    localparam logic [CSR_MEM_ADDR_WIDTH-1:0] CSR_IDX_MEPC     = 8'd6;
    localparam logic [CSR_MEM_ADDR_WIDTH-1:0] CSR_IDX_MCAUSE   = 8'd7;
    localparam logic [CSR_MEM_ADDR_WIDTH-1:0] CSR_IDX_MTVAL    = 8'd8;
    localparam logic [31:0] CSR_MISA_CONST = 32'h4000_0105 |  // Base RV32IAC: MXL[31:30]=01, I[8], A[0], C[2]
                                             (ENABLE_M_EXT ? 32'h0000_1000 : 32'h0) |
                                             (ENABLE_F_EXT ? 32'h0000_0020 : 32'h0);

    logic [CSR_MEM_ADDR_WIDTH-1:0] csr_mem_addr;
    logic [CSR_MEM_ADDR_WIDTH-1:0] csr_mem_waddr;
    logic [31:0] csr_mem_rdata;
    logic [31:0] csr_mem_wdata;
    logic        csr_mem_we;

    function automatic logic [CSR_MEM_ADDR_WIDTH-1:0] csr_addr_to_index(input logic [11:0] addr);
        case (addr)
            CSR_MSTATUS:   csr_addr_to_index = CSR_IDX_MSTATUS;
            CSR_MEDELEG:   csr_addr_to_index = CSR_IDX_MEDELEG;
            CSR_MIDELEG:   csr_addr_to_index = CSR_IDX_MIDELEG;
            CSR_MIE:       csr_addr_to_index = CSR_IDX_MIE;
            CSR_MTVEC:     csr_addr_to_index = CSR_IDX_MTVEC;
            CSR_MSCRATCH:  csr_addr_to_index = CSR_IDX_MSCRATCH;
            CSR_MEPC:      csr_addr_to_index = CSR_IDX_MEPC;
            CSR_MCAUSE:    csr_addr_to_index = CSR_IDX_MCAUSE;
            CSR_MTVAL:     csr_addr_to_index = CSR_IDX_MTVAL;
            default:       csr_addr_to_index = {CSR_MEM_ADDR_WIDTH{1'b0}};
        endcase
    endfunction

    function automatic logic is_writable_csr_addr(input logic [11:0] addr);
        case (addr)
            CSR_MSTATUS, CSR_MEDELEG, CSR_MIDELEG, CSR_MIE,
            CSR_MTVEC, CSR_MSCRATCH, CSR_MEPC, CSR_MCAUSE, CSR_MTVAL:
                is_writable_csr_addr = 1'b1;
            default:
                is_writable_csr_addr = 1'b0;
        endcase
    endfunction

    assign csr_mem_addr = csr_addr_to_index(csr_addr);
    
    // Performance counters (32-bit simplification for this project)
    // Note: CYCLE increments every clock cycle, INSTRET increments on instr_complete.
    // High counter CSRs (CYCLEH/TIMEH/INSTRETH) intentionally read as 0.
    logic [31:0] csr_cycle;
    logic [31:0] csr_instret;
    
    // ============================================================
    // CSR Read Logic
    // ============================================================
    always_comb begin
        case (csr_addr)
            // F extension CSRs (handled externally via fcsr input)
            CSR_FFLAGS:    csr_rdata = {27'h0, fcsr[4:0]};
            CSR_FRM:       csr_rdata = {29'h0, fcsr[7:5]};
            CSR_FCSR:      csr_rdata = fcsr;
            
            // Machine-level CSRs
            CSR_MISA:      csr_rdata = CSR_MISA_CONST;
            CSR_MSTATUS, CSR_MEDELEG, CSR_MIDELEG, CSR_MIE,
            CSR_MTVEC, CSR_MSCRATCH, CSR_MEPC, CSR_MCAUSE, CSR_MTVAL:
                            csr_rdata = csr_mem_rdata;
            CSR_MIP:       csr_rdata = 32'h0;  // No interrupts pending (simplified)
            
            // Performance counters (lower 32 bits)
            CSR_CYCLE:     csr_rdata = csr_cycle[31:0];
            CSR_TIME:      csr_rdata = csr_cycle[31:0];  // TIME == CYCLE in simple impl
            CSR_INSTRET:   csr_rdata = csr_instret[31:0];
            
            // Performance counters (upper 32 bits)
            CSR_CYCLEH:    csr_rdata = 32'h0;
            CSR_TIMEH:     csr_rdata = 32'h0;
            CSR_INSTRETH:  csr_rdata = 32'h0;
            
            // Machine information (read-only, hardcoded)
            CSR_MVENDORID: csr_rdata = 32'h0;           // Non-commercial
            CSR_MARCHID:   csr_rdata = 32'h0;           // Not assigned
            CSR_MIMPID:    csr_rdata = 32'h00010000;    // Version 1.0
            CSR_MHARTID:   csr_rdata = 32'h0;           // Hart 0
            
            // Unimplemented CSRs return 0
            default:       csr_rdata = 32'h0;
        endcase
    end
    
    // ============================================================
    // CSR Write Logic
    // ============================================================
    
    // Compute new value based on CSR operation type
    logic [31:0] csr_wdata;
    
    always_comb begin
        case (funct3)
            3'b001: csr_wdata = rs1_data;                    // CSRRW
            3'b010: csr_wdata = csr_rdata | rs1_data;        // CSRRS
            3'b011: csr_wdata = csr_rdata & ~rs1_data;       // CSRRC
            3'b101: csr_wdata = {27'b0, rs1};                // CSRRWI
            3'b110: csr_wdata = csr_rdata | {27'b0, rs1};    // CSRRSI
            3'b111: csr_wdata = csr_rdata & ~{27'b0, rs1};   // CSRRCI
            default: csr_wdata = 32'h0;
        endcase
    end
    
    // Check if write should occur (rs1/zimm != 0 for set/clear operations)
    logic csr_write_en;
    always_comb begin
        case (funct3)
            3'b001, 3'b101: csr_write_en = 1'b1;             // CSRRW/CSRRWI always write
            3'b010, 3'b011, 3'b110, 3'b111: csr_write_en = (rs1 != 5'b0);  // Others only if rs1/zimm != 0
            default: csr_write_en = 1'b0;
        endcase
    end
    
    assign csr_mem_we = is_csr && csr_write_en && is_writable_csr_addr(csr_addr);
    assign csr_mem_waddr = csr_mem_addr;
    assign csr_mem_wdata = csr_wdata;

    sync_dpram #(
        .DATA_WIDTH(32),
        .ADDR_WIDTH(CSR_MEM_ADDR_WIDTH),
        .INIT_ZERO(1'b1)
    ) u_csr_mem (
        .wclk(clk),
        .rclk(clk),
        .we(csr_mem_we),
        .waddr(csr_mem_waddr),
        .wdata(csr_mem_wdata),
        .raddr(csr_mem_addr),
        .rdata(csr_mem_rdata)
    );

    // CSR updates
    always_ff @(posedge clk) begin
        if (rst) begin
            csr_cycle    <= 32'h0;
            csr_instret  <= 32'h0;
        end else begin
            // Cycle counter always increments
            csr_cycle <= csr_cycle + 32'd1;
            
            // Instruction retired counter increments when instruction completes
            if (instr_complete) begin
                csr_instret <= csr_instret + 32'd1;
            end
            
            // Writable machine-level CSR writes are handled by BRAM write port.
            // FCSR/FRM/FFLAGS are handled in cpu.sv.
        end
    end

endmodule
`default_nettype wire
