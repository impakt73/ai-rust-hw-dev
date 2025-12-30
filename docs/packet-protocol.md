# Binary Packet Communication Protocol Specification

## Version 1.0

**Last Updated:** 2025-12-30

---

## Table of Contents

1. [Overview](#overview)
2. [Design Goals](#design-goals)
3. [Architecture](#architecture)
4. [Packet Structure](#packet-structure)
5. [Packet Types](#packet-types)
6. [Serialization Format](#serialization-format)
7. [Transport Layer](#transport-layer)
8. [Error Handling](#error-handling)
9. [Example Implementations](#example-implementations)
10. [Memory Layout](#memory-layout)
11. [Usage Examples](#usage-examples)
12. [Future Extensions](#future-extensions)

---

## Overview

This specification defines a **binary packet-based communication protocol** for bidirectional data exchange between:
- **Simulated CPU Programs**: RISC-V RV32I bare-metal programs running on the hardware simulator
- **Host Code**: Rust applications controlling the simulation environment

The protocol is designed to be:
- **Zero-copy efficient** using the `rkyv` serialization framework
- **Type-safe** with shared Rust struct definitions across host and CPU code
- **Extensible** for future packet types and features
- **Simple** to implement with Rust on both host and CPU sides, enabling code reuse

---

## Design Goals

### Primary Goals

1. **Bidirectional Communication**: Enable both CPU→Host and Host→CPU data transfer
2. **Type Safety**: Use Rust's type system to prevent protocol violations on both host and CPU
3. **Zero-Copy Serialization**: Minimize memory copies using `rkyv`
4. **Code Sharing**: Share packet definitions between host and CPU via common Rust crate
5. **Compatibility**: Work with existing FIFO-based memory-mapped I/O infrastructure
6. **Simplicity**: Easy to use from Rust code on both host and CPU sides
7. **Extensibility**: Support for future packet types without breaking changes

### Non-Goals

- Real-time guarantees (this is a simulation environment)
- Encryption or authentication (trusted environment)
- Flow control beyond basic FIFO status flags
- Complex routing or multiplexing (single point-to-point channel)

### Code Sharing Strategy

A key advantage of using Rust for both host and CPU code is **complete code reuse** of packet definitions:

1. **Shared Packet Definitions**: A single `riscv_protocol` crate defines all packet structures
2. **No Manual Synchronization**: Both sides use identical `rkyv`-derived structs
3. **Compile-Time Safety**: Type mismatches are caught at compile time, not runtime
4. **`no_std` Compatibility**: The protocol crate works in both std (host) and no_std (bare-metal CPU) environments
5. **Same Serialization**: Both sides use `rkyv::to_bytes()` and `rkyv::check_archived_root()`

**Example Workflow**:
- Host imports: `use riscv_protocol::DebugPacket;`
- CPU imports: `use riscv_protocol::DebugPacket;` (same struct!)
- Both serialize identically: `rkyv::to_bytes(&packet)`
- Zero risk of protocol drift between implementations

---

## Architecture

### System Components

```
┌─────────────────────────────────────────────────────────────┐
│                        Host System                           │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Rust Application (cpu-sim or test harness)          │   │
│  │  - Encodes packets using rkyv                        │   │
│  │  - Writes to FIFO RX (Host→CPU)                      │   │
│  │  - Reads from FIFO TX (CPU→Host)                     │   │
│  │  - Decodes packets using rkyv                        │   │
│  └────────────┬──────────────────────────┬───────────────┘   │
│               │                          │                   │
│               ▼                          ▼                   │
│  ┌────────────────────┐    ┌────────────────────┐          │
│  │   FIFO RX Queue    │    │   FIFO TX Queue    │          │
│  │   (Host → CPU)     │    │   (CPU → Host)     │          │
│  │  VecDeque<u32>     │    │  VecDeque<u32>     │          │
│  └────────────┬───────┘    └───────┬────────────┘          │
│               │                     │                        │
└───────────────┼─────────────────────┼────────────────────────┘
                │                     │
                │  Memory-Mapped I/O  │
                │   0x40000000-0x4   │
                │                     │
┌───────────────┼─────────────────────┼────────────────────────┐
│               ▼                     ▼                        │
│  ┌────────────────────┐    ┌────────────────────┐          │
│  │  FIFO_DATA (RD)    │    │  FIFO_DATA (WR)    │          │
│  │  Address:          │    │  Address:          │          │
│  │  0x40000000        │    │  0x40000000        │          │
│  └────────────────────┘    └────────────────────┘          │
│  ┌────────────────────┐                                     │
│  │  FIFO_STATUS       │                                     │
│  │  Address:          │                                     │
│  │  0x40000004        │                                     │
│  │  [0]: RX_VALID     │                                     │
│  │  [1]: TX_READY     │                                     │
│  └────────────────────┘                                     │
│                                                              │
│                    Simulated CPU                             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  RISC-V RV32I Bare-Metal Rust Program                │   │
│  │  - Uses same packet definitions via shared crate     │   │
│  │  - Encodes/decodes packets using rkyv                │   │
│  │  - Reads from FIFO using memory-mapped loads         │   │
│  │  - Writes to FIFO using memory-mapped stores         │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Data Flow

1. **Host→CPU (RX Direction)**:
   - Host encodes packet struct using `rkyv::to_bytes()`
   - Host writes u32 words to `FIFO.rx` queue
   - CPU polls `FIFO_STATUS[0]` (RX_VALID flag)
   - CPU reads u32 words from `FIFO_DATA` register
   - CPU reconstructs packet from byte stream

2. **CPU→Host (TX Direction)**:
   - CPU constructs packet in memory
   - CPU polls `FIFO_STATUS[1]` (TX_READY flag)
   - CPU writes u32 words to `FIFO_DATA` register
   - Host reads from `FIFO.tx` queue
   - Host decodes packet using `rkyv::check_archived_root()` and `archived.deserialize()`

---

## Packet Structure

### Base Packet Header

All packets share a common header structure for framing and type identification:

```rust
use rkyv::{Archive, Deserialize, Serialize};

/// Common packet header (8 bytes)
#[derive(Archive, Deserialize, Serialize, Debug, Clone, PartialEq)]
#[archive(check_bytes)]
pub struct PacketHeader {
    /// Magic number for packet validation (0x52565043 = "RVPC" in ASCII)
    pub magic: u32,
    
    /// Total packet length in bytes (including header)
    pub length: u16,
    
    /// Packet type identifier
    pub packet_type: PacketType,
    
    /// Reserved for future use / alignment (set to 0)
    pub reserved: u8,
}

/// Packet type enumeration
#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[archive(check_bytes)]
#[repr(u8)]
pub enum PacketType {
    // Basic communication packets
    Nop = 0,              // No operation / keepalive
    Echo = 1,             // Echo request/response for testing
    
    // Data transfer packets
    DataU32 = 10,         // Single 32-bit unsigned integer
    DataI32 = 11,         // Single 32-bit signed integer
    DataBuffer = 12,      // Arbitrary byte buffer
    DataString = 13,      // UTF-8 string
    
    // Control packets
    Reset = 20,           // Request CPU reset
    Halt = 21,            // Request simulation halt
    Status = 22,          // Status query/response
    
    // Register access packets
    RegisterRead = 30,    // Read CPU register(s)
    RegisterWrite = 31,   // Write CPU register(s)
    
    // Memory access packets
    MemoryRead = 40,      // Read memory region
    MemoryWrite = 41,     // Write memory region
    
    // Test/Debug packets
    Assert = 50,          // Test assertion result
    Debug = 51,           // Debug message
    
    // Error packets
    Error = 255,          // Error notification
}
```

### Packet Header Layout

```
Offset | Size | Field         | Description
-------|------|---------------|----------------------------------
0x00   | 4    | magic         | 0x52565043 ("RVPC" magic number)
0x04   | 2    | length        | Total packet size (bytes)
0x06   | 1    | packet_type   | PacketType enum value
0x07   | 1    | reserved      | Reserved (must be 0)
-------|------|---------------|----------------------------------
Total: 8 bytes
```

### Design Rationale

- **Magic Number**: Provides packet framing validation
- **Length Field**: Enables variable-length packets and proper buffer allocation
- **Type Field**: Explicit packet type for deserialization
- **Reserved Byte**: Ensures 32-bit alignment and future extensibility

---

## Packet Types

### 1. NOP Packet

**Purpose**: Keepalive, synchronization, or padding

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct NopPacket {
    pub header: PacketHeader,
}
```

**Size**: 8 bytes (header only)

---

### 2. Echo Packet

**Purpose**: Testing connectivity and round-trip latency

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct EchoPacket {
    pub header: PacketHeader,
    pub sequence: u32,    // Sequence number for matching request/response
    pub timestamp: u64,   // Timestamp in cycles or microseconds
}
```

**Size**: 8 + 12 = 20 bytes

**Usage**:
- Host sends `EchoPacket` with sequence number
- CPU responds with same sequence number
- Host measures round-trip time

---

### 3. Data Packets

#### DataU32Packet

**Purpose**: Transfer a single unsigned 32-bit value

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct DataU32Packet {
    pub header: PacketHeader,
    pub value: u32,
    pub tag: u32,         // Optional identifier/tag
}
```

**Size**: 8 + 8 = 16 bytes

#### DataI32Packet

**Purpose**: Transfer a single signed 32-bit value

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct DataI32Packet {
    pub header: PacketHeader,
    pub value: i32,
    pub tag: u32,         // Optional identifier/tag
}
```

**Size**: 8 + 8 = 16 bytes

#### DataBufferPacket

**Purpose**: Transfer arbitrary binary data

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct DataBufferPacket {
    pub header: PacketHeader,
    pub buffer_id: u32,   // Buffer identifier
    pub offset: u32,      // Offset within buffer (for partial transfers)
    pub data: Vec<u8>,    // Variable-length payload
}
```

**Size**: 8 + 8 + variable (minimum 16 bytes)

**Note**: When serializing with `rkyv`, `Vec<u8>` is stored inline

#### DataStringPacket

**Purpose**: Transfer UTF-8 text strings

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct DataStringPacket {
    pub header: PacketHeader,
    pub string_id: u32,   // String identifier
    pub text: String,     // UTF-8 string (variable-length)
}
```

**Size**: 8 + 4 + variable (minimum 12 bytes)

---

### 4. Control Packets

#### ResetPacket

**Purpose**: Request CPU reset

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct ResetPacket {
    pub header: PacketHeader,
    pub reset_type: ResetType,
    pub reserved: [u8; 3],
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[archive(check_bytes)]
#[repr(u8)]
pub enum ResetType {
    Soft = 0,    // Software-triggered reset
    Hard = 1,    // Hardware reset (full state clear)
}
```

**Size**: 8 + 4 = 12 bytes

#### HaltPacket

**Purpose**: Request simulation halt/termination

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct HaltPacket {
    pub header: PacketHeader,
    pub exit_code: i32,   // Exit code (0 = success, non-zero = error)
}
```

**Size**: 8 + 4 = 12 bytes

#### StatusPacket

**Purpose**: Query or report system status

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct StatusPacket {
    pub header: PacketHeader,
    pub cycle_count: u64,      // Current cycle count
    pub pc: u32,               // Current program counter
    pub status_flags: u32,     // Bit flags for various status indicators
}
```

**Size**: 8 + 16 = 24 bytes

---

### 5. Register Access Packets

#### RegisterReadPacket

**Purpose**: Read one or more CPU registers

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct RegisterReadPacket {
    pub header: PacketHeader,
    pub register_indices: Vec<u8>,  // List of register numbers (0-31)
}

/// Response packet
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct RegisterReadResponsePacket {
    pub header: PacketHeader,
    pub values: Vec<u32>,    // Register values in same order as request
}
```

**Request Size**: 8 + variable (minimum 8 bytes)
**Response Size**: 8 + variable (minimum 8 bytes)

#### RegisterWritePacket

**Purpose**: Write one or more CPU registers

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct RegisterWritePacket {
    pub header: PacketHeader,
    pub writes: Vec<RegisterWrite>,
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct RegisterWrite {
    pub register_index: u8,
    pub reserved: [u8; 3],
    pub value: u32,
}
```

**Size**: 8 + variable (minimum 8 bytes)

---

### 6. Memory Access Packets

#### MemoryReadPacket

**Purpose**: Read a contiguous memory region

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct MemoryReadPacket {
    pub header: PacketHeader,
    pub address: u32,     // Starting memory address
    pub length: u32,      // Number of bytes to read
}

/// Response packet
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct MemoryReadResponsePacket {
    pub header: PacketHeader,
    pub address: u32,     // Starting address (echoed from request)
    pub data: Vec<u8>,    // Memory contents
}
```

**Request Size**: 8 + 8 = 16 bytes
**Response Size**: 8 + 4 + variable (minimum 12 bytes)

#### MemoryWritePacket

**Purpose**: Write to a contiguous memory region

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct MemoryWritePacket {
    pub header: PacketHeader,
    pub address: u32,     // Starting memory address
    pub data: Vec<u8>,    // Data to write
}
```

**Size**: 8 + 4 + variable (minimum 12 bytes)

---

### 7. Test/Debug Packets

#### AssertPacket

**Purpose**: Report test assertion results

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct AssertPacket {
    pub header: PacketHeader,
    pub passed: bool,         // True if assertion passed
    pub reserved: [u8; 3],
    pub test_id: u32,         // Test case identifier
    pub expected: u32,        // Expected value
    pub actual: u32,          // Actual value
    pub message: String,      // Optional description
}
```

**Size**: 8 + 16 + variable (minimum 24 bytes)

#### DebugPacket

**Purpose**: General debug messages from CPU to host

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct DebugPacket {
    pub header: PacketHeader,
    pub level: DebugLevel,
    pub reserved: [u8; 3],
    pub message: String,
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[archive(check_bytes)]
#[repr(u8)]
pub enum DebugLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warning = 3,
    Error = 4,
}
```

**Size**: 8 + 4 + variable (minimum 12 bytes)

---

### 8. Error Packet

**Purpose**: Report errors in packet processing

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct ErrorPacket {
    pub header: PacketHeader,
    pub error_code: ErrorCode,
    pub reserved: [u8; 3],
    pub details: String,      // Human-readable error description
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[archive(check_bytes)]
#[repr(u8)]
pub enum ErrorCode {
    InvalidMagic = 1,         // Bad magic number
    InvalidLength = 2,        // Length field doesn't match data
    UnknownPacketType = 3,    // Unrecognized packet type
    DeserializationFailed = 4,// rkyv deserialization error
    BufferOverflow = 5,       // Packet too large for buffer
    FifoOverflow = 6,         // FIFO queue full
    InvalidAddress = 7,       // Memory access to invalid address
    InvalidRegister = 8,      // Invalid register index
    PermissionDenied = 9,     // Operation not allowed
}
```

**Size**: 8 + 4 + variable (minimum 12 bytes)

---

## Serialization Format

### Using `rkyv`

The `rkyv` crate provides zero-copy deserialization with validation. All packet structs use the following attributes:

```rust
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]  // Enable bytecheck validation
pub struct MyPacket {
    // fields...
}
```

### Serialization Process (Host Side)

```rust
use rkyv::{to_bytes, rancor::Error};

// Create packet
let packet = DataU32Packet {
    header: PacketHeader {
        magic: 0x52565043,
        length: 16,  // Must match actual serialized size
        packet_type: PacketType::DataU32,
        reserved: 0,
    },
    value: 42,
    tag: 100,
};

// Serialize to bytes
let bytes = to_bytes::<Error>(&packet)
    .expect("Serialization failed");

// Write to FIFO as u32 words
for chunk in bytes.chunks(4) {
    let mut word: u32 = 0;
    for (i, &byte) in chunk.iter().enumerate() {
        word |= (byte as u32) << (i * 8);
    }
    simulator.fifo_write_rx(word);
}
```

### Deserialization Process (Host Side)

```rust
use rkyv::{check_archived_root, Deserialize};
use rkyv::rancor::Error;

// Read from FIFO as u32 words
let mut bytes = Vec::new();
while let Some(word) = fifo.tx.pop_front() {
    bytes.extend_from_slice(&word.to_le_bytes());
}

// Validate and deserialize
let archived = check_archived_root::<ErrorPacket>(&bytes)
    .expect("Validation failed");
let packet: ErrorPacket = archived.deserialize(&mut rkyv::Infallible)
    .expect("Deserialization failed");
```

### Memory Layout Considerations

`rkyv` produces **architecture-independent** serialized data by default:
- Little-endian byte order
- Aligned struct fields
- Fixed-size representation

This matches RISC-V's little-endian architecture perfectly.

---

## Transport Layer

### FIFO-Based Transport

The protocol uses the existing FIFO infrastructure documented in [cpu-sim/README.md](../cpu-sim/README.md).

#### Memory-Mapped Registers

| Address      | Name         | Access | Description                    |
|--------------|--------------|--------|--------------------------------|
| 0x40000000   | FIFO_DATA    | R/W    | Read from RX / Write to TX     |
| 0x40000004   | FIFO_STATUS  | R      | Bit[0]=RX_VALID, Bit[1]=TX_READY |

#### Transmit Sequence (CPU→Host)

**Rust Example for Bare-Metal CPU**:

```rust
// Memory-mapped FIFO registers (CPU-side bare metal)
const FIFO_DATA: *mut u32 = 0x40000000 as *mut u32;
const FIFO_STATUS: *const u32 = 0x40000004 as *const u32;
const TX_READY: u32 = 1 << 1;

/// Send a packet to the host via FIFO
/// 
/// # Safety
/// This function performs volatile memory operations to FIFO MMIO registers
pub unsafe fn send_packet(packet_bytes: &[u8]) {
    // Ensure length is multiple of 4 (pad if necessary)
    let num_words = (packet_bytes.len() + 3) / 4;
    
    for i in 0..num_words {
        // Wait for TX ready
        while FIFO_STATUS.read_volatile() & TX_READY == 0 {
            // Busy-wait (could use WFI instruction in real implementation)
            core::hint::spin_loop();
        }
        
        // Construct word from up to 4 bytes (little-endian)
        let mut word: u32 = 0;
        let base = i * 4;
        for j in 0..4 {
            if base + j < packet_bytes.len() {
                word |= (packet_bytes[base + j] as u32) << (j * 8);
            }
        }
        
        // Write word to FIFO
        FIFO_DATA.write_volatile(word);
    }
}
```

#### Receive Sequence (CPU reads from Host)

**Rust Example for Bare-Metal CPU**:

```rust
const RX_VALID: u32 = 1 << 0;

/// Receive a complete packet from the host via FIFO
/// 
/// # Safety
/// This function performs volatile memory operations to FIFO MMIO registers
/// 
/// Returns the number of bytes received, or 0 on error
pub unsafe fn receive_packet(buffer: &mut [u8]) -> usize {
    let max_length = buffer.len();
    let words = buffer.as_mut_ptr() as *mut u32;
    let mut word_count: usize = 0;
    
    // Read header first (2 words = 8 bytes)
    while word_count < 2 {
        if FIFO_STATUS.read_volatile() & RX_VALID != 0 {
            words.add(word_count).write_volatile(FIFO_DATA.read_volatile());
            word_count += 1;
        }
    }
    
    // Parse header to get total length
    let header = &*(buffer.as_ptr() as *const PacketHeader);
    if header.magic != 0x52565043 {
        return 0;  // Invalid magic
    }
    
    let total_words = (header.length as usize + 3) / 4;
    if total_words * 4 > max_length {
        return 0;  // Buffer too small
    }
    
    // Read remaining words
    while word_count < total_words {
        if FIFO_STATUS.read_volatile() & RX_VALID != 0 {
            words.add(word_count).write_volatile(FIFO_DATA.read_volatile());
            word_count += 1;
        }
    }
    
    header.length as usize
}
```

### Framing and Synchronization

**Packet Boundaries**: 
- Each packet is self-contained with its `length` field
- No inter-packet delimiters needed
- Magic number provides framing validation

**Word Alignment**:
- All packets should be padded to 4-byte boundaries
- FIFO operates on 32-bit words
- Padding bytes should be zero

**Synchronization Strategy**:
1. Receiver scans for magic number (0x52565043)
2. Validates length field
3. Reads complete packet
4. Validates with `rkyv::check_archived_root()`

---

## Error Handling

### Error Detection

1. **Magic Number Validation**: First line of defense against desynchronization
2. **Length Validation**: Prevents buffer overruns
3. **rkyv Validation**: The `#[archive(check_bytes)]` attribute enables:
   - Struct alignment validation
   - Enum discriminant validation
   - String UTF-8 validation
   - Pointer safety checks

### Error Response Protocol

When an error occurs during packet processing:

1. **Generate ErrorPacket**: Create packet with appropriate `ErrorCode`
2. **Send Response**: Transmit error packet to sender
3. **Log Locally**: Record error for debugging
4. **Attempt Recovery**: 
   - Discard invalid packet
   - Continue processing subsequent packets
   - Optional: Request retransmission

### Example Error Handling (Host)

```rust
fn process_received_packet(bytes: &[u8]) -> Result<(), String> {
    // Validate magic number
    if bytes.len() < 4 {
        return Err("Packet too short".to_string());
    }
    
    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if magic != 0x52565043 {
        send_error_packet(ErrorCode::InvalidMagic);
        return Err("Invalid magic number".to_string());
    }
    
    // Validate length
    if bytes.len() < 8 {
        return Err("Incomplete header".to_string());
    }
    
    let length = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    if length != bytes.len() {
        send_error_packet(ErrorCode::InvalidLength);
        return Err("Length mismatch".to_string());
    }
    
    // Determine packet type and deserialize
    let packet_type = bytes[6];
    match packet_type {
        x if x == PacketType::DataU32 as u8 => {
            match check_archived_root::<DataU32Packet>(bytes) {
                Ok(archived) => {
                    let packet = archived.deserialize(&mut rkyv::Infallible)?;
                    handle_data_u32_packet(packet);
                    Ok(())
                }
                Err(e) => {
                    send_error_packet(ErrorCode::DeserializationFailed);
                    Err(format!("Deserialization failed: {:?}", e))
                }
            }
        }
        // ... other packet types ...
        _ => {
            send_error_packet(ErrorCode::UnknownPacketType);
            Err("Unknown packet type".to_string())
        }
    }
}
```

---

## Example Implementations

### Example 1: CPU Sends Debug Message to Host

**CPU Side (Rust bare-metal)**:

```rust
use rkyv::{to_bytes, rancor::Error};

/// Send a debug message from CPU to host
/// 
/// # Safety
/// Uses unsafe FIFO operations for memory-mapped I/O
pub fn send_debug_message(level: DebugLevel, message: &str) {
    // Create debug packet using shared packet definitions
    let packet = DebugPacket {
        header: PacketHeader {
            magic: 0x52565043,
            length: 0,  // Will be set by rkyv
            packet_type: PacketType::Debug,
            reserved: 0,
        },
        level,
        reserved: [0; 3],
        message: message.to_string(),
    };
    
    // Serialize with rkyv (same as host side!)
    let bytes = to_bytes::<Error>(&packet).expect("Serialization failed");
    
    // Send via FIFO
    unsafe {
        send_packet(&bytes);
    }
}

// Example usage in CPU program
#[no_mangle]
pub extern "C" fn main() -> ! {
    // Initialize hardware...
    
    send_debug_message(DebugLevel::Info, "CPU started successfully");
    
    // ... rest of program ...
    
    loop {}
}
```

**Host Side (Rust)**:

```rust
use cpu_sim::Simulator;
use rkyv::{check_archived_root, Deserialize};

/// Handle debug messages from CPU
fn handle_cpu_debug_messages(simulator: &mut Simulator) {
    // Collect complete packet from FIFO TX queue
    let mut bytes = vec![];
    while let Some(word) = simulator.bus.fifo.tx.pop_front() {
        bytes.extend_from_slice(&word.to_le_bytes());
        
        // Check if we have a complete packet
        if bytes.len() >= 8 {
            let header = unsafe { &*(bytes.as_ptr() as *const PacketHeader) };
            if header.magic == 0x52565043 && bytes.len() >= header.length as usize {
                break;
            }
        }
    }
    
    // Deserialize using rkyv (same as CPU side!)
    if let Ok(archived) = check_archived_root::<DebugPacket>(&bytes) {
        let packet: DebugPacket = archived.deserialize(&mut rkyv::Infallible).unwrap();
        println!("[CPU DEBUG] {}: {}", 
            match packet.level {
                DebugLevel::Info => "INFO",
                DebugLevel::Error => "ERROR",
                DebugLevel::Warning => "WARN",
                DebugLevel::Debug => "DEBUG",
                DebugLevel::Trace => "TRACE",
            },
            packet.message
        );
    }
}
```

---

### Example 2: Host Reads CPU Registers

**Host Side (Rust)**:

```rust
use rkyv::{to_bytes, check_archived_root, Deserialize, rancor::Error};

fn read_cpu_registers(simulator: &mut Simulator, registers: &[u8]) -> Vec<u32> {
    // Create request packet
    let request = RegisterReadPacket {
        header: PacketHeader {
            magic: 0x52565043,
            length: 0,  // Will be set by rkyv
            packet_type: PacketType::RegisterRead,
            reserved: 0,
        },
        register_indices: registers.to_vec(),
    };
    
    // Serialize and send
    let bytes = to_bytes::<Error>(&request).unwrap();
    for chunk in bytes.chunks(4) {
        let mut word: u32 = 0;
        for (i, &byte) in chunk.iter().enumerate() {
            word |= (byte as u32) << (i * 8);
        }
        simulator.fifo_write_rx(word);
    }
    
    // Wait for response (with timeout)
    let mut timeout = 1000;
    while simulator.bus.fifo.tx.is_empty() && timeout > 0 {
        simulator.step();  // Execute one cycle
        timeout -= 1;
    }
    
    // Read response
    let mut response_bytes = vec![];
    while let Some(word) = simulator.bus.fifo.tx.pop_front() {
        response_bytes.extend_from_slice(&word.to_le_bytes());
    }
    
    // Deserialize response
    let archived = check_archived_root::<RegisterReadResponsePacket>(&response_bytes).unwrap();
    let response: RegisterReadResponsePacket = archived.deserialize(&mut rkyv::Infallible).unwrap();
    response.values
}
```

**CPU Side (Rust bare-metal handler)**:

```rust
use rkyv::{to_bytes, check_archived_root, Deserialize, rancor::Error};

/// Handle register read request from host
/// 
/// # Safety
/// Accesses CPU registers and FIFO hardware
pub unsafe fn handle_register_read_request(packet_bytes: &[u8]) {
    // Deserialize request using shared packet definitions
    let archived = check_archived_root::<RegisterReadPacket>(packet_bytes)
        .expect("Invalid packet");
    let request: RegisterReadPacket = archived.deserialize(&mut rkyv::Infallible)
        .expect("Deserialization failed");
    
    // Read CPU registers (implementation specific)
    let mut values = vec![];
    for &reg_idx in &request.register_indices {
        let value = read_cpu_register(reg_idx);
        values.push(value);
    }
    
    // Build response packet
    let response = RegisterReadResponsePacket {
        header: PacketHeader {
            magic: 0x52565043,
            length: 0,  // Will be set by rkyv
            packet_type: PacketType::RegisterRead,
            reserved: 0,
        },
        values,
    };
    
    // Serialize and send (same as host!)
    let bytes = to_bytes::<Error>(&response).expect("Serialization failed");
    send_packet(&bytes);
}

/// Read a CPU register value (bare-metal implementation)
/// This would use inline assembly or intrinsics
unsafe fn read_cpu_register(index: u8) -> u32 {
    // Example: reading from register file
    // In real implementation, might use inline asm or global register array
    match index {
        0 => 0,  // x0 is always zero
        1..=31 => {
            // Read from actual register file
            // This is implementation-specific
            core::arch::asm!(
                "mv {0}, x{1}",
                out(reg) _,
                const index,
            );
            0  // Placeholder
        }
        _ => 0,
    }
}
```

---

### Example 3: CPU Performs Self-Test and Reports Results

**CPU Side (Rust bare-metal)**:

```rust
use rkyv::{to_bytes, rancor::Error};

/// Run a self-test and report results to host
pub fn run_self_test() {
    let test_value = compute_something();
    let expected = 0x1234;
    
    // Create assertion packet using shared definitions
    let packet = AssertPacket {
        header: PacketHeader {
            magic: 0x52565043,
            length: 0,  // Will be set by rkyv
            packet_type: PacketType::Assert,
            reserved: 0,
        },
        passed: test_value == expected,
        reserved: [0; 3],
        test_id: 1,
        expected,
        actual: test_value,
        message: if test_value == expected {
            "Test passed".to_string()
        } else {
            "Test failed".to_string()
        },
    };
    
    // Serialize with rkyv (same as host!)
    let bytes = to_bytes::<Error>(&packet).expect("Serialization failed");
    
    // Send to host
    unsafe {
        send_packet(&bytes);
    }
}

fn compute_something() -> u32 {
    // Actual test computation
    0x1234
}
```

**Host Side (Rust)**:

```rust
use rkyv::{check_archived_root, Deserialize};

/// Collect test results from CPU
fn collect_test_results(simulator: &mut Simulator) -> Vec<AssertPacket> {
    let mut results = vec![];
    
    // Read all assertion packets from FIFO
    while let Some(packet_bytes) = read_packet_from_fifo(&mut simulator.bus.fifo.tx) {
        // Deserialize using shared packet definitions
        if let Ok(archived) = check_archived_root::<AssertPacket>(&packet_bytes) {
            let packet: AssertPacket = archived.deserialize(&mut rkyv::Infallible).unwrap();
            results.push(packet);
        }
    }
    
    results
}

/// Helper to read a complete packet from FIFO
fn read_packet_from_fifo(fifo_tx: &mut std::collections::VecDeque<u32>) -> Option<Vec<u8>> {
    if fifo_tx.is_empty() {
        return None;
    }
    
    let mut bytes = vec![];
    
    // Read header
    for _ in 0..2 {
        if let Some(word) = fifo_tx.pop_front() {
            bytes.extend_from_slice(&word.to_le_bytes());
        } else {
            return None;
        }
    }
    
    // Parse header to get length
    let header = unsafe { &*(bytes.as_ptr() as *const PacketHeader) };
    let total_words = (header.length as usize + 3) / 4;
    
    // Read remaining words
    for _ in 2..total_words {
        if let Some(word) = fifo_tx.pop_front() {
            bytes.extend_from_slice(&word.to_le_bytes());
        } else {
            return None;
        }
    }
    
    Some(bytes)
}
```

---

## Memory Layout

### Packet Buffer Sizing

When implementing packet reception:

**Host Side**:
- Use dynamic `Vec<u8>` buffers (no fixed size needed)
- Allocate based on `length` field from header

**CPU Side** (bare-metal Rust with limited memory):
- Preallocate fixed-size receive buffer (use static arrays)
- Typical sizes:
  - Minimal: 64 bytes (supports most control packets)
  - Standard: 256 bytes (supports small data transfers)
  - Large: 1024 bytes (supports larger data buffers)

### Example CPU Buffer Management

**Rust Bare-Metal Example**:

```rust
use core::mem::MaybeUninit;

const MAX_PACKET_SIZE: usize = 256;

/// Global receive buffer for packet reception
static mut RX_BUFFER: [MaybeUninit<u8>; MAX_PACKET_SIZE] = 
    [MaybeUninit::uninit(); MAX_PACKET_SIZE];

/// Safely receive a packet into a provided buffer
/// 
/// # Safety
/// Uses unsafe FIFO operations and global mutable state
pub unsafe fn receive_packet_safe(out_buffer: &mut [u8]) -> usize {
    // Receive into temporary buffer first
    let rx_buf = &mut RX_BUFFER;
    let rx_slice = core::slice::from_raw_parts_mut(
        rx_buf.as_mut_ptr() as *mut u8,
        MAX_PACKET_SIZE
    );
    
    let length = receive_packet(rx_slice);
    
    if length == 0 {
        return 0;  // Error
    }
    
    if length > out_buffer.len() {
        // Send error packet to host
        send_error_response(ErrorCode::BufferOverflow);
        return 0;
    }
    
    // Copy to output buffer
    out_buffer[..length].copy_from_slice(&rx_slice[..length]);
    length
}

/// Send an error response to the host
unsafe fn send_error_response(error_code: ErrorCode) {
    let packet = ErrorPacket {
        header: PacketHeader {
            magic: 0x52565043,
            length: 0,
            packet_type: PacketType::Error,
            reserved: 0,
        },
        error_code,
        reserved: [0; 3],
        details: "Buffer overflow".to_string(),
    };
    
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&packet)
        .expect("Error packet serialization failed");
    send_packet(&bytes);
}
```

---

## Usage Examples

### Use Case 1: Automated Testing Framework

```rust
// Test harness that runs CPU test programs and validates results
fn run_cpu_test_suite() {
    let simulator = create_simulator("test_program.elf");
    
    // Send test parameters to CPU
    let params = DataBufferPacket {
        header: create_header(PacketType::DataBuffer),
        buffer_id: 1,
        offset: 0,
        data: vec![0x10, 0x20, 0x30, 0x40],
    };
    send_packet_to_cpu(&mut simulator, &params);
    
    // Run simulation
    run_until_halt(&mut simulator);
    
    // Collect test assertions
    let assertions = collect_packets::<AssertPacket>(&simulator);
    
    // Verify all tests passed
    for assertion in assertions {
        assert!(assertion.passed, 
            "Test {} failed: expected {:08x}, got {:08x}", 
            assertion.test_id, assertion.expected, assertion.actual);
    }
}
```

### Use Case 2: Performance Profiling

```rust
// Profile code execution by reading PC and cycle count periodically
fn profile_execution(simulator: &mut Simulator) {
    let mut profile_data = vec![];
    
    for cycle in 0..10000 {
        simulator.step();
        
        if cycle % 100 == 0 {
            // Request status packet every 100 cycles
            send_packet_to_cpu(simulator, &StatusPacket { /* ... */ });
            
            if let Some(status) = receive_packet::<StatusPacket>(simulator) {
                profile_data.push((status.cycle_count, status.pc));
            }
        }
    }
    
    // Analyze hotspots
    analyze_profile_data(profile_data);
}
```

### Use Case 3: Interactive Debugging

```rust
// Interactive debugger that single-steps and inspects state
fn debug_session(simulator: &mut Simulator) {
    loop {
        // Display current state
        let status = request_and_receive::<StatusPacket>(simulator);
        println!("PC: 0x{:08x}, Cycle: {}", status.pc, status.cycle_count);
        
        // Get user command
        let cmd = read_user_command();
        
        match cmd {
            "step" => simulator.step(),
            "continue" => run_until_breakpoint(simulator),
            "read x1" => {
                let values = read_cpu_registers(simulator, &[1]);
                println!("x1 = 0x{:08x}", values[0]);
            }
            "quit" => break,
            _ => println!("Unknown command"),
        }
    }
}
```

---

## Future Extensions

### Planned Enhancements

1. **Packet Versioning**:
   - Add `version` field to header
   - Support backward compatibility

2. **Compression**:
   - Optional compression for large data packets
   - Useful for memory dumps and trace buffers

3. **Checksums/CRC**:
   - Optional integrity checking beyond rkyv validation
   - Useful for detecting transmission errors in real hardware

4. **Streaming Data**:
   - Multi-packet streaming for large transfers
   - Sequence numbers and reassembly

5. **Interrupt/Event Packets**:
   - Asynchronous event notification from CPU
   - Interrupt acknowledgment

6. **Performance Counters**:
   - Dedicated packet types for performance metrics
   - Cache miss rates, branch predictions, etc.

7. **File Transfer Protocol**:
   - Higher-level protocol for file uploads/downloads
   - Useful for loading data into simulated storage

### Extensibility Guidelines

When adding new packet types:

1. **Assign unique `PacketType` enum value**
2. **Document packet structure** in this specification
3. **Add serialization attributes**: `#[derive(Archive, Deserialize, Serialize)]`
4. **Implement handlers** on both host and CPU sides
5. **Add test cases** for round-trip serialization
6. **Update protocol version** if making breaking changes

---

## Implementation Checklist

### Phase 1: Core Infrastructure
- [ ] Create `riscv_protocol` crate with packet definitions
  - [ ] Support `no_std` for bare-metal CPU usage
  - [ ] Add `rkyv` dependency with appropriate features
- [ ] Implement packet serialization/deserialization helpers
- [ ] Add packet framing and validation utilities
- [ ] Write unit tests for all packet types (host and `no_std` environments)

### Phase 2: Integration
- [ ] Integrate with cpu-sim FIFO infrastructure (host side)
- [ ] Add packet send/receive functions to Simulator
- [ ] Create CPU-side Rust library for bare-metal packet handling
  - [ ] Implement MMIO wrappers for FIFO access
  - [ ] Add packet send/receive functions for `no_std`
  - [ ] Include examples of using shared packet definitions
- [ ] Create example bare-metal Rust programs using the protocol

### Phase 3: Advanced Features
- [ ] Implement register/memory access handlers (both host and CPU)
- [ ] Add debug packet support to cpu-sim
- [ ] Create automated test framework using AssertPacket
- [ ] Write comprehensive integration tests
- [ ] Add cross-compilation support for RISC-V bare-metal targets

### Phase 4: Documentation & Examples
- [ ] Document API usage with Rust examples (host and bare-metal)
- [ ] Create tutorial programs (CPU bare-metal and host)
- [ ] Add protocol conformance tests
- [ ] Performance benchmarking and optimization
- [ ] Document `no_std` considerations and memory requirements

---

## References

- **rkyv Documentation**: https://docs.rs/rkyv/
- **RISC-V Specification**: https://riscv.org/technical/specifications/
- **cpu-sim README**: [../cpu-sim/README.md](../cpu-sim/README.md)
- **Project Architecture**: [../AGENTS.md](../AGENTS.md)

---

## Appendix A: Complete Packet Type Summary

| Packet Type          | Type ID | Direction   | Purpose                    | Min Size |
|----------------------|---------|-------------|----------------------------|----------|
| NopPacket            | 0       | Bidirectional | Keepalive/padding         | 8 bytes  |
| EchoPacket           | 1       | Bidirectional | Connectivity test         | 20 bytes |
| DataU32Packet        | 10      | Bidirectional | Transfer u32 value        | 16 bytes |
| DataI32Packet        | 11      | Bidirectional | Transfer i32 value        | 16 bytes |
| DataBufferPacket     | 12      | Bidirectional | Transfer byte buffer      | 16+ bytes|
| DataStringPacket     | 13      | Bidirectional | Transfer UTF-8 string     | 12+ bytes|
| ResetPacket          | 20      | Host→CPU    | Request CPU reset         | 12 bytes |
| HaltPacket           | 21      | CPU→Host    | Signal termination        | 12 bytes |
| StatusPacket         | 22      | Bidirectional | Status query/response    | 24 bytes |
| RegisterReadPacket   | 30      | Host→CPU    | Read register request     | 8+ bytes |
| RegisterReadResponse | 30      | CPU→Host    | Read register response    | 8+ bytes |
| RegisterWritePacket  | 31      | Host→CPU    | Write registers           | 8+ bytes |
| MemoryReadPacket     | 40      | Host→CPU    | Read memory request       | 16 bytes |
| MemoryReadResponse   | 40      | CPU→Host    | Read memory response      | 12+ bytes|
| MemoryWritePacket    | 41      | Host→CPU    | Write memory              | 12+ bytes|
| AssertPacket         | 50      | CPU→Host    | Test assertion result     | 24+ bytes|
| DebugPacket          | 51      | CPU→Host    | Debug message             | 12+ bytes|
| ErrorPacket          | 255     | Bidirectional | Error notification       | 12+ bytes|

---

## Appendix B: Rust Module Structure

Suggested directory layout for implementation with code sharing between host and CPU:

```
riscv_protocol/                 # Shared packet definitions (no_std compatible)
├── Cargo.toml                  # Features: std (default), no_std
├── src/
│   ├── lib.rs                  # Re-exports and public API
│   ├── header.rs               # PacketHeader, PacketType
│   ├── packets/
│   │   ├── mod.rs              # Packet module exports
│   │   ├── control.rs          # Reset, Halt, Status
│   │   ├── data.rs             # DataU32, DataI32, DataBuffer, DataString
│   │   ├── register.rs         # RegisterRead, RegisterWrite
│   │   ├── memory.rs           # MemoryRead, MemoryWrite
│   │   ├── debug.rs            # Assert, Debug
│   │   └── error.rs            # ErrorPacket, ErrorCode
│   └── validation.rs           # Validation helpers (optional for no_std)
└── tests/
    ├── roundtrip.rs            # Serialization roundtrip tests
    ├── validation.rs           # Validation tests
    └── examples.rs             # Example usage tests

riscv_protocol_host/            # Host-side utilities (depends on riscv_protocol)
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── transport/
│   │   ├── mod.rs
│   │   ├── fifo.rs             # Host FIFO integration
│   │   └── framing.rs          # Packet framing for host
│   └── simulator.rs            # Simulator extensions for packet I/O

riscv_protocol_cpu/             # CPU bare-metal utilities (depends on riscv_protocol)
├── Cargo.toml                  # Target: riscv32i-unknown-none-elf
├── src/
│   ├── lib.rs                  # no_std library
│   ├── mmio.rs                 # FIFO MMIO register wrappers
│   ├── send.rs                 # Packet transmission (unsafe MMIO)
│   ├── receive.rs              # Packet reception (unsafe MMIO)
│   └── handlers.rs             # Example packet handlers
└── examples/
    ├── debug_hello.rs          # Send debug message
    ├── self_test.rs            # Run tests and report with AssertPacket
    └── echo_server.rs          # Respond to host packets
```

### Key Design Points

1. **`riscv_protocol` crate**: Core packet definitions, `no_std` compatible
   - Used by both host and CPU code
   - Conditional compilation for `alloc` features (Vec, String)
   - Supports both `std` and `no_std` environments

2. **`riscv_protocol_host` crate**: Host-specific utilities
   - Depends on `riscv_protocol` with `std` feature
   - Integration with cpu-sim
   - High-level packet send/receive APIs

3. **`riscv_protocol_cpu` crate**: Bare-metal CPU utilities
   - Depends on `riscv_protocol` with `no_std` feature
   - MMIO abstractions for FIFO hardware
   - Example bare-metal programs

### Example `Cargo.toml` for `riscv_protocol`

```toml
[package]
name = "riscv_protocol"
version = "0.1.0"
edition = "2021"

[dependencies]
rkyv = { version = "0.8", default-features = false, features = ["size_32"] }

[features]
default = ["std"]
std = ["rkyv/std", "alloc"]
alloc = ["rkyv/alloc"]

[dev-dependencies]
rkyv = { version = "0.8", features = ["std"] }
```

---

**End of Specification**
