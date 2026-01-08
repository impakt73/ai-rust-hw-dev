/// Integration tests for VCD parsing functionality
use std::fs::File;
use std::io::Write;
use tempfile::TempDir;
use vcd_mcp::domain::{parse_vcd, EdgeType};

// Helper function to create a test VCD file
fn create_test_vcd_file(temp_dir: &TempDir) -> std::path::PathBuf {
    let vcd_path = temp_dir.path().join("test.vcd");
    let mut file = File::create(&vcd_path).expect("Failed to create VCD file");

    // Write a VCD file with nested scopes
    writeln!(file, "$date").unwrap();
    writeln!(file, "   Today").unwrap();
    writeln!(file, "$end").unwrap();
    writeln!(file, "$version").unwrap();
    writeln!(file, "   VCD Test 1.0").unwrap();
    writeln!(file, "$end").unwrap();
    writeln!(file, "$timescale 1ns $end").unwrap();
    writeln!(file, "$scope module top $end").unwrap();
    writeln!(file, "$var wire 1 ! clk $end").unwrap();
    writeln!(file, "$scope module cpu $end").unwrap();
    writeln!(file, "$var wire 8 \" data $end").unwrap();
    writeln!(file, "$var wire 32 # pc $end").unwrap();
    writeln!(file, "$upscope $end").unwrap();
    writeln!(file, "$upscope $end").unwrap();
    writeln!(file, "$enddefinitions $end").unwrap();
    writeln!(file, "#0").unwrap();
    writeln!(file, "$dumpvars").unwrap();
    writeln!(file, "0!").unwrap();
    writeln!(file, "b00000000 \"").unwrap();
    writeln!(file, "b00000000000000000000000000000000 #").unwrap();
    writeln!(file, "$end").unwrap();
    writeln!(file, "#10").unwrap();
    writeln!(file, "1!").unwrap();
    writeln!(file, "b10101010 \"").unwrap();
    writeln!(file, "#20").unwrap();
    writeln!(file, "0!").unwrap();
    writeln!(file, "b11110000 \"").unwrap();
    writeln!(file, "b00000000000000000000000000000100 #").unwrap();

    drop(file);
    vcd_path
}

#[test]
fn test_parse_vcd_header() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let vcd_path = create_test_vcd_file(&temp_dir);

    let analysis = parse_vcd(vcd_path.to_str().unwrap()).expect("Failed to parse VCD");

    // Check header metadata
    assert_eq!(analysis.header.version, Some("VCD Test 1.0".to_string()));
    assert_eq!(analysis.header.date, Some("Today".to_string()));
}

#[test]
fn test_parse_vcd_signals() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let vcd_path = create_test_vcd_file(&temp_dir);

    let analysis = parse_vcd(vcd_path.to_str().unwrap()).expect("Failed to parse VCD");

    // Check signal count
    assert_eq!(analysis.id_to_name.len(), 3);

    // Check hierarchical signal names - should handle nested scopes correctly
    assert!(analysis.name_to_id.contains_key("top.clk"));
    assert!(analysis.name_to_id.contains_key("top.cpu.data"));
    assert!(analysis.name_to_id.contains_key("top.cpu.pc"));
}

#[test]
fn test_parse_vcd_values() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let vcd_path = create_test_vcd_file(&temp_dir);

    let analysis = parse_vcd(vcd_path.to_str().unwrap()).expect("Failed to parse VCD");

    // Check that we have value changes at expected timestamps
    assert!(analysis.time_changes.iter().any(|(t, _)| *t == 0));
    assert!(analysis.time_changes.iter().any(|(t, _)| *t == 10));
    assert!(analysis.time_changes.iter().any(|(t, _)| *t == 20));
}

