use std::collections::VecDeque;

/// FIFO peripheral for UART-style communication
/// Provides buffered I/O between the simulated CPU and host
pub struct Fifo {
    /// Data sent FROM CPU -> Host
    pub tx: VecDeque<u8>,
    /// Data sent FROM Host -> CPU
    pub rx: VecDeque<u8>,
}

impl Fifo {
    /// Create a new FIFO with empty TX and RX queues
    pub fn new() -> Self {
        Fifo {
            tx: VecDeque::new(),
            rx: VecDeque::new(),
        }
    }

    /// Read the STATUS register
    /// Bit 0 (RX_VALID): 1 if RX has data, 0 if empty
    /// Bit 1 (TX_READY): Always 1 (simulated buffer is infinite)
    pub fn read_status(&self) -> u32 {
        let rx_valid = if self.rx.is_empty() { 0 } else { 1 };
        let tx_ready = 1; // Always ready (infinite buffer)
        (tx_ready << 1) | rx_valid
    }

    /// Read the DATA register
    /// Pops a byte from the RX queue
    /// Returns 0 if RX is empty
    pub fn read_data(&mut self) -> u32 {
        self.rx.pop_front().unwrap_or(0) as u32
    }

    /// Write to the DATA register
    /// Pushes a byte to the TX queue
    pub fn write_data(&mut self, val: u32) {
        self.tx.push_back(val as u8);
    }
}

impl Default for Fifo {
    fn default() -> Self {
        Self::new()
    }
}
