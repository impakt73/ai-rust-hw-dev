// CSR (Control and Status Register) File Module
// Implements RISC-V CSR register file and operations
// Includes support for F extension FCSR/FRM/FFLAGS
//
// OPTIMIZED: Uses sparse register implementation instead of 4096-entry array
// to reduce FPGA resource usage. Only commonly used CSRs are implemented.

module csr_file (
    input  logic        clk,
    input  logic        rst_n,
    
    // Control signals
    input  logic        is_csr,
    input  logic [2:0]  funct3,
    input  logic [4:0]  rs1,
    
    // Data signals
    input  logic [11:0] csr_addr,
    input  logic [31:0] rs1_data,
    input  logic [31:0] fcsr,        // F extension: FCSR value from top module
    
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
    localparam CSR_MISA      = 12'h301;  // Machine ISA (writable for test compatibility)
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
    // Sparse CSR Register Storage
    // Only implement the CSRs we actually use
    // ============================================================
    
    // Machine-level writable CSRs
    logic [31:0] csr_mstatus;
    logic [31:0] csr_misa;      // Made writable for test compatibility
    logic [31:0] csr_medeleg;   // Exception delegation
    logic [31:0] csr_mideleg;   // Interrupt delegation
    logic [31:0] csr_mie;
    logic [31:0] csr_mtvec;
    logic [31:0] csr_mscratch;
    logic [31:0] csr_mepc;
    logic [31:0] csr_mcause;
    logic [31:0] csr_mtval;
    
    // Performance counters (64-bit)
    logic [63:0] csr_cycle;
    logic [63:0] csr_instret;
    
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
            CSR_MSTATUS:   csr_rdata = csr_mstatus;
            CSR_MISA:      csr_rdata = csr_misa;  // Writable for test compatibility
            CSR_MEDELEG:   csr_rdata = csr_medeleg;
            CSR_MIDELEG:   csr_rdata = csr_mideleg;
            CSR_MIE:       csr_rdata = csr_mie;
            CSR_MTVEC:     csr_rdata = csr_mtvec;
            CSR_MSCRATCH:  csr_rdata = csr_mscratch;
            CSR_MEPC:      csr_rdata = csr_mepc;
            CSR_MCAUSE:    csr_rdata = csr_mcause;
            CSR_MTVAL:     csr_rdata = csr_mtval;
            CSR_MIP:       csr_rdata = 32'h0;  // No interrupts pending (simplified)
            
            // Performance counters (lower 32 bits)
            CSR_CYCLE:     csr_rdata = csr_cycle[31:0];
            CSR_TIME:      csr_rdata = csr_cycle[31:0];  // TIME == CYCLE in simple impl
            CSR_INSTRET:   csr_rdata = csr_instret[31:0];
            
            // Performance counters (upper 32 bits)
            CSR_CYCLEH:    csr_rdata = csr_cycle[63:32];
            CSR_TIMEH:     csr_rdata = csr_cycle[63:32];
            CSR_INSTRETH:  csr_rdata = csr_instret[63:32];
            
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
    
    // CSR register updates
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            csr_mstatus  <= 32'h0;
            csr_misa     <= 32'h40141101;  // RV32IMACF default
            csr_medeleg  <= 32'h0;
            csr_mideleg  <= 32'h0;
            csr_mie      <= 32'h0;
            csr_mtvec    <= 32'h0;
            csr_mscratch <= 32'h0;
            csr_mepc     <= 32'h0;
            csr_mcause   <= 32'h0;
            csr_mtval    <= 32'h0;
            csr_cycle    <= 64'h0;
            csr_instret  <= 64'h0;
        end else begin
            // Cycle counter always increments
            csr_cycle <= csr_cycle + 64'd1;
            
            // CSR writes
            if (is_csr && csr_write_en) begin
                case (csr_addr)
                    // Note: FCSR/FRM/FFLAGS are handled in top.sv, not here
                    CSR_MSTATUS:   csr_mstatus  <= csr_wdata;
                    CSR_MISA:      csr_misa     <= csr_wdata;  // Writable for test compatibility
                    CSR_MEDELEG:   csr_medeleg  <= csr_wdata;
                    CSR_MIDELEG:   csr_mideleg  <= csr_wdata;
                    CSR_MIE:       csr_mie      <= csr_wdata;
                    CSR_MTVEC:     csr_mtvec    <= csr_wdata;
                    CSR_MSCRATCH:  csr_mscratch <= csr_wdata;
                    CSR_MEPC:      csr_mepc     <= csr_wdata;
                    CSR_MCAUSE:    csr_mcause   <= csr_wdata;
                    CSR_MTVAL:     csr_mtval    <= csr_wdata;
                    // Read-only CSRs: CYCLE, TIME, INSTRET, MVENDORID, etc.
                    // Writes to these are silently ignored
                    default: ; // Ignore writes to unimplemented/read-only CSRs
                endcase
            end
        end
    end

endmodule
