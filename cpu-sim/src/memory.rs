use std::collections::HashMap;
use std::path::Path;

/// Memory model for the RISC-V CPU simulator
/// Uses a byte-addressable HashMap for sparse memory representation
pub struct Memory {
    data: HashMap<u32, u8>,
}

impl Memory {
    /// Create a new, empty memory
    pub fn new() -> Self {
        Memory {
            data: HashMap::new(),
        }
    }

    /// Load an ELF file into memory
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
                        let segment_data = &file_data[offset..offset + file_size];
                        for (i, &byte) in segment_data.iter().enumerate() {
                            self.data.insert(vaddr + i as u32, byte);
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

    /// Read a 32-bit word from memory (little-endian)
    pub fn read_word(&self, addr: u32) -> u32 {
        let b0 = *self.data.get(&addr).unwrap_or(&0) as u32;
        let b1 = *self.data.get(&addr.wrapping_add(1)).unwrap_or(&0) as u32;
        let b2 = *self.data.get(&addr.wrapping_add(2)).unwrap_or(&0) as u32;
        let b3 = *self.data.get(&addr.wrapping_add(3)).unwrap_or(&0) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    /// Write a 32-bit word to memory (little-endian)
    pub fn write_word(&mut self, addr: u32, data: u32) {
        self.data.insert(addr, (data & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(1), ((data >> 8) & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(2), ((data >> 16) & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(3), ((data >> 24) & 0xFF) as u8);
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}
