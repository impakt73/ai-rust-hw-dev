mod domain;
mod handlers;
mod state;
mod tools;

use rmcp::{
    handler::server::wrapper::Parameters, model::*, tool, tool_handler, tool_router,
    transport::stdio, ErrorData as McpError, ServerHandler, ServiceExt,
};
use state::AppState;
use tools::{GetValuesArgs, InspectHeaderArgs, ListSignalsArgs};

#[derive(Debug, Clone)]
pub struct VcdMcpServer {
    state: AppState,
    tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
}

impl Default for VcdMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl VcdMcpServer {
    pub fn new() -> Self {
        Self {
            state: AppState::new(),
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl VcdMcpServer {
    #[tool(
        description = "Reads the VCD header to extract metadata (timescale, date, version) and top-level modules. Used to validate the file and understand the scope."
    )]
    async fn inspect_vcd_header(
        &self,
        Parameters(args): Parameters<InspectHeaderArgs>,
    ) -> Result<String, McpError> {
        match handlers::handle_inspect_header(self.state.clone(), args).await {
            Ok(result) => {
                let text =
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
                Ok(text)
            }
            Err(e) => Err(McpError::internal_error(format!("Error: {}", e), None)),
        }
    }

    #[tool(description = "Returns a list of all signal names within the file or a specific scope.")]
    async fn list_signals(
        &self,
        Parameters(args): Parameters<ListSignalsArgs>,
    ) -> Result<String, McpError> {
        match handlers::handle_list_signals(self.state.clone(), args).await {
            Ok(result) => {
                let text =
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
                Ok(text)
            }
            Err(e) => Err(McpError::internal_error(format!("Error: {}", e), None)),
        }
    }

    #[tool(
        description = "Retrieves the value of specific signals at a specific time or over a time range."
    )]
    async fn get_signal_values(
        &self,
        Parameters(args): Parameters<GetValuesArgs>,
    ) -> Result<String, McpError> {
        match handlers::handle_get_values(self.state.clone(), args).await {
            Ok(result) => {
                let text =
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
                Ok(text)
            }
            Err(e) => Err(McpError::internal_error(format!("Error: {}", e), None)),
        }
    }
}

#[tool_handler]
impl ServerHandler for VcdMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("VCD MCP Server - Analyze VCD (Value Change Dump) files".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = VcdMcpServer::new();
    let service = server.serve(stdio()).await.inspect_err(|e| {
        eprintln!("Server error: {:?}", e);
    })?;

    service.waiting().await?;
    Ok(())
}
