/// Hung State Detector
///
/// Provides multiple safety mechanisms to detect when the CPU simulator
/// has entered a hung state (infinite loop or stuck FSM).
/// This allows tests to use very high max_cycles limits while still
/// catching problematic conditions quickly.
use std::collections::VecDeque;

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
        4 => "MEM_ADDR",
        5 => "MEM_READ",
        6 => "MEM_WRITE",
        7 => "WRITEBACK",
        8 => "BRANCH",
        9 => "CSR",
        10 => "HALT",
        11 => "ATOMIC_RMW",
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
        }
    }

    /// Create a new hung state detector with default configuration
    #[allow(dead_code)]
    pub(crate) fn new_default() -> Self {
        Self::new(HungDetectorConfig::default())
    }

    /// Check for hung state on every cycle
    ///
    /// This detects cases where the FSM gets stuck and never completes an instruction,
    /// and tracks PC loops when instructions complete.
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
            // Reset long instruction tracking since instruction completed
            self.current_instruction_pc = None;
            self.current_instruction_start_cycle = cycle_count;

            // Check for PC stuck (if enabled). Callers should skip check_cycle() once CPU halts.
            if self.config.enable_pc_loop_detection {
                self.check_pc_loop(pc, instruction)?;
            }
        }

        // Per-cycle checks (run every cycle, not just on instruction completion)

        // Long instruction detection
        if self.config.enable_long_instruction_detection {
            if let Some(instr_pc) = self.current_instruction_pc {
                let cycles_for_this_instruction =
                    cycle_count.saturating_sub(self.current_instruction_start_cycle);

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
    fn test_reset() {
        let mut detector = HungDetector::new_default();

        // Build up some state
        for i in 0..30 {
            let _ = detector.check_cycle(i, 0x8000_0000, 0x00000013, 2, true);
        }

        assert_eq!(detector.pc_stuck_count, 30);
        assert!(!detector.pc_history.is_empty());

        // Reset should clear detector state
        detector.reset();

        assert_eq!(detector.pc_stuck_count, 0);
        assert!(detector.pc_history.is_empty());
        assert!(detector.last_pc.is_none());
    }

    #[test]
    fn test_long_instruction_detection() {
        let config = HungDetectorConfig {
            max_cycles_per_instruction: 100,
            enable_pc_loop_detection: false,
            ..Default::default()
        };

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
            Err(HungStateError::LongInstruction {
                pc,
                cycle_count,
                fsm_state,
                ..
            }) => {
                assert_eq!(pc, 0x8000_0000);
                assert!(cycle_count > 100);
                assert_eq!(fsm_state, 3);
            }
            _ => panic!("Expected LongInstruction error"),
        }
    }
}
