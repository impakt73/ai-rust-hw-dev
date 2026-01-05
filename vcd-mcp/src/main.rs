mod domain;
mod handlers;
mod state;
mod tools;

use rmcp::{
    handler::server::tool::ToolCallContext,
    model::{
        CallToolRequestParam, Implementation, PaginatedRequestParam, ProtocolVersion,
        ServerCapabilities, ServerInfo, ToolsCapability,
    },
    service::RequestContext,
    Error, RoleServer, ServerHandler,
};
use state::AppState;
use tools::{GetValuesArgs, InspectHeaderArgs, ListSignalsArgs};

#[derive(Debug, Clone, Default)]
pub struct VcdMcpServer {
    state: AppState,
}

impl VcdMcpServer {
    pub fn new() -> Self {
        Self {
            state: AppState::new(),
        }
    }

    /// Tool: inspect_vcd_header
    /// Reads the VCD header to extract metadata (timescale, date, version) and top-level modules.
    #[rmcp::tool(
        name = "inspect_vcd_header",
        description = "Reads the VCD header to extract metadata (timescale, date, version) and top-level modules. Used to validate the file and understand the scope."
    )]
    async fn inspect_vcd_header(&self, #[tool(param)] args: InspectHeaderArgs) -> String {
        match handlers::handle_inspect_header(self.state.clone(), args).await {
            Ok(result) => {
                serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    /// Tool: list_signals
    /// Returns a list of all signal names within the file or a specific scope.
    #[rmcp::tool(
        name = "list_signals",
        description = "Returns a list of all signal names within the file or a specific scope."
    )]
    async fn list_signals(&self, #[tool(param)] args: ListSignalsArgs) -> String {
        match handlers::handle_list_signals(self.state.clone(), args).await {
            Ok(result) => {
                serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    /// Tool: get_signal_values
    /// Retrieves the value of specific signals at a specific time or over a time range.
    #[rmcp::tool(
        name = "get_signal_values",
        description = "Retrieves the value of specific signals at a specific time or over a time range."
    )]
    async fn get_signal_values(&self, #[tool(param)] args: GetValuesArgs) -> String {
        match handlers::handle_get_values(self.state.clone(), args).await {
            Ok(result) => {
                serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }
}

impl ServerHandler for VcdMcpServer {
    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::CallToolResult, Error> {
        let tcc = ToolCallContext::new(self, request, context);
        match tcc.name() {
            "inspect_vcd_header" => Self::inspect_vcd_header_tool_call(tcc).await,
            "list_signals" => Self::list_signals_tool_call(tcc).await,
            "get_signal_values" => Self::get_signal_values_tool_call(tcc).await,
            _ => Err(Error::invalid_params("Unknown tool", None)),
        }
    }

    async fn list_tools(
        &self,
        _param: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, Error> {
        Ok(rmcp::model::ListToolsResult {
            next_cursor: None,
            tools: vec![
                Self::inspect_vcd_header_tool_attr(),
                Self::list_signals_tool_attr(),
                Self::get_signal_values_tool_attr(),
            ],
        })
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability { list_changed: None }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "vcd-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: None,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = VcdMcpServer::new();

    // Create stdio transport
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let transport = (stdin, stdout);

    rmcp::serve_server(server, transport).await?;

    Ok(())
}
