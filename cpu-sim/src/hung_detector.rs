/// Hung State Detector
///
/// Provides multiple safety mechanisms to detect when the CPU simulator
/// has entered a hung state (infinite loop, stuck FSM, or invalid PC).
/// This allows tests to use very high max_cycles limits while still
/// catching problematic conditions quickly.
use std::collections::VecDeque;

/// Represents a contiguous range of valid instruction memory
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRange {
    pub start: u32,
    pub end: u32, // exclusive
}

impl MemoryRange {
    /// Check if an address is within this range
    pub fn contains(&self, addr: u32) -> bool {
        addr >= self.start && addr < self.end
    }

    /// Check if this range overlaps with another range
    pub fn overlaps(&self, other: &MemoryRange) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// Check if this range is adjacent to another range
    pub fn adjacent_to(&self, other: &MemoryRange) -> bool {
        self.end == other.start || other.end == self.start
    }

    /// Merge two overlapping or adjacent ranges
    pub fn merge(&self, other: &MemoryRange) -> MemoryRange {
        MemoryRange {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

/// Configuration for the hung state detector
#[derive(Debug, Clone)]
pub struct HungDetectorConfig {
    /// Size of the PC history window (number of PCs to track)
    pub pc_history_size: usize,

    /// Number of identical consecutive PCs before declaring a hang
    pub pc_stuck_threshold: u32,

    /// Maximum number of cycles an instruction can take before declaring a hang
    /// This catches cases where FSM gets stuck and never completes an instruction
    pub max_cycles_per_instruction: u64,

    /// Enable PC loop detection
    pub enable_pc_loop_detection: bool,

    /// Enable out-of-bounds PC detection
    pub enable_pc_bounds_detection: bool,

    /// Enable detection of instructions that take too many cycles
    pub enable_long_instruction_detection: bool,
}

impl Default for HungDetectorConfig {
    fn default() -> Self {
        Self {
            pc_history_size: 100,
            pc_stuck_threshold: 50,
            max_cycles_per_instruction: 10000,
            enable_pc_loop_detection: true,
            enable_pc_bounds_detection: true,
            enable_long_instruction_detection: true,
        }
    }
}

/// Errors that can be detected by the hung state detector
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HungStateError {
    /// PC hasn't changed for N consecutive instructions
    PcStuck {
        pc: u32,
        instruction: u32,
        count: u32,
        history: Vec<u32>,
    },

    /// PC has jumped outside valid instruction memory
    PcOutOfBounds {
        pc: u32,
        valid_ranges: Vec<MemoryRange>,
    },

    /// Current instruction has taken too many cycles to complete
    LongInstruction {
        pc: u32,
        instruction: u32,
        cycle_count: u64,
        fsm_state: u8,
    },
}

impl std::fmt::Display for HungStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HungStateError::PcStuck {
                pc,
                instruction,
                count,
                history,
            } => {
                write!(
                    f,
                    "CPU hung: PC stuck at 0x{:08x} (instruction: 0x{:08x}) for {} consecutive executions.\n\
                     Recent PC history (last {} entries): {:?}",
                    pc,
                    instruction,
                    count,
                    history.len(),
                    history.iter().map(|p| format!("0x{:08x}", p)).collect::<Vec<_>>()
                )
            }
            HungStateError::PcOutOfBounds {
                pc,
                valid_ranges,
            } => {
                write!(f, "CPU hung: PC 0x{:08x} is outside valid instruction memory. Valid ranges: ", pc)?;
                for (i, range) in valid_ranges.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "[0x{:08x}, 0x{:08x})", range.start, range.end)?;
                }
                Ok(())
            }
            HungStateError::LongInstruction {
                pc,
                instruction,
                cycle_count,
                fsm_state,
            } => {
                write!(
                    f,
                    "CPU hung: Instruction at PC 0x{:08x} (0x{:08x}) has taken {} cycles without completing. FSM state: {} ({})",
                    pc, instruction, cycle_count, fsm_state_name(*fsm_state), fsm_state
                )
            }
        }
    }
}

