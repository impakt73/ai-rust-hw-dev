use std::collections::HashMap;
use std::path::Path;

/// DRAM model for the RISC-V CPU simulator
/// Uses a byte-addressable HashMap for sparse memory representation
pub struct Dram {
    data: HashMap<u32, u8>,
}

impl Dram {
    /// Create a new, empty DRAM
    pub fn new() -> Self {
        Dram {
            data: HashMap::new(),
        }
    }

    /// Load an ELF file into DRAM
    /// Returns the entry point address
    pub fn load_elf(&mut self, path: &Path) -> Result<u32, Box<dyn std::error::Error>> {
        let file_data = std::fs::read(path)?;
        let elf_file = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(&file_data)?;

        let mut entry_point = 0;

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
                        // Validate that the segment lies within the file data to avoid panics
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
                        for (i, &byte) in segment_data.iter().enumerate() {
                            self.data.insert(vaddr.wrapping_add(i as u32), byte);
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

        log::info!("ELF loaded with entry point: 0x{:08x}", entry_point);
        Ok(entry_point)
    }

    /// Read a 32-bit word from DRAM (little-endian)
    /// The address is aligned to a word boundary before reading
    pub fn read_word(&self, addr: u32) -> u32 {
        // Align address to word boundary
        let aligned_addr = addr & !3;
        let b0 = *self.data.get(&aligned_addr).unwrap_or(&0) as u32;
        let b1 = *self.data.get(&aligned_addr.wrapping_add(1)).unwrap_or(&0) as u32;
        let b2 = *self.data.get(&aligned_addr.wrapping_add(2)).unwrap_or(&0) as u32;
        let b3 = *self.data.get(&aligned_addr.wrapping_add(3)).unwrap_or(&0) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    /// Write a 32-bit word to DRAM (little-endian)
    pub fn write_word(&mut self, addr: u32, data: u32) {
        self.data.insert(addr, (data & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(1), ((data >> 8) & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(2), ((data >> 16) & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(3), ((data >> 24) & 0xFF) as u8);
    }

    /// Write a 32-bit word to DRAM with byte enables (little-endian)
    /// Only writes bytes where the corresponding bit in byte_enable is set
    /// The address is the unaligned address; we align it and use byte_enable to select bytes
    pub fn write_word_with_be(&mut self, addr: u32, data: u32, byte_enable: u8) {
        // Align address to word boundary
        let aligned_addr = addr & !3;

        // The RTL replicates bytes for SB and halfwords for SH across all positions.
        // For SB: all 4 bytes contain the same value
        // For SH: bytes 0-1 contain the halfword, bytes 2-3 contain the same halfword
        // For SW: each byte is different
        //
        // Extract bytes from their respective positions in the data word
        if byte_enable & 0b0001 != 0 {
            self.data.insert(aligned_addr, (data & 0xFF) as u8);
        }
        if byte_enable & 0b0010 != 0 {
            self.data
                .insert(aligned_addr.wrapping_add(1), ((data >> 8) & 0xFF) as u8);
        }
        if byte_enable & 0b0100 != 0 {
            self.data
                .insert(aligned_addr.wrapping_add(2), ((data >> 16) & 0xFF) as u8);
        }
        if byte_enable & 0b1000 != 0 {
            self.data
                .insert(aligned_addr.wrapping_add(3), ((data >> 24) & 0xFF) as u8);
        }
    }
}

impl Default for Dram {
    fn default() -> Self {
        Self::new()
    }
}
