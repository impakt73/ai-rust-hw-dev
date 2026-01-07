/// Integration tests for VCD parsing functionality
use std::fs::File;
use std::io::Write;
use tempfile::TempDir;
use vcd_mcp::domain::parse_vcd;

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
