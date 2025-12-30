.section .text
.global _start

# Register Trace Audit Test Program
#
# This program is designed to verify that the instruction trace feature
# correctly reports source and destination register values.
#
# Strategy: Use simple ADD operations where the destination value can be
# trivially verified as the sum of the source registers. This makes it
# obvious if register values are incorrect in the trace output.
#
# Expected trace format: "add x{rd}=0x{rd_value}, x{rs1}=0x{rs1_value}, x{rs2}=0x{rs2_value}"

_start:
    # ====== Phase 1: Sequential Additions - Build up registers from 0 ======
    # Start with x0 (always 0) and build up predictable values
    
    # Initialize first register: x1 = 0 + 1 = 1
    addi x1, x0, 1          # x1 = 1
    # Expected: addi x1=0x1, x0=0x0, 1
    
    # Initialize second register: x2 = 0 + 2 = 2
    addi x2, x0, 2          # x2 = 2
    # Expected: addi x2=0x2, x0=0x0, 2
    
    # ====== Phase 2: Simple Additions with Known Values ======
    
    # Test 1: 1 + 2 = 3
    add x3, x1, x2          # x3 = 1 + 2 = 3
    # Expected: add x3=0x3, x1=0x1, x2=0x2
    
    # Test 2: 2 + 3 = 5
    add x4, x2, x3          # x4 = 2 + 3 = 5
    # Expected: add x4=0x5, x2=0x2, x3=0x3
    
    # Test 3: 3 + 5 = 8 (Fibonacci sequence)
    add x5, x3, x4          # x5 = 3 + 5 = 8
    # Expected: add x5=0x8, x3=0x3, x4=0x5
    
    # Test 4: 5 + 8 = 13
    add x6, x4, x5          # x6 = 5 + 8 = 13 (0xd)
    # Expected: add x6=0xd, x4=0x5, x5=0x8
    
    # Test 5: 8 + 13 = 21
    add x7, x5, x6          # x7 = 8 + 13 = 21 (0x15)
    # Expected: add x7=0x15, x5=0x8, x6=0xd
    
    # ====== Phase 3: Larger Round Numbers ======
    
    # Initialize with larger values
    addi x8, x0, 10         # x8 = 10 (0xa)
    # Expected: addi x8=0xa, x0=0x0, 10
    
    addi x9, x0, 20         # x9 = 20 (0x14)
    # Expected: addi x9=0x14, x0=0x0, 20
    
    # Test 6: 10 + 20 = 30
    add x10, x8, x9         # x10 = 10 + 20 = 30 (0x1e)
    # Expected: add x10=0x1e, x8=0xa, x9=0x14
    
    addi x11, x0, 50        # x11 = 50 (0x32)
    # Expected: addi x11=0x32, x0=0x0, 50
    
    # Test 7: 30 + 50 = 80
    add x12, x10, x11       # x12 = 30 + 50 = 80 (0x50)
    # Expected: add x12=0x50, x10=0x1e, x11=0x32
    
    # Test 8: 80 + 20 = 100
    add x13, x12, x9        # x13 = 80 + 20 = 100 (0x64)
    # Expected: add x13=0x64, x12=0x50, x9=0x14
    
    # ====== Phase 4: Powers of 2 ======
    
    addi x14, x0, 1         # x14 = 1
    # Expected: addi x14=0x1, x0=0x0, 1
    
    # Build powers of 2 using addition (double)
    add x15, x14, x14       # x15 = 1 + 1 = 2
    # Expected: add x15=0x2, x14=0x1, x14=0x1
    
    add x16, x15, x15       # x16 = 2 + 2 = 4
    # Expected: add x16=0x4, x15=0x2, x15=0x2
    
    add x17, x16, x16       # x17 = 4 + 4 = 8
    # Expected: add x17=0x8, x16=0x4, x16=0x4
    
    add x18, x17, x17       # x18 = 8 + 8 = 16 (0x10)
    # Expected: add x18=0x10, x17=0x8, x17=0x8
    
    add x19, x18, x18       # x19 = 16 + 16 = 32 (0x20)
    # Expected: add x19=0x20, x18=0x10, x18=0x10
    
    add x20, x19, x19       # x20 = 32 + 32 = 64 (0x40)
    # Expected: add x20=0x40, x19=0x20, x19=0x20
    
    add x21, x20, x20       # x21 = 64 + 64 = 128 (0x80)
    # Expected: add x21=0x80, x20=0x40, x20=0x40
    
    add x22, x21, x21       # x22 = 128 + 128 = 256 (0x100)
    # Expected: add x22=0x100, x21=0x80, x21=0x80
    
    # ====== Phase 5: Subtraction Tests (verify rs2 in SUB) ======
    
    # Initialize values for subtraction
    addi x23, x0, 100       # x23 = 100 (0x64)
    # Expected: addi x23=0x64, x0=0x0, 100
    
    addi x24, x0, 40        # x24 = 40 (0x28)
    # Expected: addi x24=0x28, x0=0x0, 40
    
    # Test 9: 100 - 40 = 60
    sub x25, x23, x24       # x25 = 100 - 40 = 60 (0x3c)
    # Expected: sub x25=0x3c, x23=0x64, x24=0x28
    
    # Test 10: 60 - 40 = 20
    sub x26, x25, x24       # x26 = 60 - 40 = 20 (0x14)
    # Expected: sub x26=0x14, x25=0x3c, x24=0x28
    
    # ====== Phase 6: Load/Store Register Value Tests ======
    
    # Set up memory base
    lui x27, 0x80001        # x27 = 0x80001000
    # Expected: lui x27=0x80001000, 0x80001
    
    # Store a value
    addi x28, x0, 123       # x28 = 123 (0x7b)
    # Expected: addi x28=0x7b, x0=0x0, 123
    
    sw x28, 0(x27)          # mem[0x80001000] = 123
    # Expected: sw x28=0x7b, 0(x27=0x80001000)
    
    # Load it back
    lw x29, 0(x27)          # x29 = mem[0x80001000] = 123 (0x7b)
    # Expected: lw x29=0x7b, 0(x27=0x80001000)
    
    # Verify the loaded value by adding to it
    add x30, x29, x1        # x30 = 123 + 1 = 124 (0x7c)
    # Expected: add x30=0x7c, x29=0x7b, x1=0x1
    
    # ====== Test Complete - Signal Success ======
    lui x31, 0x0            # x31 = 0
    addi x31, x31, -16      # x31 = 0xFFFFFFF0 (tohost address)
    addi x30, x0, 42        # x30 = 42 (success code)
    sw x30, 0(x31)          # Store to tohost to halt
    
halt:
    j halt                  # Infinite loop
