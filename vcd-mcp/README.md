# VCD MCP Server

An MCP (Model Context Protocol) server that provides tools for analyzing Value Change Dump (VCD) files. Designed for integration with GitHub Copilot and other MCP clients.

## Features

The server exposes three tools for VCD analysis:

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

**Returns:**
- Array of signal values with timestamps
- Warning if any signals were not found

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

- "Inspect the header of `/path/to/simulation.vcd`"
- "List all signals in the `top.cpu` module from `/path/to/simulation.vcd`"
- "Get the value of `top.cpu.pc` and `top.cpu.instruction` at time 1000 in `/path/to/simulation.vcd`"
- "Show me all changes to `top.clk` between time 0 and 5000"

## License

Same as the parent repository.
