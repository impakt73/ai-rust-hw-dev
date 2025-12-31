# riscv_macros

A `no_std` compatible Rust library providing println-like macros for RISC-V bare-metal programs running on the simulated CPU.

## Overview

This library provides formatted print macros that send messages from the simulated RISC-V CPU to the host via the MMIO FIFO using the DebugPacket protocol.

## Macros

### `rvprint!(...)` and `rvprintln!(...)`

Print formatted output to the host console at Info level.

**Usage:**
```rust
use riscv_macros::{rvprint, rvprintln};

rvprintln!("Hello, world!");
rvprintln!("The answer is: {}", 42);
rvprintln!("x = {}, y = {}", 10, 20);

rvprint!("No newline");
rvprint!(" continuation");
```

### `rvdebug!(...)` and `rvdebugln!(...)`

Print formatted debug output to the host console at Debug level.

**Usage:**
```rust
use riscv_macros::{rvdebug, rvdebugln};

rvdebugln!("Debug: variable value = {}", value);
rvdebug!("partial ");
rvdebug!("message");
```

### `rverror!(...)` and `rverrorln!(...)`

Print formatted error output to the host console at Error level.

**Usage:**
```rust
use riscv_macros::{rverror, rverrorln};

rverrorln!("Error: invalid state = {}", state);
rverror!("Error code: ");
rverror!("{}", code);
```

## Requirements

- Programs must be `no_std` compatible
- Requires an allocator (uses `alloc` crate for string formatting)
- Uses MMIO address `0x4000_0000` for FIFO communication

## Example

```rust
#![no_std]
#![no_main]

extern crate alloc;
use riscv_macros::rvprintln;

// ... allocator setup ...

#[entry]
fn main() -> ! {
    rvprintln!("Hello from RISC-V CPU!");
    rvprintln!("The answer is {}", 42);
    
    // Signal program completion
    write_tohost(42);
}
```

## How It Works

1. The macro formats the string using Rust's `format!` macro
2. Creates a `DebugPacket` with the formatted message
3. Serializes the packet using postcard
4. Writes the packet word-by-word to the MMIO FIFO (address `0x4000_0000`)
5. cpu-sim receives the packets and prints them to stdout

## Output

Messages are printed with a level prefix:
- `[INFO]` for `rvprint!` and `rvprintln!`
- `[DEBUG]` for `rvdebug!` and `rvdebugln!`
- `[ERROR]` for `rverror!` and `rverrorln!`

Example output:
```
[INFO] Hello from RISC-V CPU!
[INFO] The answer is 42
[DEBUG] Debug: variable value = 123
[ERROR] Error: invalid state = 5
```
