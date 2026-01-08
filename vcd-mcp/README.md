# VCD MCP Server

An MCP (Model Context Protocol) server that provides tools for analyzing Value Change Dump (VCD) files. Designed for integration with GitHub Copilot and other MCP clients.

## Features

The server exposes **six** tools for efficient VCD analysis:

### Core Tools

### 1. `inspect_vcd_header`
Reads the VCD header to extract metadata and understand the file structure.

**Arguments:**
- `file_path` (string, required): Absolute path to the .vcd file

**Returns:**
- Timescale information
- Date and version metadata
- List of top-level scopes/modules
- Total signal count

### 2. `list_signals`
Lists all signal names in the VCD file, optionally filtered by scope.

**Arguments:**
- `file_path` (string, required): Absolute path to the .vcd file
- `scope_filter` (string, optional): Filter to a specific module (e.g., "top.cpu")

**Returns:**
- Array of signal names (hierarchical paths)
- Total signal count

### 3. `get_signal_values`
Retrieves signal values at a specific time or over a time range.

**Arguments:**
- `file_path` (string, required): Absolute path to the .vcd file
- `signal_names` (array of strings, required): Full hierarchical signal names to query
- `start_time` (integer, required): Simulation timestamp
- `end_time` (integer, optional): If provided, returns all changes between start and end times
- `only_changes` (boolean, optional, default=false): If true, only return actual changes (excludes initial values from before start_time)
- `limit` (integer, optional): Maximum number of value changes to return per signal (for pagination)

**Returns:**
- Array of signal values with timestamps
- Warning if any signals were not found
- Truncation info if limit was applied

### Efficient Query Tools

### 4. `get_file_info`
Get file metadata including timescale, time range, signal count, and file size. **Use this first** to understand the scope before querying.

**Arguments:**
- `file_path` (string, required): Absolute path to the .vcd file

**Returns:**
- `timescale`: Time unit (e.g., "1 NS")
- `first_time`: Earliest timestamp in the file
- `last_time`: Latest timestamp in the file
- `signal_count`: Total number of signals
- `top_scopes`: Top-level module names
- `file_size_bytes`: File size in bytes

**Example Response:**
```json
{
  "timescale": "1 NS",
  "first_time": 0,
  "last_time": 10005,
  "signal_count": 173,
  "top_scopes": ["TOP"],
  "file_size_bytes": 4100000
}
```

### 5. `get_signal_summary`
Get summary statistics for signals including change count, first/last change time, bit width, and last value. **Efficient for exploratory analysis** - returns small payloads instead of full value streams.

**Arguments:**
- `file_path` (string, required): Absolute path to the .vcd file
- `signal_names` (array of strings, required): Signals to summarize
- `start_time` (integer, optional, default=0): Start of summary range
- `end_time` (integer, optional, default=end of simulation): End of summary range

**Returns:**
- Per-signal summary with:
  - `change_count`: Number of times the signal changed
  - `first_change_time`: Time of first change (if any)
  - `last_change_time`: Time of last change (if any)
  - `last_value`: Most recent value
  - `bit_width`: Width of the signal in bits

**Example Response:**
```json
{
  "summaries": {
    "TOP.top.instr_complete": {
      "change_count": 5003,
      "first_change_time": 8,
      "last_change_time": 10005,
      "last_value": "1",
      "bit_width": 1
    },
    "TOP.top.completed_instr_reg": {
      "change_count": 2500,
      "bit_width": 32
    }
  }
}
```

### 6. `count_signal_edges`
Count rising, falling, or both edges for a signal. **Essential for clock cycle counting and event analysis**. Supports scalar signals and individual bits of vector signals.

**Arguments:**
- `file_path` (string, required): Absolute path to the .vcd file
- `signal_name` (string, required): Signal to analyze
- `edge_type` (string, optional, default="rising"): Type of edge - "rising", "falling", or "both"
- `bit_index` (integer, optional): For vector signals, which bit to analyze (0 = LSB). Omit for scalar signals.
- `start_time` (integer, optional, default=0): Start of counting range
- `end_time` (integer, optional, default=end of simulation): End of counting range

