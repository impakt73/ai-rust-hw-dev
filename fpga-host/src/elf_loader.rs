//! ELF file loader
//!
//! This module provides functionality to load ELF executables into sparse memory.

use device_runtime::memory::SparseMemory;
use std::path::Path;

/// Errors that can occur during ELF loading
#[derive(Debug)]
pub enum ElfError {
    /// I/O error reading the file
    IoError(std::io::Error),
    /// Error parsing the ELF file
    ParseError(String),
    /// Segment extends beyond file bounds
    SegmentOutOfBounds {
        offset: usize,
        size: usize,
        file_len: usize,
    },
}

impl std::fmt::Display for ElfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElfError::IoError(e) => write!(f, "I/O error: {}", e),
            ElfError::ParseError(s) => write!(f, "Parse error: {}", s),
            ElfError::SegmentOutOfBounds {
                offset,
                size,
                file_len,
            } => {
                write!(
                    f,
                    "Segment out of bounds: offset=0x{:x}, size=0x{:x}, file_len=0x{:x}",
                    offset, size, file_len
                )
            }
        }
    }
}

impl std::error::Error for ElfError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ElfError::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ElfError {
    fn from(e: std::io::Error) -> Self {
        ElfError::IoError(e)
    }
}

/// Load an ELF file into sparse memory
///
/// Returns the entry point address on success
pub fn load_elf(memory: &mut SparseMemory, path: &Path) -> Result<u32, ElfError> {
    let file_data = std::fs::read(path)?;
    let elf_file = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(&file_data)
        .map_err(|e| ElfError::ParseError(e.to_string()))?;

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
                            return Err(ElfError::SegmentOutOfBounds {
                                offset,
                                size: file_size,
                                file_len: file_data.len(),
                            });
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
