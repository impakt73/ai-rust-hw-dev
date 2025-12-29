.section .text
.global _start

_start:
    # Initialize registers
    addi x1, x0, 10      # x1 = 10
    addi x2, x0, 20      # x2 = 20
    add x3, x1, x2       # x3 = x1 + x2 = 30
    sub x4, x2, x1       # x4 = x2 - x1 = 10
    
    # Logic operations
    and x5, x1, x2       # x5 = x1 & x2
    or x6, x1, x2        # x6 = x1 | x2
    xor x7, x1, x2       # x7 = x1 ^ x2
    
    # Store result to tohost to halt
    lui x8, 0x0          # Load upper immediate (0)
    addi x8, x8, -16     # x8 = 0xFFFFFFF0 (tohost address)
    sw x3, 0(x8)         # Store result to tohost
    
    # Should halt here
loop:
    j loop