**Returns:**
- Edge count and query parameters

**Example Response:**
```json
{
  "signal": "TOP.clk",
  "edge_type": "rising",
  "bit_index": null,
  "start_time": 0,
  "end_time": 10005,
  "count": 5002
}
```

## Why Use the Efficient Tools?

**Problem:** Traditional approaches that fetch full value streams for frequently-changing signals can produce **multi-megabyte payloads** and often get truncated by token limits.

**Solution:** The new efficient tools (`get_file_info`, `get_signal_summary`, `count_signal_edges`) provide:
- ✅ **Small responses** - typically < 1KB vs. potentially MBs
- ✅ **Fast server-side computation** - no need to transfer and process large datasets
- ✅ **Direct answers** - get counts, summaries, and metadata instantly
- ✅ **Better for AI agents** - easier to understand and act on concise data

**Workflow Example:**
1. `get_file_info` → Understand time range and scope
2. `get_signal_summary` → Check which signals are active
3. `count_signal_edges` → Count clock cycles or events
4. `get_signal_values` (with `limit`) → Only if you need specific value details

## Building

```bash
# Build the server
cargo build --release --package vcd-mcp

# The binary will be at: target/release/vcd-mcp
```

## Running

The server communicates via stdio (standard input/output):

```bash
./target/release/vcd-mcp
```

## Configuration for GitHub Copilot / VS Code

To use with GitHub Copilot, add this server to your MCP settings:

**For Claude Desktop / MCP clients:**

Edit your MCP settings file (typically `~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

```json
{
  "mcpServers": {
    "vcd-mcp": {
      "command": "/absolute/path/to/target/release/vcd-mcp"
    }
  }
}
```

**For VS Code with GitHub Copilot:**

Add to your VS Code settings (`.vscode/settings.json`):

```json
{
  "mcp.servers": {
    "vcd-mcp": {
      "command": "/absolute/path/to/target/release/vcd-mcp"
    }
  }
}
```

## Architecture

The server is built with:

- **`rmcp`**: MCP protocol implementation
- **`vcd`**: VCD file parsing
- **`tokio`**: Async runtime
- **`serde`/`serde_json`**: Serialization
- **`schemars`**: JSON schema generation for tool arguments

### Internal Structure

- **`domain.rs`**: Synchronous VCD parsing and query logic
- **`state.rs`**: Application state with LRU cache for parsed files
- **`tools.rs`**: Tool argument definitions with JSON schemas
- **`handlers.rs`**: Async handlers that bridge tools to domain logic
- **`main.rs`**: Server setup and entry point

### Performance

- VCD parsing is offloaded to blocking threads (`spawn_blocking`) to avoid blocking the async event loop
- Parsed VCD files are cached in memory for fast repeated queries
- Large files are handled efficiently with incremental value tracking

## Example Usage

Once configured with an MCP client, you can ask questions like:

**Basic Queries:**
- "Inspect the header of `/path/to/simulation.vcd`"
- "List all signals in the `top.cpu` module from `/path/to/simulation.vcd`"
- "Get the value of `top.cpu.pc` and `top.cpu.instruction` at time 1000 in `/path/to/simulation.vcd`"
- "Show me all changes to `top.clk` between time 0 and 5000"

**Efficient Queries (Recommended):**
- "Get file info for `/path/to/simulation.vcd`" - Start here to understand the file
- "Summarize signals `top.cpu.pc` and `top.cpu.state` from time 0 to 10000" - Quick overview
- "Count rising edges of `top.clk` signal" - Clock cycle counting
- "How many times did `top.cpu.instr_complete` signal go high?" - Event counting
- "Count transitions on bit 5 of `top.cpu.alu_flags` vector signal" - Per-bit analysis

**Efficient Data Extraction:**
- "Get signal values for `top.pc` with only_changes=true and limit=100" - Avoid huge payloads
- "Show me the first 50 changes to `top.state_machine` after time 1000"

## License

Same as the parent repository.