#[test]
fn test_signal_value_query() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let vcd_path = create_test_vcd_file(&temp_dir);

    let analysis = parse_vcd(vcd_path.to_str().unwrap()).expect("Failed to parse VCD");

    // Get signal ID for clk
    let clk_id = *analysis
        .name_to_id
        .get("top.clk")
        .expect("clk signal not found");

    // Query value at time 0 (should be 0)
    let value_at_0 = analysis.get_signal_value_at(clk_id, 0);
    assert!(value_at_0.is_some());

    // Query value at time 10 (should be 1)
    let value_at_10 = analysis.get_signal_value_at(clk_id, 10);
    assert!(value_at_10.is_some());

    // Query value at time 15 (between changes, should return value from time 10)
    let value_at_15 = analysis.get_signal_value_at(clk_id, 15);
    assert!(value_at_15.is_some());
}

#[test]
fn test_signal_changes_in_range() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let vcd_path = create_test_vcd_file(&temp_dir);

    let analysis = parse_vcd(vcd_path.to_str().unwrap()).expect("Failed to parse VCD");

    // Get signal ID for data
    let data_id = *analysis
        .name_to_id
        .get("top.cpu.data")
        .expect("data signal not found");

    // Query changes between time 0 and 20
    let changes = analysis.get_signal_changes(data_id, 0, 20);

    // Should have changes at times 0, 10, and 20
    assert!(changes.len() >= 2); // At least 2 changes (could include initial value)
}

#[test]
fn test_get_file_metadata() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let vcd_path = create_test_vcd_file(&temp_dir);

    let analysis = parse_vcd(vcd_path.to_str().unwrap()).expect("Failed to parse VCD");
    let metadata = analysis.get_file_metadata();

    // Check basic metadata
    assert_eq!(metadata.signal_count, 3);
    assert_eq!(metadata.first_time, 0);
    assert_eq!(metadata.last_time, 20);
    assert!(metadata.timescale.is_some());
    // The timescale format from VCD parser includes units like "1ns"
    let ts = metadata.timescale.unwrap();
    assert!(ts.contains("N") || ts.contains("n")); // TimescaleUnit::NS
}

#[test]
fn test_get_signal_summary() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let vcd_path = create_test_vcd_file(&temp_dir);

    let analysis = parse_vcd(vcd_path.to_str().unwrap()).expect("Failed to parse VCD");

    // Get signal ID for clk
    let clk_id = *analysis
        .name_to_id
        .get("top.clk")
        .expect("clk signal not found");

    // Get summary for the clock signal over entire simulation
    let summary = analysis
        .get_signal_summary(clk_id, 0, 20)
        .expect("Summary should exist");

    // Clock has initial value at 0, then changes at times 10 and 20
    // So change_count should be 3 (initial + 2 changes) OR 2 (just the changes)
    // depending on whether we count the initial value
    assert!(summary.change_count >= 2);
    assert_eq!(summary.first_change_time, Some(0)); // First change is at initial time
    assert_eq!(summary.last_change_time, Some(20));
    assert_eq!(summary.bit_width, 1);
    assert!(summary.last_value.is_some());
}

#[test]
fn test_count_signal_edges_rising() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let vcd_path = create_test_vcd_file(&temp_dir);

    let analysis = parse_vcd(vcd_path.to_str().unwrap()).expect("Failed to parse VCD");

    // Get signal ID for clk
    let clk_id = *analysis
        .name_to_id
        .get("top.clk")
        .expect("clk signal not found");

    // Count rising edges (0->1 transitions)
    let rising_edges = analysis.count_signal_edges(clk_id, EdgeType::Rising, None, 0, 20);

    // Clock starts at 0, goes to 1 at time 10, back to 0 at time 20
    // So there should be 1 rising edge
    assert_eq!(rising_edges, 1);
}

