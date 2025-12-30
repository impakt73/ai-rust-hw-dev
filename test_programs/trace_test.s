# Simple RISC-V assembly test for instruction trace validation
# This program performs specific operations that can be validated via the trace callback

.section .text
.globl _start

_start:
    # Test ADDI instructions - easy to validate
    addi x1, x0, 10      # x1 = 10
    addi x2, x0, 20      # x2 = 20
    addi x3, x0, 5       # x3 = 5

    # Test ADD instruction
    add x4, x1, x2       # x4 = x1 + x2 = 30

    # Test SUB instruction  
    sub x5, x2, x3       # x5 = x2 - x3 = 15

    # Test AND instruction
    andi x6, x1, 0xFF    # x6 = x1 & 0xFF = 10

    # Test OR instruction
    ori x7, x2, 0x1      # x7 = x2 | 0x1 = 21

    # Test LUI instruction
    lui x8, 0x12345      # x8 = 0x12345000

    # Test SW and LW instructions
    sw x1, 0(x0)         # Store x1 (10) to address 0
    lw x9, 0(x0)         # Load from address 0 to x9 (should be 10)

    # Exit with success code (42)
    # Write to tohost address 0xFFFFFFF0 (which is -16 in 32-bit)
    addi x10, x0, 42     # x10 = 42
    addi x11, x0, -16    # x11 = 0xFFFFFFF0 (sign-extended -16)
    sw x10, 0(x11)       # Write to tohost address (0xFFFFFFF0)

    # Infinite loop (should never reach here)
loop:
    j loop
