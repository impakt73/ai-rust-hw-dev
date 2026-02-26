# Host Bus Burst Transfer Protocol Upgrade Plan

## 1. Goal and Scope

### 1.1 Goal

Eliminate the current serialized single-request/single-response bottleneck on the host↔FPGA link by introducing **native burst transfers up to 64K words** (65,536 x 32-bit words per burst command) in the host bus packet protocol.

### 1.2 Required Constraints

- **Do not preserve protocol backward compatibility.** This is a clean protocol replacement.
- **Do not change RTL system bus modules/arbiter/memory map.** Burst behavior is implemented as a loop inside host bus interface logic.
- **Modify only:**
  - `rtl/io/host_bus_interface.sv`
  - associated internal host bus packet modules (`rtl/io/host_bus_rx.sv`, `rtl/io/host_bus_tx.sv`, and any new helper module under `rtl/io/`)
  - host-side Rust communication logic (`host-bus-handler` and `device-runtime`, with most call-site impact isolated in `DeviceRuntime::read_memory_region` / `write_memory_region`)

---

## 2. New Packet Protocol (v2, non-backward-compatible)

### 2.1 Design Principles

1. **Descriptor-first framing:** command always starts with a fixed-size descriptor, making decode simple in RTL and Rust.
2. **Word-native transfers:** burst length is encoded in words to align with system bus datapath.
3. **Address stride control:** source and destination increment controls are explicit and independent.
4. **Direction-neutral semantics:** protocol models a transfer from `src` endpoint to `dst` endpoint.

### 2.2 Packet Types

- `0x1` = `CMD_BURST` (Host → FPGA): submit burst descriptor + write payload (if needed)
- `0x2` = `RSP_BURST_STATUS` (FPGA → Host): completion status and metadata
- `0x3` = `RSP_BURST_READ_DATA` (FPGA → Host): stream of read data words for read-direction bursts
- `0xF` = `RSP_ERROR` (FPGA → Host): malformed packet / unsupported descriptor / runtime error

### 2.3 Fixed Descriptor Layout (`CMD_BURST`)

All multi-byte fields are little-endian.

| Byte(s) | Field | Bits | Description |
|---|---|---:|---|
| 0 | `opcode` | 7:0 | `0x1` for burst command |
| 1 | `flags` | 7:0 | Bit0 `src_inc`, Bit1 `dst_inc`, Bit2 `is_write_to_bus`, Bit3 `reserved`, Bits7:4 reserved |
| 2..3 | `word_count_minus1` | 15:0 | Encodes 1..65536 words (`actual = value + 1`; `0 => 1 word`, `65535 => 65536 words`) |
| 4..7 | `src_addr` | 31:0 | Source address (meaning depends on endpoint role) |
| 8..11 | `dst_addr` | 31:0 | Destination address (meaning depends on endpoint role) |

Payload immediately follows descriptor:
- If `is_write_to_bus=1` (Host writes into bus space): host sends `word_count * 4` bytes after descriptor.
- If `is_write_to_bus=0` (Host reads from bus space): no payload in command packet.

### 2.4 Endpoint Semantics and Increment Controls

- One endpoint is always **stream side** (serial RX/TX side), the other is **bus side** (`host_bus_*` master interface).
- `src_inc=0` means reuse same source address each word.
- `dst_inc=0` means reuse same destination address each word.

Expected usage:
- **Read burst (bus → host):** typically `src_inc=1`, `dst_inc=1` (normal memory read) or `src_inc=0` for FIFO-like MMIO pop register.
- **Write burst (host → bus):** typically `src_inc=1`, `dst_inc=1` (normal memory write) or `dst_inc=0` for FIFO-like MMIO push register.

Notes:
- For stream endpoint, “address increment” is interpreted as stream index advance behavior in RTL loop bookkeeping.
- Protocol keeps both increment flags explicit for symmetry and future-proofing.

### 2.5 Completion and Error Response Formats

### `RSP_BURST_STATUS` (`opcode=0x2`)

| Byte(s) | Field | Description |
|---|---|---|
| 0 | `opcode` | `0x2` |
| 1 | `status` | `0=OK`, `1=ERR_MALFORMED` (invalid packet format/flags), `2=ERR_BUSY` (command while active burst exists), `3=ERR_BUS` (bus transaction failed/timed out), `4=ERR_INTERNAL` (unexpected FSM/internal consistency fault) |
| 2..3 | `words_completed_minus1` | Number of words completed minus 1 before finishing/error (`0` means 1 word completed) |

