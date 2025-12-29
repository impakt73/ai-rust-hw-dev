.section .text
.global _start

_start:
    # Initialize base registers
    addi x1, x0, 10      # x1 = 10
    addi x2, x0, 20      # x2 = 20
    
    # ====== Test 1: Arithmetic ALU Operations ======
    add x3, x1, x2       # x3 = 10 + 20 = 30
    sub x4, x2, x1       # x4 = 20 - 10 = 10
    addi x5, x1, 5       # x5 = 10 + 5 = 15
    
    # ====== Test 2: Logical ALU Operations ======
    and x6, x1, x2       # x6 = 10 & 20 = 0
    or x7, x1, x2        # x7 = 10 | 20 = 30
    xor x8, x1, x2       # x8 = 10 ^ 20 = 30
    andi x9, x1, 15      # x9 = 10 & 15 = 10
    ori x10, x1, 5       # x10 = 10 | 5 = 15
    xori x11, x1, 7      # x11 = 10 ^ 7 = 13
    
    # ====== Test 3: Shift Operations ======
    addi x12, x0, 8      # x12 = 8
    slli x13, x12, 2     # x13 = 8 << 2 = 32
    srli x14, x13, 1     # x14 = 32 >> 1 = 16
    addi x15, x0, -8     # x15 = -8 (0xFFFFFFF8)
    srai x16, x15, 1     # x16 = -8 >>> 1 = -4 (0xFFFFFFFC)
    
    # ====== Test 4: Comparison Operations ======
    addi x17, x0, 5      # x17 = 5
    addi x18, x0, 10     # x18 = 10
    slt x19, x17, x18    # x19 = 1 (5 < 10)
    slti x20, x17, 3     # x20 = 0 (5 < 3 is false)
    sltu x21, x17, x18   # x21 = 1 (5 < 10 unsigned)
    
    # ====== Test 5: Conditional Branches (BEQ, BNE) ======
    addi x22, x0, 42     # x22 = 42
    addi x23, x0, 42     # x23 = 42
    beq x22, x23, beq_pass  # Should branch (42 == 42)
    addi x24, x0, 99     # x24 = 99 (should be skipped)
beq_pass:
    addi x24, x0, 1      # x24 = 1 (branched correctly)
    
    addi x25, x0, 10     # x25 = 10
    addi x26, x0, 20     # x26 = 20
    bne x25, x26, bne_pass  # Should branch (10 != 20)
    addi x27, x0, 99     # x27 = 99 (should be skipped)
bne_pass:
    addi x27, x0, 1      # x27 = 1 (branched correctly)
    
    # ====== Test 6: Conditional Branches (BLT, BGE) ======
    addi x28, x0, 5      # x28 = 5
    addi x29, x0, 10     # x29 = 10
    blt x28, x29, blt_pass  # Should branch (5 < 10)
    addi x30, x0, 99     # x30 = 99 (should be skipped)
blt_pass:
    addi x30, x0, 1      # x30 = 1 (branched correctly)
    
    addi x31, x0, 15     # x31 = 15
    bge x31, x29, bge_pass  # Should branch (15 >= 10)
    addi x28, x0, 99     # Should be skipped
bge_pass:
    addi x28, x0, 2      # x28 = 2 (branched correctly)
    
    # ====== Test 7: Memory Store and Load Verification ======
    # Set up memory test area at 0x80001000
    lui x1, 0x80001      # x1 = 0x80001000
    
    # Store test values
    addi x2, x0, 100     # x2 = 100
    sw x2, 0(x1)         # mem[0x80001000] = 100
    
    addi x3, x0, 200     # x3 = 200
    sw x3, 4(x1)         # mem[0x80001004] = 200
    
    addi x4, x0, 300     # x4 = 300
    sw x4, 8(x1)         # mem[0x80001008] = 300
    
    # Load and verify
    lw x5, 0(x1)         # x5 = mem[0x80001000] should be 100
    lw x6, 4(x1)         # x6 = mem[0x80001004] should be 200
    lw x7, 8(x1)         # x7 = mem[0x80001008] should be 300
    
    # Verify loaded values match stored values
    bne x2, x5, test_fail   # Check if 100 == 100
    bne x3, x6, test_fail   # Check if 200 == 200
    bne x4, x7, test_fail   # Check if 300 == 300
    
    # ====== Test 8: Loop with Constant Counter ======
    addi x8, x0, 0       # x8 = 0 (accumulator)
    addi x9, x0, 5       # x9 = 5 (loop counter)
const_loop:
    addi x8, x8, 1       # x8++
    addi x9, x9, -1      # x9--
    bne x9, x0, const_loop  # Continue if x9 != 0
    # After loop: x8 should be 5
    
    # ====== Test 9: Loop with Variable Iterations from Memory ======
    # Store loop count in memory
    lui x10, 0x80001     # x10 = 0x80001000
    addi x11, x0, 7      # x11 = 7 (loop count)
    sw x11, 12(x10)      # mem[0x8000100C] = 7
    
    # Load loop count and iterate
    lw x12, 12(x10)      # x12 = 7 (from memory)
    addi x13, x0, 0      # x13 = 0 (accumulator)
var_loop:
    addi x13, x13, 2     # x13 += 2
    addi x12, x12, -1    # x12--
    bne x12, x0, var_loop   # Continue if x12 != 0
    # After loop: x13 should be 14 (7 * 2)
    
    # ====== Test 10: Nested Arithmetic Sequence ======
    addi x14, x0, 3      # x14 = 3
    addi x15, x0, 4      # x15 = 4
    addi x16, x0, 5      # x16 = 5
    add x17, x14, x15    # x17 = 3 + 4 = 7
    add x18, x17, x16    # x18 = 7 + 5 = 12
    slli x19, x18, 1     # x19 = 12 << 1 = 24
    srli x20, x19, 2     # x20 = 24 >> 2 = 6
    
    # ====== Test 11: Upper Immediate Operations ======
    lui x21, 0x12345     # x21 = 0x12345000
    addi x21, x21, 0x678 # x21 = 0x12345678
    auipc x22, 0         # x22 = PC + 0
    
    # ====== All Tests Passed - Store success to tohost ======
    lui x31, 0x0         # Load upper immediate (0)
    addi x31, x31, -16   # x31 = 0xFFFFFFF0 (tohost address)
    addi x30, x0, 42     # x30 = 42 (success code)
    sw x30, 0(x31)       # Store result to tohost
    j halt
    
test_fail:
    # Store failure code to tohost
    lui x31, 0x0         # Load upper immediate (0)
    addi x31, x31, -16   # x31 = 0xFFFFFFF0 (tohost address)
    addi x30, x0, 1      # x30 = 1 (failure code)
    sw x30, 0(x31)       # Store result to tohost
    
halt:
    j halt               # Infinite loop
