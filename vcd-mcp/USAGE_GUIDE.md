# VCD MCP Server - Usage Guide

This guide demonstrates how to use the VCD MCP server tools efficiently to analyze waveform data.

## Comparison: Old vs. New Approach

### ❌ Old Approach (Inefficient)

**Problem:** To count clock cycles, you had to:
1. Fetch ALL value changes for the clock signal
2. Transfer potentially MB-sized response
3. Process the data client-side to count edges
4. Risk truncation due to token limits

```
# Request
get_signal_values:
  file_path: "/path/to/sim.vcd"
  signal_names: ["TOP.clk"]
  start_time: 0
  end_time: 10000

# Response (HUGE - potentially 10,000+ entries)
{
  "values": [
    {"signal": "TOP.clk", "time": 0, "value": "0"},
    {"signal": "TOP.clk", "time": 5, "value": "1"},
    {"signal": "TOP.clk", "time": 10, "value": "0"},
    ... (thousands more entries)
  ]
}

# Then you had to count transitions manually!
```

### ✅ New Approach (Efficient)

**Solution:** Use the new efficient tools:

```
# Request
count_signal_edges:
  file_path: "/path/to/sim.vcd"
  signal_name: "TOP.clk"
  edge_type: "rising"
  start_time: 0
  end_time: 10000

# Response (TINY - just the answer)
{
  "signal": "TOP.clk",
  "edge_type": "rising",
  "count": 5000
}
```

**Result:** Same information, but:
- 99.9% reduction in response size
- Instant server-side computation
- No client-side processing needed

## Recommended Workflow

### 1. Start with File Info

Always begin by understanding the scope of your VCD file:

```json
Tool: get_file_info
Arguments:
  file_path: "/path/to/sim.vcd"

Response:
{
  "timescale": "1 NS",
  "first_time": 0,
  "last_time": 10005,
  "signal_count": 173,
  "top_scopes": ["TOP"],
  "file_size_bytes": 4100000
}
```

**Why?** Know the time range before querying signals.

### 2. Get Signal Summaries

Explore which signals are active and interesting:

```json
Tool: get_signal_summary
Arguments:
  file_path: "/path/to/sim.vcd"
  signal_names: ["TOP.top.instr_complete", "TOP.top.cpu_state"]
  start_time: 0
  end_time: 10005

Response:
{
  "summaries": {
    "TOP.top.instr_complete": {
      "change_count": 5003,
      "first_change_time": 8,
      "last_change_time": 10005,
      "last_value": "1",
      "bit_width": 1
    },
    "TOP.top.cpu_state": {
      "change_count": 234,
      "first_change_time": 10,
      "last_change_time": 9987,
      "bit_width": 3
    }
  }
}
```

**Why?** Small payload shows which signals are actively changing.

### 3. Count Events/Edges

For counting clock cycles or events:

```json
Tool: count_signal_edges
Arguments:
  file_path: "/path/to/sim.vcd"
  signal_name: "TOP.clk"
  edge_type: "rising"
  start_time: 0

Response:
{
  "signal": "TOP.clk",
  "edge_type": "rising",
  "count": 5002
}
```

**Why?** Direct answer without transferring all transitions.

### 4. Get Specific Values (When Needed)

Only fetch detailed values when you need them, and use limits:

```json
Tool: get_signal_values
Arguments:
  file_path: "/path/to/sim.vcd"
  signal_names: ["TOP.top.pc"]
  start_time: 1000
  end_time: 1100
  only_changes: true
  limit: 50

Response:
{
  "values": [
    {"signal": "TOP.top.pc", "time": 1008, "value": "0b00000000000000000000000000000100"},
    {"signal": "TOP.top.pc", "time": 1018, "value": "0b00000000000000000000000000001000"},
    ... (up to 50 changes)
  ],
  "truncated": false
}
```

**Why?** Get precise data for small time windows with pagination control.

## Use Case Examples

### Counting Clock Cycles

```json
count_signal_edges:
  file_path: "/path/to/sim.vcd"
  signal_name: "TOP.clk"
  edge_type: "rising"
```

### Counting Instruction Completions

```json
count_signal_edges:
  file_path: "/path/to/sim.vcd"
  signal_name: "TOP.cpu.instr_complete"
  edge_type: "rising"
```

### Analyzing Per-Bit Vector Signals

Count transitions on a specific bit of a vector:

```json
count_signal_edges:
  file_path: "/path/to/sim.vcd"
  signal_name: "TOP.cpu.alu_flags"
  edge_type: "both"
  bit_index: 2  # Zero flag
```

### Quick Health Check

Get summaries of multiple signals to see activity:

```json
get_signal_summary:
  file_path: "/path/to/sim.vcd"
  signal_names: ["TOP.clk", "TOP.reset", "TOP.cpu.state", "TOP.mem.busy"]
  start_time: 0
```

### Debugging Specific Time Window

When you need actual values:

```json
get_signal_values:
  file_path: "/path/to/sim.vcd"
  signal_names: ["TOP.cpu.pc", "TOP.cpu.instruction"]
  start_time: 5000
  end_time: 5100
  only_changes: true
  limit: 100
```

## Performance Benefits

### Response Size Comparison

| Task | Old Approach | New Approach | Improvement |
|------|-------------|--------------|-------------|
| Count 5000 clock edges | ~500 KB | ~100 bytes | 5000x smaller |
| Get signal summary | ~500 KB | ~200 bytes | 2500x smaller |
| File metadata | N/A | ~150 bytes | New capability |
| Check signal activity | ~1 MB | ~300 bytes | 3300x smaller |

### Query Time Comparison

| Task | Old (fetch + process) | New (server-side) |
|------|----------------------|-------------------|
| Count edges | 2-5 seconds | < 100ms |
| Get summary | 2-5 seconds | < 100ms |
| File info | N/A | < 50ms |

## Best Practices

1. **Always start with `get_file_info`** to understand scope
2. **Use `get_signal_summary`** to explore before fetching values
3. **Use `count_signal_edges`** for all counting tasks
4. **Only use `get_signal_values`** when you need actual values
5. **Always use `limit`** parameter when fetching values over large ranges
6. **Use `only_changes=true`** to reduce payload size
7. **Query small time windows** instead of entire simulation

## Common Patterns

### Pattern 1: Clock Cycle Analysis
```
1. get_file_info → get time range
2. count_signal_edges(clk, rising) → get total cycles
3. If needed: get_signal_values(clk, small window) → verify clock behavior
```

### Pattern 2: Signal Activity Analysis
```
1. get_file_info → get signal list and time range
2. get_signal_summary(multiple signals) → see which are active
3. Focus on active signals for detailed analysis
```

### Pattern 3: Event Debugging
```
1. get_signal_summary(event_signal) → see when events occur
2. count_signal_edges(event_signal) → count total events
3. get_signal_values(related_signals, window around event) → debug specific event
```

## Integration with AI Agents

The new tools are specifically designed for AI agents:

- **Small responses** fit within token budgets
- **Direct answers** reduce need for processing
- **Structured data** is easier to understand
- **Minimal queries** reduce conversation length
- **Server-side computation** is faster and more reliable

Instead of:
> "Fetch all clock values and count the rising edges"

You can now ask:
> "How many clock cycles occurred?"

And get a direct answer instantly.
