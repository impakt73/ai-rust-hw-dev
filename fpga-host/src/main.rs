//! FPGA Host Interface
//!
//! This binary provides a host interface for communicating with a RISC-V CPU
//! running on an FPGA over a serial connection. It handles serialized bus
//! requests from the FPGA and routes them to sparse memory.

use clap::Parser;
use riscv_shared::bus::{DRAM_BASE, DRAM_END};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Check if an address is within the DRAM range
fn is_dram_address(addr: u32) -> bool {
    (DRAM_BASE..=DRAM_END).contains(&addr)
}

/// Sparse memory model using a byte-addressable HashMap
///
/// Similar to cpu-sim/src/memory.rs but simplified for fpga-host use case
struct SparseMemory {
    data: HashMap<u32, u8>,
}

impl SparseMemory {
    fn new() -> Self {
        SparseMemory {
            data: HashMap::new(),
        }
    }

    /// Read a single byte from memory
    fn read_byte(&self, addr: u32) -> u8 {
        *self.data.get(&addr).unwrap_or(&0)
    }

    /// Read a 16-bit halfword from memory (little-endian)
    fn read_halfword(&self, addr: u32) -> u16 {
        let b0 = self.read_byte(addr) as u16;
        let b1 = self.read_byte(addr.wrapping_add(1)) as u16;
        b0 | (b1 << 8)
    }

    /// Read a 32-bit word from memory (little-endian)
    fn read_word(&self, addr: u32) -> u32 {
        let b0 = self.read_byte(addr) as u32;
        let b1 = self.read_byte(addr.wrapping_add(1)) as u32;
        let b2 = self.read_byte(addr.wrapping_add(2)) as u32;
        let b3 = self.read_byte(addr.wrapping_add(3)) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    /// Write a single byte to memory
    fn write_byte(&mut self, addr: u32, data: u8) {
        self.data.insert(addr, data);
    }

    /// Write a 16-bit halfword to memory (little-endian)
    fn write_halfword(&mut self, addr: u32, data: u16) {
        self.data.insert(addr, (data & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(1), ((data >> 8) & 0xFF) as u8);
    }

    /// Write a 32-bit word to memory (little-endian)
    fn write_word(&mut self, addr: u32, data: u32) {
        self.data.insert(addr, (data & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(1), ((data >> 8) & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(2), ((data >> 16) & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(3), ((data >> 24) & 0xFF) as u8);
    }
}

/// Load an ELF file into sparse memory
///
/// Similar to cpu-sim/src/lib.rs load_elf function
fn load_elf(memory: &mut SparseMemory, path: &PathBuf) -> Result<u32, Box<dyn std::error::Error>> {
    let file_data = std::fs::read(path)?;
    let elf_file = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(&file_data)?;

    let mut entry_point = 0u32;

    // Get the entry point
    if let Ok(header) = elf_file.ehdr.e_entry.try_into() {
        entry_point = header;
    }

    // Load program headers (segments)
    if let Some(phdrs) = elf_file.segments() {
        for phdr in phdrs.iter() {
            // Only load LOAD segments
            if phdr.p_type == elf::abi::PT_LOAD {
                let vaddr = phdr.p_vaddr as u32;
                let file_size = phdr.p_filesz as usize;
                let offset = phdr.p_offset as usize;

                if file_size > 0 {
                    // Validate that the segment lies within the file data
                    let end = match offset.checked_add(file_size) {
                        Some(end) if end <= file_data.len() => end,
                        _ => {
                            return Err(Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!(
                                    "ELF segment out of bounds: offset=0x{:x}, size=0x{:x}, file_len=0x{:x}",
                                    offset,
                                    file_size,
                                    file_data.len()
                                ),
                            )));
                        }
                    };

                    let segment_data = &file_data[offset..end];
                    // Write to memory byte by byte
                    for (i, &byte) in segment_data.iter().enumerate() {
                        memory.write_byte(vaddr.wrapping_add(i as u32), byte);
                    }
                    log::info!(
                        "Loaded segment: vaddr=0x{:08x}, size=0x{:x} bytes",
                        vaddr,
                        file_size
                    );
                }
            }
        }
    }

    Ok(entry_point)
}

/// Host bus interface state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostBusState {
    /// Waiting for header byte from FPGA
    WaitHeader,
    /// Receiving address bytes (4 bytes, little-endian)
    RxAddr { byte_idx: u8 },
    /// Receiving write data bytes (1-4 bytes based on size)
    RxWdata { byte_idx: u8 },
    /// Sending write acknowledgement
    TxAck,
    /// Sending read data bytes (1-4 bytes based on size)
    TxRdata { byte_idx: u8 },
}

/// Captured transaction from host bus interface
#[derive(Debug, Clone, Default)]
struct HostBusTransaction {
    /// Write enable (true = write, false = read)
    we: bool,
    /// Access size (0 = byte, 1 = halfword, 2 = word)
    size: u8,
    /// Address (accumulated little-endian)
    addr: u32,
    /// Write data (accumulated little-endian, only valid for writes)
    wdata: u32,
    /// Read data to send back (only valid for reads)
    rdata: u32,
}

/// Get the size name for logging
fn size_name(size: u8) -> &'static str {
    match size {
        0 => "byte",
        1 => "halfword",
        _ => "word",
    }
}

/// Get the number of bytes for a given size code
fn bytes_for_size(size: u8) -> u8 {
    match size {
        0 => 1,
        1 => 2,
        _ => 4,
    }
}

#[derive(Parser)]
#[command(author, version, about = "FPGA Host Interface for RISC-V CPU")]
struct Args {
    /// Path to the serial device (e.g., /dev/ttyUSB0)
    #[arg(short, long)]
    serial: PathBuf,

