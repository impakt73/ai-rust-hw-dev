# RTL Bug Analysis: Byte Store (SB) Implementation

## Root Cause Identified

The duplicate bytes issue is caused by **missing byte-enable logic in the RTL**'s store byte (SB) implementation.

### The Problem

**Current RTL behavior (top.sv lines 173-180):**
```systemverilog
3'b000: begin // SB - Store Byte
    case (alu_result[1:0])
        2'b00: formatted_store_data = {24'b0, rs2_data[7:0]};
        2'b01: formatted_store_data = {16'b0, rs2_data[7:0], 8'b0};
        2'b10: formatted_store_data = {8'b0, rs2_data[7:0], 16'b0};
        2'b11: formatted_store_data = {rs2_data[7:0], 24'b0};
    endcase
end
```

**What happens:**
1. CPU executes: `sb  a1,0(a0)`  where a1=0x34, a0=0x80001
2. RTL computes address offset: alu_result[1:0] = 0x80001 & 0x3 = 2'b01
3. RTL formats data: {16'b0, 0x34, 8'b0} = 0x00003400
4. RTL sets dmem_wdata = 0x00003400, dmem_we = 1
5. Simulator calls write_word(0x80001, 0x00003400)
6. write_word writes 4 bytes at aligned address 0x80000:
   - [0x80000] = 0x00
   - [0x80001] = 0x34  ✓ correct
   - [0x80002] = 0x00  ✗ overwrites adjacent byte!
   - [0x80003] = 0x00  ✗ overwrites adjacent byte!

### Disassembly Evidence

**Stack test (WORKS):**
```assembly
li   a0,18          # Load immediate value
sw   a0,0(a1)       # Store WORD to FIFO (no byte operations)
```

**Heap test (FAILS):**
```assembly
sb   a1,0(a0)       # Store BYTE to heap
sb   a2,1(a0)       # Store BYTE to heap
...
lbu  a1,0(a0)       # Load BYTE unsigned from heap
sw   a1,0(s3)       # Store word to FIFO
```

Stack test uses only word operations → works perfectly
Heap test uses byte operations → data corruption

### Test Results

**Stack memory:** ✓ All bytes correct
```
Wrote: [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0]
Read:  [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0]
```

**Heap memory:** ✗ Adjacent bytes overwritten with zeros
```
Wrote: [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0]
Read:  [0x12, 0x00, 0x9A, 0x00, 0x9A, 0x00, 0x00, 0xF0]
```

## Solution

The RTL needs to implement proper byte-enable signaling for byte and halfword stores.

### Option 1: Add byte-enable output from CPU
```systemverilog
output logic [3:0]  dmem_be,  // Byte enable (one bit per byte)
```

Then set dmem_be based on funct3 and address alignment.

### Option 2: Implement read-modify-write in simulator
The simulator's write_word function should check funct3 and only write enabled bytes.

### Option 3: Implement read-modify-write in RTL
CPU reads word, modifies byte/halfword, writes word back.

Option 1 is the cleanest and most hardware-realistic solution.
