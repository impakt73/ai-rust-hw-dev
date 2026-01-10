/// Hung State Detector
///
/// Provides multiple safety mechanisms to detect when the CPU simulator
/// has entered a hung state (infinite loop, stuck FSM, or invalid PC).
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
    
    /// Number of cycles a non-waiting FSM state can stay before warning
    pub fsm_stuck_threshold: u64,
    
    /// Start address of valid instruction memory region
    pub valid_pc_start: Option<u32>,
    
    /// End address of valid instruction memory region (exclusive)
    pub valid_pc_end: Option<u32>,
    
    /// Enable PC loop detection
    pub enable_pc_loop_detection: bool,
    
    /// Enable FSM state stuck detection
    pub enable_fsm_stuck_detection: bool,
    
    /// Enable out-of-bounds PC detection
    pub enable_pc_bounds_detection: bool,
}

impl Default for HungDetectorConfig {
    fn default() -> Self {
        Self {
            pc_history_size: 100,
            pc_stuck_threshold: 50,
            fsm_stuck_threshold: 500,
            valid_pc_start: None,
            valid_pc_end: None,
            enable_pc_loop_detection: true,
            enable_fsm_stuck_detection: true,
            enable_pc_bounds_detection: true,
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
    
    /// FSM has been stuck in a non-waiting state for too long
    FsmStuck {
        state: u8,
        state_name: String,
        cycle_count: u64,
    },
    
    /// PC has jumped outside valid instruction memory
    PcOutOfBounds {
        pc: u32,
        valid_start: u32,
        valid_end: u32,
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
            HungStateError::FsmStuck {
                state,
                state_name,
                cycle_count,
            } => {
                write!(
                    f,
                    "CPU hung: FSM stuck in state {} ({}) for {} cycles",
                    state_name, state, cycle_count
                )
            }
            HungStateError::PcOutOfBounds {
                pc,
                valid_start,
                valid_end,
            } => {
                write!(
                    f,
                    "CPU hung: PC 0x{:08x} is outside valid instruction memory range [0x{:08x}, 0x{:08x})",
                    pc, valid_start, valid_end
                )
            }
        }
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
    
    // FSM state tracking
    current_fsm_state: Option<u8>,
    fsm_state_cycle_count: u64,
}

impl HungDetector {
    /// Create a new hung state detector with the given configuration
    pub fn new(config: HungDetectorConfig) -> Self {
        Self {
            pc_history: VecDeque::with_capacity(config.pc_history_size),
            config,
            last_pc: None,
            pc_stuck_count: 0,
            current_fsm_state: None,
            fsm_state_cycle_count: 0,
        }
    }
    
    /// Create a new hung state detector with default configuration
    pub fn new_default() -> Self {
        Self::new(HungDetectorConfig::default())
    }
    
    /// Set the valid PC range for bounds checking
    ///
    /// # Arguments
    /// * `start` - Start address of valid instruction memory (inclusive)
    /// * `end` - End address of valid instruction memory (exclusive)
    pub fn set_valid_pc_range(&mut self, start: u32, end: u32) {
        self.config.valid_pc_start = Some(start);
        self.config.valid_pc_end = Some(end);
    }
    
    /// Check for hung state after an instruction completes
    ///
    /// # Arguments
    /// * `pc` - Program counter of completed instruction
    /// * `instruction` - Instruction word that was executed
    /// * `fsm_state` - Current FSM state (4-bit value)
    /// * `cycle_count` - Total simulation cycle count
    ///
    /// # Returns
    /// * `Ok(())` if no hang detected
    /// * `Err(HungStateError)` if a hang condition was detected
    pub fn check_instruction(
        &mut self,
        pc: u32,
        instruction: u32,
        fsm_state: u8,
        cycle_count: u64,
    ) -> Result<(), HungStateError> {
        // Check PC bounds first (if enabled and range is set)
        if self.config.enable_pc_bounds_detection {
            if let (Some(start), Some(end)) = (self.config.valid_pc_start, self.config.valid_pc_end)
            {
                if pc < start || pc >= end {
                    return Err(HungStateError::PcOutOfBounds {
                        pc,
                        valid_start: start,
                        valid_end: end,
                    });
                }
            }
        }
        
        // Check for PC stuck (if enabled)
        if self.config.enable_pc_loop_detection {
            self.check_pc_loop(pc, instruction)?;
        }
        
        // Check for FSM stuck (if enabled)
        if self.config.enable_fsm_stuck_detection {
            self.check_fsm_stuck(fsm_state, cycle_count)?;
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
    
    /// Check if FSM is stuck in a state for too long
    fn check_fsm_stuck(&mut self, fsm_state: u8, _cycle_count: u64) -> Result<(), HungStateError> {
        // Check if FSM state changed
        if let Some(last_state) = self.current_fsm_state {
            if last_state == fsm_state {
                self.fsm_state_cycle_count += 1;
                
                // Check if stuck in non-waiting state
                // States 1 (FETCH), 5 (MEM_READ), 6 (MEM_WRITE) can have variable latency
                // State 10 (HALT) is intentionally permanent
                let is_waiting_state = matches!(fsm_state, 1 | 5 | 6 | 10);
                
                if !is_waiting_state && self.fsm_state_cycle_count > self.config.fsm_stuck_threshold
                {
                    return Err(HungStateError::FsmStuck {
                        state: fsm_state,
                        state_name: Self::fsm_state_name(fsm_state).to_string(),
                        cycle_count: self.fsm_state_cycle_count,
                    });
                }
            } else {
                // State changed, reset counter to 1 for this new state
                self.fsm_state_cycle_count = 1;
            }
        } else {
            // First state seen, initialize counter
            self.fsm_state_cycle_count = 1;
        }
        
        self.current_fsm_state = Some(fsm_state);
        Ok(())
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
            _ => "UNKNOWN",
        }
    }
    
    /// Reset the detector state (useful when starting a new simulation)
    pub fn reset(&mut self) {
        self.pc_history.clear();
        self.last_pc = None;
        self.pc_stuck_count = 0;
        self.current_fsm_state = None;
        self.fsm_state_cycle_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pc_stuck_detection() {
        let mut detector = HungDetector::new_default();
        
        // Simulate same PC being executed repeatedly
        for i in 0..49 {
            let result = detector.check_instruction(0x8000_0000, 0x00000013, 2, i);
            assert!(result.is_ok(), "Should not detect hang at iteration {}", i);
        }
        
        // 50th iteration should trigger hang detection
        let result = detector.check_instruction(0x8000_0000, 0x00000013, 2, 50);
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
            let result = detector.check_instruction(0x8000_0000, 0x00000013, 2, i);
            assert!(result.is_ok());
        }
        
        // Change PC (should reset counter)
        let result = detector.check_instruction(0x8000_0004, 0x00000013, 2, 30);
        assert!(result.is_ok());
        
        // Go back to original PC - counter should be reset
        for i in 31..80 {
            let result = detector.check_instruction(0x8000_0000, 0x00000013, 2, i);
            assert!(result.is_ok());
        }
        
        // Should trigger at 50 after reset
        let result = detector.check_instruction(0x8000_0000, 0x00000013, 2, 80);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_fsm_stuck_detection() {
        let mut config = HungDetectorConfig::default();
        config.fsm_stuck_threshold = 100; // Lower threshold for testing
        config.enable_pc_loop_detection = false; // Disable PC detection
        
        let mut detector = HungDetector::new(config);
        
        // Simulate being stuck in EXECUTE state (state 3)
        for i in 0..100u64 {
            // Use different PCs to avoid PC stuck detection
            let result = detector.check_instruction(0x8000_0000 + ((i as u32) * 4), 0x00000013, 3, i);
            assert!(result.is_ok(), "Should not detect hang at iteration {}", i);
        }
        
        // Next cycle should trigger FSM stuck detection
        let result = detector.check_instruction(0x8000_0194, 0x00000013, 3, 101);
        assert!(result.is_err(), "Should detect FSM stuck");
        
        match result {
            Err(HungStateError::FsmStuck { state, cycle_count, .. }) => {
                assert_eq!(state, 3);
                assert!(cycle_count > 100);
            }
            _ => panic!("Expected FsmStuck error"),
        }
    }
    
    #[test]
    fn test_fsm_waiting_states_not_flagged() {
        let mut config = HungDetectorConfig::default();
        config.fsm_stuck_threshold = 10; // Very low threshold
        config.enable_pc_loop_detection = false;
        
        let mut detector = HungDetector::new(config);
        
        // FETCH state (1) should not trigger even with low threshold
        for i in 0..1000 {
            let result = detector.check_instruction(0x8000_0000, 0x00000013, 1, i);
            assert!(result.is_ok(), "FETCH state should not trigger stuck detection");
        }
        
        // MEM_READ state (5) should not trigger
        for i in 1000..2000 {
            let result = detector.check_instruction(0x8000_0000, 0x00000013, 5, i);
            assert!(result.is_ok(), "MEM_READ state should not trigger stuck detection");
        }
        
        // HALT state (10) should not trigger
        for i in 2000..3000 {
            let result = detector.check_instruction(0x8000_0000, 0x00000013, 10, i);
            assert!(result.is_ok(), "HALT state should not trigger stuck detection");
        }
    }
    
    #[test]
    fn test_pc_bounds_detection() {
        let mut detector = HungDetector::new_default();
        detector.set_valid_pc_range(0x8000_0000, 0x8001_0000);
        
        // PC within bounds should be ok
        let result = detector.check_instruction(0x8000_0000, 0x00000013, 2, 0);
        assert!(result.is_ok());
        
        let result = detector.check_instruction(0x8000_FFFC, 0x00000013, 2, 1);
        assert!(result.is_ok());
        
        // PC below bounds should fail
        let result = detector.check_instruction(0x7FFF_FFFC, 0x00000013, 2, 2);
        assert!(result.is_err());
        match result {
            Err(HungStateError::PcOutOfBounds { pc, .. }) => {
                assert_eq!(pc, 0x7FFF_FFFC);
            }
            _ => panic!("Expected PcOutOfBounds error"),
        }
        
        // PC above bounds should fail
        let result = detector.check_instruction(0x8001_0000, 0x00000013, 2, 3);
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
        let result = detector.check_instruction(0x0000_0000, 0x00000013, 2, 0);
        assert!(result.is_ok());
        
        let result = detector.check_instruction(0xFFFF_FFFC, 0x00000013, 2, 1);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_reset() {
        let mut detector = HungDetector::new_default();
        
        // Build up some state
        for i in 0..30 {
            let _ = detector.check_instruction(0x8000_0000, 0x00000013, 2, i);
        }
        
        assert_eq!(detector.pc_stuck_count, 30);
        assert!(!detector.pc_history.is_empty());
        
        // Reset should clear everything
        detector.reset();
        
        assert_eq!(detector.pc_stuck_count, 0);
        assert!(detector.pc_history.is_empty());
        assert!(detector.last_pc.is_none());
        assert!(detector.current_fsm_state.is_none());
    }
}