### `RSP_BURST_READ_DATA` (`opcode=0x3`)

| Byte(s) | Field | Description |
|---|---|---|
| 0 | `opcode` | `0x3` |
| 1..2 | `word_count_minus1` | Number of words in this data response, minus 1 (`0 => 1 word`) |
| 3..N | `data` | `word_count * 4` bytes, little-endian words |

For this implementation, emit one `RSP_BURST_READ_DATA` per command (entire burst). Future chunking can be added later if needed.

---

## 3. RTL Implementation Plan

### 3.1 Architectural Change

Implement a burst execution FSM in `host_bus_interface.sv` that:
1. accepts one `CMD_BURST`,
2. executes a loop of per-word bus operations over existing `host_bus_*` master handshake,
3. uses RX/TX packet engines for payload ingest/egress,
4. emits final status response.

No changes to system bus RTL interfaces or transaction semantics.

### 3.2 RTL Module Responsibilities

### `host_bus_rx.sv`

- Replace old header parser with v2 command parser for:
  - fixed 12-byte descriptor
  - optional command payload stream (`word_count * 4`)
- Output structured burst command fields:
  - `cmd_valid`, `cmd_ready`
  - `cmd_is_write_to_bus`, `cmd_src_inc`, `cmd_dst_inc`
  - `cmd_word_count` (17-bit internal **actual** word count: `1..65536`; 17 bits are intentional so value `65536` is representable after expanding 16-bit `word_count_minus1`)
  - `cmd_src_addr`, `cmd_dst_addr`
- For host→bus writes, expose streaming payload word interface to `host_bus_interface`:
  - `payload_word_valid`, `payload_word_ready`, `payload_word_data`
- Enforce single in-flight command buffering.

### `host_bus_tx.sv`

- Add support for v2 response packet emission:
  - `RSP_BURST_READ_DATA` with variable payload length (up to 64K words)
  - `RSP_BURST_STATUS`
  - `RSP_ERROR`
- Provide input handshake from interface FSM:
  - descriptor/metadata channel for response type and counts
  - streaming data channel for read payload words
- Keep transmit path fully backpressure-safe on `tx_ready`.

### `host_bus_interface.sv`

- Add burst control registers:
  - active descriptor fields
  - current source/destination addresses
  - words remaining / words completed counters
  - error/status latch
- Add execution FSM (high-level):
  1. `S_IDLE`: wait for `cmd_valid`
  2. `S_VALIDATE`: check descriptor legality, reject invalid
  3. `S_WRITE_FETCH_WORD` / `S_WRITE_BUS_REQ` / `S_WRITE_BUS_WAIT`
  4. `S_READ_BUS_REQ` / `S_READ_BUS_WAIT` / `S_READ_PUSH_WORD`
  5. `S_SEND_READ_DATA_PKT`
  6. `S_SEND_STATUS_PKT`
  7. `S_ERROR_PKT`
- Loop behavior:
  - For write burst: read each word from RX payload stream, issue bus write, wait `host_bus_ready`, update addresses per inc flags.
  - For read burst: issue bus read, wait `host_bus_ready`, push `host_bus_rdata` to TX read-data stream, update addresses per inc flags.
- Bus size hard-wired to word (`2'b10`, existing host bus encoding for 32-bit/word access) for burst path.
- Busy rule: while one command is active, reject/ERR_BUSY any new command.

### 3.3 Address Increment Rules in FSM

- `next_src_addr = src_inc ? src_addr + 4 : src_addr`
- `next_dst_addr = dst_inc ? dst_addr + 4 : dst_addr`
- On each completed word transfer, update both based on direction and descriptor flags.
- For fixed-address FIFO interaction, one side remains constant across the burst.

### 3.4 RTL Validation Tasks

Update/add focused tests in `testbench/tests/host_bus_interface_test.rs` (and any RX/TX module tests) for:
1. Burst write incrementing dst (`dst_inc=1`)
2. Burst write fixed dst (`dst_inc=0`, FIFO-like)
3. Burst read incrementing src (`src_inc=1`)
4. Burst read fixed src (`src_inc=0`, FIFO-like)
5. Max length burst (`65536` words) counter rollover correctness
6. Backpressure stress on both RX and TX interfaces
7. Malformed descriptor / illegal flags / zero length encoding rejection
8. Busy handling with concurrent command attempt

