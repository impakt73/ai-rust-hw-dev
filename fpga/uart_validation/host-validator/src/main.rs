use std::env;
use std::io::{Read, Write};
use std::time::Duration;

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let port_name = args
        .next()
        .ok_or_else(|| "usage: uart-host-validator <serial-port> [baud]".to_string())?;

    let baud = match args.next() {
        Some(value) => value
            .parse::<u32>()
            .map_err(|e| format!("invalid baud '{}': {e}", value))?,
        None => 115_200,
    };

    let mut port = serialport::new(&port_name, baud)
        .timeout(Duration::from_millis(500))
        .open()
        .map_err(|e| format!("failed to open serial port '{}': {e}", port_name))?;

    let mut pattern = Vec::new();
    pattern.extend([0x00, 0xFF, 0x55, 0xAA, 0x12, 0x34, 0x7E, 0x81]);
    pattern.extend(0u8..=31u8);

    for &byte in &pattern {
        port.write_all(&[byte])
            .map_err(|e| format!("write failed for 0x{byte:02X}: {e}"))?;

        let mut response = [0u8; 1];
        port.read_exact(&mut response)
            .map_err(|e| format!("read failed for 0x{byte:02X}: {e}"))?;

        if response[0] != byte {
            return Err(format!(
                "mismatch: sent 0x{byte:02X}, received 0x{:02X}",
                response[0]
            ));
        }
    }

    println!(
        "UART validation passed on {} at {} baud ({} bytes)",
        port_name,
        baud,
        pattern.len()
    );

    Ok(())
}