    /// Baud rate for serial communication
    #[arg(short, long, default_value_t = 115200)]
    baud: u32,

    /// Path to the RISC-V ELF executable to load
    #[arg(short, long)]
    elf: PathBuf,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();

    // Initialize logger
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    log::info!("FPGA Host Interface");
    log::info!("Serial device: {}", args.serial.display());
    log::info!("Baud rate: {}", args.baud);
    log::info!("ELF file: {}", args.elf.display());

    // Set up Ctrl+C handler for graceful exit
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        log::info!("Received Ctrl+C, shutting down...");
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    // Create sparse memory
    let mut memory = SparseMemory::new();

    // Load ELF file
    match load_elf(&mut memory, &args.elf) {
        Ok(entry_point) => {
            log::info!(
                "ELF loaded successfully, entry point: 0x{:08x}",
                entry_point
            );
        }
        Err(e) => {
            log::error!("Failed to load ELF file: {}", e);
            std::process::exit(1);
        }
    }

    // Open serial port
    let port = serialport::new(args.serial.to_string_lossy(), args.baud)
        .timeout(Duration::from_millis(100))
        .open();

    let mut port = match port {
        Ok(p) => {
            log::info!("Serial port opened successfully");
            p
        }
        Err(e) => {
            log::error!("Failed to open serial port: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize state machine
    let mut state = HostBusState::WaitHeader;
    let mut txn = HostBusTransaction::default();
    let mut request_count: u64 = 0;

    log::info!("Starting bus request loop (press Ctrl+C to exit)...");

    // Main loop processing serial bus requests
    while running.load(Ordering::SeqCst) {
        let mut byte_buf = [0u8; 1];

        match state {
            HostBusState::WaitHeader => {
                // Try to read header byte
                match port.read(&mut byte_buf) {
                    Ok(1) => {
                        let header = byte_buf[0];
                        // Parse header: {4'b0000, size[1:0], 1'b0, we}
                        txn.we = (header & 0x01) != 0;
                        txn.size = (header >> 2) & 0x03;
                        txn.addr = 0;
                        txn.wdata = 0;
                        txn.rdata = 0;

                        log::debug!(
                            "Received header: 0x{:02x} (we={}, size={})",
                            header,
                            txn.we,
                            size_name(txn.size)
                        );
                        state = HostBusState::RxAddr { byte_idx: 0 };
                    }
                    Ok(0) | Err(_) => {
                        // No data available or timeout, continue waiting
                        continue;
                    }
                    Ok(_) => unreachable!(),
                }
            }

            HostBusState::RxAddr { byte_idx } => {
                match port.read(&mut byte_buf) {
                    Ok(1) => {
                        let byte = byte_buf[0] as u32;
                        // Accumulate address (little-endian)
                        txn.addr |= byte << (byte_idx * 8);

                        if byte_idx == 3 {
                            // Address complete
                            if txn.we {
                                // Write: continue receiving write data
                                state = HostBusState::RxWdata { byte_idx: 0 };
                            } else {
                                // Read: perform read and start sending response
                                perform_read(&memory, &mut txn);
                                request_count += 1;

                                // Log the read request
                                let is_dram = is_dram_address(txn.addr);
                                log::info!(
                                    "[{}] READ {} @ 0x{:08x} => 0x{:0width$x}{}",
                                    request_count,
                                    size_name(txn.size),
                                    txn.addr,
                                    txn.rdata,
                                    if is_dram {
                                        ""
                                    } else {
                                        " (non-DRAM, returned 0)"
                                    },
                                    width = (bytes_for_size(txn.size) * 2) as usize
                                );

                                state = HostBusState::TxRdata { byte_idx: 0 };
                            }
                        } else {
                            state = HostBusState::RxAddr {
                                byte_idx: byte_idx + 1,
                            };
                        }
                    }
                    Ok(0) | Err(_) => {
                        // No data, continue waiting
                        continue;
                    }
                    Ok(_) => unreachable!(),
                }
            }

            HostBusState::RxWdata { byte_idx } => {
                match port.read(&mut byte_buf) {
                    Ok(1) => {
                        let byte = byte_buf[0] as u32;
                        // Accumulate write data (little-endian)
                        txn.wdata |= byte << (byte_idx * 8);

                        let bytes_needed = bytes_for_size(txn.size);

                        if byte_idx + 1 >= bytes_needed {
                            // Write data complete - perform write and send ack
                            let is_dram = is_dram_address(txn.addr);
                            perform_write(&mut memory, &txn);
                            request_count += 1;

                            // Log the write request
                            log::info!(
                                "[{}] WRITE {} @ 0x{:08x} <= 0x{:0width$x}{}",
                                request_count,
                                size_name(txn.size),
                                txn.addr,
                                txn.wdata,
                                if is_dram { "" } else { " (non-DRAM, dropped)" },
                                width = (bytes_needed * 2) as usize
                            );

                            state = HostBusState::TxAck;
                        } else {
                            state = HostBusState::RxWdata {
                                byte_idx: byte_idx + 1,
                            };
                        }
                    }
                    Ok(0) | Err(_) => {
                        // No data, continue waiting
                        continue;
                    }
                    Ok(_) => unreachable!(),
                }
            }

            HostBusState::TxAck => {
                // Send acknowledgement byte (0x00)
                let ack_buf = [0x00u8];
                match port.write(&ack_buf) {
                    Ok(1) => {
                        log::debug!("Sent ACK");
                        state = HostBusState::WaitHeader;
                    }
                    Ok(0) | Err(_) => {
                        // Write failed, retry
                        continue;
                    }
                    Ok(_) => unreachable!(),
                }
            }

            HostBusState::TxRdata { byte_idx } => {
                // Send read data byte (little-endian)
                let byte = ((txn.rdata >> (byte_idx * 8)) & 0xFF) as u8;
                let data_buf = [byte];

                match port.write(&data_buf) {
                    Ok(1) => {
                        let bytes_needed = bytes_for_size(txn.size);

                        if byte_idx + 1 >= bytes_needed {
                            log::debug!("Sent all read data bytes");
                            state = HostBusState::WaitHeader;
                        } else {
                            state = HostBusState::TxRdata {
                                byte_idx: byte_idx + 1,
                            };
                        }
                    }
                    Ok(0) | Err(_) => {
                        // Write failed, retry
                        continue;
                    }
                    Ok(_) => unreachable!(),
                }
            }
        }
    }

    log::info!("Exiting. Processed {} bus requests.", request_count);
}

/// Perform a read operation
/// For DRAM addresses, read from sparse memory
/// For other addresses, return 0
fn perform_read(memory: &SparseMemory, txn: &mut HostBusTransaction) {
    if is_dram_address(txn.addr) {
        txn.rdata = match txn.size {
            0 => memory.read_byte(txn.addr) as u32,
            1 => memory.read_halfword(txn.addr) as u32,
            _ => memory.read_word(txn.addr),
        };
    } else {
        // Non-DRAM reads return 0
        txn.rdata = 0;
    }
}

/// Perform a write operation
/// For DRAM addresses, write to sparse memory
/// For other addresses, drop the write (do nothing)
fn perform_write(memory: &mut SparseMemory, txn: &HostBusTransaction) {
    if is_dram_address(txn.addr) {
        match txn.size {
            0 => memory.write_byte(txn.addr, txn.wdata as u8),
            1 => memory.write_halfword(txn.addr, txn.wdata as u16),
            _ => memory.write_word(txn.addr, txn.wdata),
        }
    }
    // Non-DRAM writes are silently dropped
}