/// Helper function to decode FSM state value to human-readable string
fn fsm_state_name(state: u8) -> &'static str {
    match state {
        0 => "IDLE",
        1 => "FETCH",
        2 => "DECODE",
        3 => "EXECUTE",
        4 => "BRANCH_RESOLVE",
        5 => "MEM_READ",
        6 => "MEM_WRITE",
        7 => "CSR_READ",
        8 => "CSR_WRITE",
        9 => "MUL_DIV",
        10 => "HALT",
        _ => "UNKNOWN",
    }
}

impl std::error::Error for HungStateError {}

/// Hung state detector implementation
pub struct HungDetector {
    config: HungDetectorConfig,

    // PC loop detection state
    pc_history: VecDeque<u32>,
    last_pc: Option<u32>,
    pc_stuck_count: u32,

    // Long instruction detection
    current_instruction_pc: Option<u32>,
    current_instruction_word: u32,
    current_instruction_start_cycle: u64,

    // Multiple non-contiguous valid PC ranges
    valid_pc_ranges: Vec<MemoryRange>,
    
    // Track if we've completed at least one valid instruction from a valid PC
    // Used to avoid false positives during simulator initialization
    has_executed_valid_instruction: bool,
}

impl HungDetector {
    /// Create a new hung state detector with the given configuration
    pub fn new(config: HungDetectorConfig) -> Self {
        Self {
            pc_history: VecDeque::with_capacity(config.pc_history_size),
            config,
            last_pc: None,
            pc_stuck_count: 0,
            current_instruction_pc: None,
            current_instruction_word: 0,
            current_instruction_start_cycle: 0,
            valid_pc_ranges: Vec::new(),
            has_executed_valid_instruction: false,
        }
    }

    /// Create a new hung state detector with default configuration
    pub fn new_default() -> Self {
        Self::new(HungDetectorConfig::default())
    }

    /// Add or update a valid PC range for bounds checking
    ///
    /// If the new range overlaps or is adjacent to existing ranges, they will be merged.
    /// This allows building up non-contiguous executable regions properly.
    ///
    /// # Arguments
    /// * `start` - Start address of valid instruction memory (inclusive)
    /// * `end` - End address of valid instruction memory (exclusive)
    /// * `is_instructions` - If true, marks this range as containing instructions;
    ///                       if false, removes this range from valid PC ranges
    pub fn update_pc_range(&mut self, start: u32, end: u32, is_instructions: bool) {
        if !is_instructions {
            // Remove or split existing ranges that overlap with this data region
            let data_range = MemoryRange { start, end };
            let mut new_ranges = Vec::new();
            
            for range in &self.valid_pc_ranges {
                if !range.overlaps(&data_range) {
                    // No overlap, keep the range
                    new_ranges.push(*range);
                } else {
                    // Overlaps - need to split or remove
                    if range.start < data_range.start {
                        // Keep the part before the data region
                        new_ranges.push(MemoryRange {
                            start: range.start,
                            end: range.end.min(data_range.start),
                        });
                    }
                    if range.end > data_range.end {
                        // Keep the part after the data region
                        new_ranges.push(MemoryRange {
                            start: range.start.max(data_range.end),
                            end: range.end,
                        });
                    }
                }
            }
            self.valid_pc_ranges = new_ranges;
        } else {
            // Add instruction range, merging with overlapping/adjacent ranges
            let new_range = MemoryRange { start, end };
            let mut merged_range = new_range;
            let mut remaining_ranges = Vec::new();

            for range in &self.valid_pc_ranges {
                if range.overlaps(&merged_range) || range.adjacent_to(&merged_range) {
                    // Merge this range into the new range
                    merged_range = merged_range.merge(range);
                } else {
                    // Keep this range separate
                    remaining_ranges.push(*range);
                }
            }

            remaining_ranges.push(merged_range);
            self.valid_pc_ranges = remaining_ranges;
        }
    }

    /// Get all valid PC ranges
    pub fn get_valid_pc_ranges(&self) -> &[MemoryRange] {
        &self.valid_pc_ranges
    }