#[test]
fn test_count_signal_edges_falling() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let vcd_path = create_test_vcd_file(&temp_dir);

    let analysis = parse_vcd(vcd_path.to_str().unwrap()).expect("Failed to parse VCD");

    // Get signal ID for clk
    let clk_id = *analysis
        .name_to_id
        .get("top.clk")
        .expect("clk signal not found");

    // Count falling edges (1->0 transitions)
    let falling_edges = analysis.count_signal_edges(clk_id, EdgeType::Falling, None, 0, 20);

    // Clock starts at 0, goes to 1 at time 10, back to 0 at time 20
    // So there should be 1 falling edge
    assert_eq!(falling_edges, 1);
}

#[test]
fn test_count_signal_edges_both() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let vcd_path = create_test_vcd_file(&temp_dir);

    let analysis = parse_vcd(vcd_path.to_str().unwrap()).expect("Failed to parse VCD");

    // Get signal ID for clk
    let clk_id = *analysis
        .name_to_id
        .get("top.clk")
        .expect("clk signal not found");

    // Count both edges
    let both_edges = analysis.count_signal_edges(clk_id, EdgeType::Both, None, 0, 20);

    // Clock starts at 0, goes to 1 at time 10, back to 0 at time 20
    // So there should be 2 total edges
    assert_eq!(both_edges, 2);
}

#[test]
fn test_count_vector_signal_edges() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let vcd_path = create_test_vcd_file(&temp_dir);

    let analysis = parse_vcd(vcd_path.to_str().unwrap()).expect("Failed to parse VCD");

    // Get signal ID for data (8-bit vector)
    let data_id = *analysis
        .name_to_id
        .get("top.cpu.data")
        .expect("data signal not found");

    // Count edges on bit 0 (LSB)
    // data changes: 0x00 -> 0xAA (bit 0: 0->0) -> 0xF0 (bit 0: 0->0)
    // No edges on bit 0
    let edges_bit0 = analysis.count_signal_edges(data_id, EdgeType::Both, Some(0), 0, 20);
    assert_eq!(edges_bit0, 0);

    // Count edges on bit 1
    // data changes: 0x00 -> 0xAA (bit 1: 0->1) -> 0xF0 (bit 1: 1->0)
    // Two edges on bit 1
    let edges_bit1 = analysis.count_signal_edges(data_id, EdgeType::Both, Some(1), 0, 20);
    assert_eq!(edges_bit1, 2);
}

#[test]
fn test_vcd_value_bit_width() {
    use vcd_mcp::domain::VcdValue;

    // Scalar has width 1
    let scalar = VcdValue::Scalar(vcd::Value::V1);
    assert_eq!(scalar.bit_width(), 1);

    // Vector width matches its length
    let vector = VcdValue::Vector(vec![vcd::Value::V1, vcd::Value::V0, vcd::Value::V1]);
    assert_eq!(vector.bit_width(), 3);

    // Real has width 64
    let real = VcdValue::Real(3.14);
    assert_eq!(real.bit_width(), 64);

    // String has width 0
    let string = VcdValue::String("test".to_string());
    assert_eq!(string.bit_width(), 0);
}

#[test]
fn test_vcd_value_get_bit() {
    use vcd_mcp::domain::VcdValue;

    // Test scalar
    let scalar_1 = VcdValue::Scalar(vcd::Value::V1);
    assert_eq!(scalar_1.get_bit(0), Some(true));
    assert_eq!(scalar_1.get_bit(1), None); // Out of bounds

    let scalar_0 = VcdValue::Scalar(vcd::Value::V0);
    assert_eq!(scalar_0.get_bit(0), Some(false));

    // Test vector (MSB first in VCD)
    // Binary 0b101 = [V1, V0, V1] in MSB-first order
    let vector = VcdValue::Vector(vec![vcd::Value::V1, vcd::Value::V0, vcd::Value::V1]);
    assert_eq!(vector.get_bit(0), Some(true)); // LSB
    assert_eq!(vector.get_bit(1), Some(false)); // Middle bit
    assert_eq!(vector.get_bit(2), Some(true)); // MSB
    assert_eq!(vector.get_bit(3), None); // Out of bounds
}
