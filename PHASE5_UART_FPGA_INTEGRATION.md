# Phase 5: UART FPGA Integration Summary

## Overview
Successfully integrated the UART peripheral into the FPGA top-level module (`fpga/fpga_top.sv`), connecting the on-chip UART controller to the external USB serial interface via the FTDI chip.

## Changes Made to `fpga/fpga_top.sv`

### 1. Added UART Parameters to `top_with_peripherals` Instantiation
```systemverilog
top_with_peripherals #(
    .ENABLE_M_EXT(ENABLE_M_EXT),
    .ENABLE_F_EXT(ENABLE_F_EXT),
    .UART_CLK_FREQ_HZ(25_000_000),  // 25 MHz (PLL output)
    .UART_BAUD_RATE(115200),
    .ENABLE_UART_LOOPBACK(1'b0)     // Disable loopback for FPGA
) cpu (
    ...
);
```

**Parameter Details:**
- `UART_CLK_FREQ_HZ = 25_000_000`: Matches the FPGA's 25 MHz system clock (PLL divides 100 MHz to 25 MHz)
- `UART_BAUD_RATE = 115200`: Standard baud rate for serial communication
- `ENABLE_UART_LOOPBACK = 1'b0`: Disables internal loopback (required for external communication)

### 2. Connected UART Ports to USB Serial Pins
```systemverilog
// UART peripheral - connect to USB serial
.uart_tx(usb_tx),
.uart_rx(usb_rx),
```

**Pin Mapping (from `fpga/ice40hx8k.pcf`):**
- `usb_tx` → Pin M9 (UART transmit to FTDI chip)
- `usb_rx` → Pin P14 (UART receive from FTDI chip)

### 3. Removed Simple USB Loopback
**Before:**
```systemverilog
// ============================================================
// USB Serial Loopback
// ============================================================
assign usb_tx = usb_rx;
```

**After:** (Removed entirely)

The simple loopback is no longer needed because the UART controller now handles the serial communication.

## Verification

### Syntax Check
Ran Verilator lint check to verify correctness:
```bash
verilator --lint-only -Wall \
  --top-module fpga_top \
  -Irtl -Irtl/peripherals -Ifpga \
  fpga/fpga_top.sv rtl/top_with_peripherals.sv ...
```

**Result:** ✅ No errors related to UART integration
- All warnings are pre-existing (not introduced by these changes)
- UART parameters and port connections verified successfully

## Integration Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ fpga_top.sv (FPGA Top Module)                               │
│                                                               │
│  ┌──────────────────────────────────────┐                    │
│  │ top_with_peripherals                 │                    │
│  │                                      │                    │
│  │  ┌──────────────────┐                │                    │
│  │  │ UART Controller  │                │                    │
│  │  │  (uart.sv)       │                │                    │
│  │  │                  │                │                    │
│  │  │  CLK: 25 MHz     │                │                    │
│  │  │  Baud: 115200    │                │                    │
│  │  │  Loopback: OFF   │                │                    │
│  │  └─────┬──────┬─────┘                │                    │
│  │        │      │                      │                    │
│  └────────┼──────┼──────────────────────┘                    │
│           │      │                                            │
│        uart_tx uart_rx                                        │
│           │      │                                            │
│        usb_tx  usb_rx (FPGA I/O Pins)                         │
└───────────┼──────┼────────────────────────────────────────────┘
            │      │
         Pin M9  Pin P14
            │      │
            v      v
     ┌──────────────────┐
     │ FTDI USB Serial  │
     │ Chip (on-board)  │
     └──────────────────┘
            │
            v
        USB Port
```

## Memory Map (Unchanged)
The UART peripheral remains at the same address range:
- **Base Address:** `0x52000000`
- **Size:** 256 bytes
- **Registers:**
  - `0x52000000`: TX Data Register
  - `0x52000004`: RX Data Register
  - `0x52000008`: Status Register
  - `0x5200000C`: Control Register

## Next Steps

### For FPGA Deployment:
1. **Synthesize the design:**
   ```bash
   cd fpga
   make
   ```

2. **Program the FPGA:**
   ```bash
   make prog
   ```

3. **Connect to USB serial:**
   ```bash
   # Linux/macOS
   screen /dev/ttyUSB0 115200
   
   # Or use minicom
   minicom -D /dev/ttyUSB0 -b 115200
   ```

### Testing:
- Write test programs that use the UART peripheral at address `0x52000000`
- Verify bidirectional communication through the USB serial interface
- Test different baud rates if needed (requires recompiling with different `UART_BAUD_RATE`)

## Files Modified
- `fpga/fpga_top.sv`: Added UART parameters and port connections, removed simple loopback

## Files Referenced
- `rtl/top_with_peripherals.sv`: Defines UART parameters and ports
- `rtl/peripherals/uart.sv`: UART controller implementation
- `fpga/ice40hx8k.pcf`: Pin constraints (USB serial pins M9, P14)

## Technical Notes

### Clock Configuration
The FPGA uses a PLL to generate the 25 MHz system clock from the 100 MHz on-board oscillator:
- Input: 100 MHz
- PLL Config: DIVR=0, DIVF=7, DIVQ=5
- Output: 100 MHz × (7+1) / 2^5 = 25 MHz

This 25 MHz clock is used for both the CPU and UART peripheral, ensuring proper timing for the 115200 baud rate.

### Baud Rate Calculation
With `UART_CLK_FREQ_HZ = 25_000_000` and `UART_BAUD_RATE = 115200`:
```
Baud Divisor = 25,000,000 / 115,200 ≈ 217
Actual Baud Rate = 25,000,000 / 217 ≈ 115,207 baud
Error: ~0.006% (well within acceptable tolerance)
```

## Status
✅ **Phase 5 Complete**: UART peripheral successfully integrated into FPGA top module
- All syntax checks pass
- Parameters configured correctly for 25 MHz clock and 115200 baud
- USB serial pins properly connected
- Ready for synthesis and FPGA deployment