    /// Check if a PC is within any valid range
    fn is_pc_valid(&self, pc: u32) -> bool {
        self.valid_pc_ranges.iter().any(|range| range.contains(pc))
    }

    /// Check for hung state on every cycle
    ///
    /// This detects cases where the FSM gets stuck and never completes an instruction,
    /// checks if PC is within valid instruction memory bounds, and tracks PC loops when
    /// instructions complete.
    ///
    /// # Arguments
    /// * `cycle_count` - Total simulation cycle count
    /// * `pc` - Current program counter
    /// * `instruction` - Current instruction word
    /// * `fsm_state` - Current FSM state (4-bit value)
    /// * `instruction_complete` - True if an instruction completed this cycle
    ///
    /// # Returns
    /// * `Ok(())` if no hang detected
    /// * `Err(HungStateError)` if a hang condition was detected
    pub fn check_cycle(
        &mut self,
        cycle_count: u64,
        pc: u32,
        instruction: u32,
        fsm_state: u8,
        instruction_complete: bool,
    ) -> Result<(), HungStateError> {
        // Per-instruction checks when instruction completes
        if instruction_complete {
            // Mark that we've completed at least one instruction
            // Only set this if we're completing an instruction from a valid PC
            if !self.valid_pc_ranges.is_empty() && self.is_pc_valid(pc) {
                self.has_executed_valid_instruction = true;
            }
            
            // Reset long instruction tracking since instruction completed
            self.current_instruction_pc = None;
            self.current_instruction_start_cycle = cycle_count;

            // Check for PC stuck (if enabled)
            if self.config.enable_pc_loop_detection {
                self.check_pc_loop(pc, instruction)?;
            }
        }

        // Per-cycle checks (run every cycle, not just on instruction completion)
        
        // Long instruction detection
        if self.config.enable_long_instruction_detection {
            if let Some(instr_pc) = self.current_instruction_pc {
                let cycles_for_this_instruction = cycle_count.saturating_sub(self.current_instruction_start_cycle);
                
                if cycles_for_this_instruction > self.config.max_cycles_per_instruction {
                    return Err(HungStateError::LongInstruction {
                        pc: instr_pc,
                        instruction: self.current_instruction_word,
                        cycle_count: cycles_for_this_instruction,
                        fsm_state,
                    });
                }
            } else {
                // First cycle or after instruction completion, start tracking
                self.current_instruction_pc = Some(pc);
                self.current_instruction_word = instruction;
                self.current_instruction_start_cycle = cycle_count;
            }
        }

        // Check PC bounds (if enabled and ranges exist)
        // Only check if:
        // 1. We have valid PC ranges defined
        // 2. FSM is not in IDLE state (0)
        // 3. We've executed at least one valid instruction (to avoid false positives during init)
        //
        // This prevents false positives during simulator initialization while still catching
        // invalid PC jumps during execution, even if the instruction never completes.
        if self.config.enable_pc_bounds_detection 
            && !self.valid_pc_ranges.is_empty() 
            && fsm_state != 0 
            && self.has_executed_valid_instruction {
            // Check current PC
            if !self.is_pc_valid(pc) {
                return Err(HungStateError::PcOutOfBounds {
                    pc,
                    valid_ranges: self.valid_pc_ranges.clone(),
                });
            }
        }

        Ok(())
    }

    /// Check if PC is stuck in a loop
    fn check_pc_loop(&mut self, pc: u32, instruction: u32) -> Result<(), HungStateError> {
        // Track PC in history
        self.pc_history.push_back(pc);
        if self.pc_history.len() > self.config.pc_history_size {
            self.pc_history.pop_front();
        }

        // Check if PC is the same as last time
        if let Some(last) = self.last_pc {
            if last == pc {
                self.pc_stuck_count += 1;

                // If stuck count exceeds or equals threshold, report hang
                if self.pc_stuck_count >= self.config.pc_stuck_threshold {
                    let history: Vec<u32> = self.pc_history.iter().copied().collect();
                    return Err(HungStateError::PcStuck {
                        pc,
                        instruction,
                        count: self.pc_stuck_count,
                        history,
                    });
                }
            } else {
                // PC changed, reset counter to 1 for this new PC
                self.pc_stuck_count = 1;
            }
        } else {
            // First PC seen, initialize counter
            self.pc_stuck_count = 1;
        }

        self.last_pc = Some(pc);
        Ok(())
    }