---

## 4. Rust Host-Side Implementation Plan

### 4.1 `host-bus-handler` crate changes

Primary protocol rewrite location.

- Replace existing packet type parsing/serialization with v2 burst protocol.
- Add new API surface (minimal and explicit):
  - `send_burst_write(addr, words: &[u32], dst_inc: bool)` (`words.len()` validated to `1..=65536`)
  - `send_burst_read(addr, num_words: NonZeroU32, src_inc: bool) -> Result<Vec<u32>, HandlerError>` (`num_words.get() <= 65536` validated)
  - internal packet encode/decode helpers for new opcodes.
- Keep low-level byte transfer API (`transfer_rx_byte`, `transfer_tx_byte`) but rewire FSMs to descriptor+stream model.
- Preserve robust partial-byte/partial-packet handling under serial fragmentation.

### 4.2 `device-runtime` changes (mostly isolated)

Main behavior change should be in low-level region APIs in `device-runtime/src/lib.rs`:

- `write_memory_region`:
  - batch contiguous aligned chunks into burst writes
  - convert bytes to little-endian words
  - use burst write call for large regions, fallback path for tiny/unaligned tails
- `read_memory_region`:
  - issue burst reads for large aligned spans
  - re-pack returned words to requested byte length
  - fallback to existing granular logic for unaligned heads/tails

Keep higher-level call sites unchanged (`load_program`, ELF loaders, tests), so integration impact stays localized.

### 4.3 Runtime event model adjustments

- Add/adjust bus events as needed to represent burst completion/errors without flooding per-word events.
- Ensure timeout handling treats an entire burst command as one pending host request lifecycle.

---

## 5. Migration Sequence (Implementation Order)

1. **Protocol constants/types (Rust + RTL docs/comments)**  
   Define v2 opcodes, flags, descriptor structs/packed fields.
2. **RX parser update (`host_bus_rx.sv`)**  
   Descriptor + optional payload ingest with backpressure.
3. **TX serializer update (`host_bus_tx.sv`)**  
   Read-data and status/error packet emission.
4. **Core burst FSM in `host_bus_interface.sv`**  
   Execute bus loop using current master interface only.
5. **Rust handler protocol rewrite (`host-bus-handler`)**  
   Keep public surface small, ensure deterministic state transitions.
6. **`device-runtime` region API migration**  
   Move bulk reads/writes to burst calls; keep external trait usage stable.
7. **Tests and regression pass**  
   Focused RTL + Rust protocol + device-runtime region tests.

---

## 6. Verification Strategy

### 6.1 RTL checks

- `find rtl -name '*.sv' -exec verilator --lint-only {} +`
- `(cd fpga && make)` (synthesis gate after RTL change)
- Targeted host bus tests in `testbench/tests/host_bus_interface_test.rs`

### 6.2 Rust checks

- `cargo fmt`
- `cargo clippy --fix --allow-dirty`
- `cargo clippy -- -D warnings`
- targeted tests:
  - `cargo test -p host-bus-handler`
  - `cargo test -p device-runtime test_packet_protocol`
  - `cargo test -p device-runtime test_memory`

---

## 7. Risks and Mitigations

1. **Large burst buffering pressure**
   - Mitigation: stream payload/data word-by-word; avoid full-burst buffering in RTL.
2. **Protocol deadlock under backpressure**
   - Mitigation: formalize ready/valid rules per channel and test stalled TX/RX scenarios.
3. **Address increment semantic ambiguity**
   - Mitigation: enforce explicit descriptor flags and document source/destination role mapping.
4. **Timeout behavior for long bursts**
   - Mitigation: scale timeout policy with burst size on Rust side (or heartbeat polling).

---

## 8. Definition of Done

- New packet protocol v2 implemented (non-backward-compatible) across RTL host bus modules and Rust host communication stack.
- Burst commands support 1..65536 words.
- Both `src_inc=0` and `dst_inc=0` modes validated for FIFO-style interactions.
- No RTL system bus module changes.
- `device-runtime` bulk region read/write paths use burst transfers by default for eligible spans.
- Lint/tests/synthesis checks pass for touched components.
