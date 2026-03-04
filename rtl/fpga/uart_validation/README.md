# UART Validation FPGA Project

This project provides a focused FPGA image for validating `rtl/uart.sv` behavior on real hardware.

It is structured similarly to `rtl/fpga/led_demo`, but uses the newer root FPGA flow (`rtl/fpga/Makefile` and `rtl/fpga/ice40_alchitry_cu/ice40hx8k.pcf`) as the baseline.

## Validation Modes

The top module (`uart_validation_top`) is parameterized by `VALIDATION_MODE`:

- `0`: **Pin loopback** (`usb_rx` is wired directly to `usb_tx`)
- `1`: **UART echo** (received byte is immediately driven to UART TX interface)
- `2`: **UART echo + FIFO** (echo path includes `rtl/sync_fifo.sv` between UART RX and TX)

Default mode is `2`.

## Build and Program

```bash
cd rtl/fpga/uart_validation
make                # build bitstream
make program        # flash to connected board
```

To choose another mode, pass `VALIDATION_MODE` to `make`:

```bash
make clean
make VALIDATION_MODE=0   # direct pin loopback
make VALIDATION_MODE=1   # uart echo
make VALIDATION_MODE=2   # uart echo + fifo
```

## Host Validator Crate

A minimal Rust validator is included in `host-validator/`.

### Usage

```bash
cd rtl/fpga/uart_validation/host-validator
cargo run -- /dev/ttyUSB0 1000000
```

The tool sends fixed test patterns and verifies each received byte matches what was transmitted.