    /// Reset the detector state (useful when starting a new simulation)
    pub fn reset(&mut self) {
        self.pc_history.clear();
        self.last_pc = None;
        self.pc_stuck_count = 0;
        self.current_instruction_pc = None;
        self.current_instruction_word = 0;
        self.current_instruction_start_cycle = 0;
        self.has_executed_valid_instruction = false;
        // Note: We don't clear valid_pc_ranges as they persist across resets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pc_stuck_detection() {
        let mut detector = HungDetector::new_default();

        // Simulate same PC being executed repeatedly (instruction_complete=true)
        for i in 0..49 {
            let result = detector.check_cycle(i, 0x8000_0000, 0x00000013, 2, true);
            assert!(result.is_ok(), "Should not detect hang at iteration {}", i);
        }

        // 50th iteration should trigger hang detection
        let result = detector.check_cycle(50, 0x8000_0000, 0x00000013, 2, true);
        assert!(result.is_err(), "Should detect PC stuck at threshold");

        match result {
            Err(HungStateError::PcStuck { pc, count, .. }) => {
                assert_eq!(pc, 0x8000_0000);
                assert_eq!(count, 50);
            }
            _ => panic!("Expected PcStuck error"),
        }
    }

    #[test]
    fn test_pc_stuck_reset_on_change() {
        let mut detector = HungDetector::new_default();

        // Execute same PC multiple times
        for i in 0..30 {
            let result = detector.check_cycle(i, 0x8000_0000, 0x00000013, 2, true);
            assert!(result.is_ok());
        }

        // Change PC (should reset counter)
        let result = detector.check_cycle(30, 0x8000_0004, 0x00000013, 2, true);
        assert!(result.is_ok());

        // Go back to original PC - counter should be reset
        for i in 31..80 {
            let result = detector.check_cycle(i, 0x8000_0000, 0x00000013, 2, true);
            assert!(result.is_ok());
        }

        // Should trigger at 50 after reset
        let result = detector.check_cycle(80, 0x8000_0000, 0x00000013, 2, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_pc_bounds_detection() {
        let mut detector = HungDetector::new_default();
        detector.update_pc_range(0x8000_0000, 0x8001_0000, true);

        // First, complete one instruction from a valid PC to enable bounds checking
        let result = detector.check_cycle(0, 0x8000_0000, 0x00000013, 2, true);
        assert!(result.is_ok());
        
        // Now bounds checking should be active
        // PC within bounds should be ok
        let result = detector.check_cycle(1, 0x8000_FFFC, 0x00000013, 2, false);
        assert!(result.is_ok());

        // PC below bounds should fail
        let result = detector.check_cycle(2, 0x7FFF_FFFC, 0x00000013, 2, false);
        assert!(result.is_err());
        match result {
            Err(HungStateError::PcOutOfBounds { pc, .. }) => {
                assert_eq!(pc, 0x7FFF_FFFC);
            }
            _ => panic!("Expected PcOutOfBounds error"),
        }

        // PC above bounds should fail
        let result = detector.check_cycle(3, 0x8001_0000, 0x00000013, 2, false);
        assert!(result.is_err());
        match result {
            Err(HungStateError::PcOutOfBounds { pc, .. }) => {
                assert_eq!(pc, 0x8001_0000);
            }
            _ => panic!("Expected PcOutOfBounds error"),
        }
    }

    #[test]
    fn test_pc_bounds_disabled_when_range_not_set() {
        let mut detector = HungDetector::new_default();
        // Don't set valid range

        // Any PC should be ok when bounds checking is enabled but range is not set
        let result = detector.check_cycle(0, 0x0000_0000, 0x00000013, 2, true);
        assert!(result.is_ok());

        let result = detector.check_cycle(1, 0xFFFF_FFFC, 0x00000013, 2, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_reset() {
        let mut detector = HungDetector::new_default();

        // Build up some state
        for i in 0..30 {
            let _ = detector.check_cycle(i, 0x8000_0000, 0x00000013, 2, true);
        }

        assert_eq!(detector.pc_stuck_count, 30);
        assert!(!detector.pc_history.is_empty());

        // Reset should clear everything except PC ranges
        detector.reset();

        assert_eq!(detector.pc_stuck_count, 0);
        assert!(detector.pc_history.is_empty());
        assert!(detector.last_pc.is_none());
    }

    #[test]
    fn test_multi_range_pc_bounds() {
        let mut detector = HungDetector::new_default();
        
        // Add two non-contiguous instruction ranges
        detector.update_pc_range(0x8000_0000, 0x8000_1000, true);
        detector.update_pc_range(0x8000_2000, 0x8000_3000, true);

        // First, complete one instruction from a valid PC to enable bounds checking
        assert!(detector.check_cycle(0, 0x8000_0000, 0, 2, true).is_ok());
        
        // Now bounds checking should be active
        // PCs in first range should be ok
        assert!(detector.check_cycle(1, 0x8000_0FFC, 0, 2, false).is_ok());

        // PCs in second range should be ok
        assert!(detector.check_cycle(2, 0x8000_2000, 0, 2, false).is_ok());
        assert!(detector.check_cycle(3, 0x8000_2FFC, 0, 2, false).is_ok());

        // PC in gap between ranges should fail
        let result = detector.check_cycle(4, 0x8000_1500, 0, 2, false);
        assert!(result.is_err());
        match result {
            Err(HungStateError::PcOutOfBounds { pc, valid_ranges }) => {
                assert_eq!(pc, 0x8000_1500);
                assert_eq!(valid_ranges.len(), 2);
            }
            _ => panic!("Expected PcOutOfBounds error"),
        }

        // PC outside all ranges should fail
        let result = detector.check_cycle(5, 0x8000_4000, 0, 2, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_range_merging() {
        let mut detector = HungDetector::new_default();
        
        // Add two adjacent ranges - they should merge
        detector.update_pc_range(0x8000_0000, 0x8000_1000, true);
        detector.update_pc_range(0x8000_1000, 0x8000_2000, true);

        let ranges = detector.get_valid_pc_ranges();
        assert_eq!(ranges.len(), 1, "Adjacent ranges should merge");
        assert_eq!(ranges[0].start, 0x8000_0000);
        assert_eq!(ranges[0].end, 0x8000_2000);

        // Add overlapping range - should extend
        detector.update_pc_range(0x8000_1800, 0x8000_2800, true);
        
        let ranges = detector.get_valid_pc_ranges();
        assert_eq!(ranges.len(), 1, "Overlapping ranges should merge");
        assert_eq!(ranges[0].start, 0x8000_0000);
        assert_eq!(ranges[0].end, 0x8000_2800);
    }

    #[test]
    fn test_long_instruction_detection() {
        let mut config = HungDetectorConfig::default();
        config.max_cycles_per_instruction = 100;
        config.enable_pc_loop_detection = false;
        
        let mut detector = HungDetector::new(config);

        // Simulate instruction taking many cycles without completing
        for cycle in 0..100u64 {
            let result = detector.check_cycle(cycle, 0x8000_0000, 0x00000013, 3, false);
            assert!(result.is_ok(), "Should not detect hang at cycle {}", cycle);
        }

        // Cycle 101 should trigger long instruction detection
        let result = detector.check_cycle(101, 0x8000_0000, 0x00000013, 3, false);
        assert!(result.is_err(), "Should detect long instruction");

        match result {
            Err(HungStateError::LongInstruction { pc, cycle_count, fsm_state, .. }) => {
                assert_eq!(pc, 0x8000_0000);
                assert!(cycle_count > 100);
                assert_eq!(fsm_state, 3);
            }
            _ => panic!("Expected LongInstruction error"),
        }
    }
}
