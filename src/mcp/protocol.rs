use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Representa una herramienta (tool) que la IA puede invocar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPTool {
    pub name: String,
    pub description: String,
    pub parameters: Value, // JSON Schema
}

/// Request JSON-RPC 2.0
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPRequest {
    pub jsonrpc: String, // "2.0"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// Response JSON-RPC 2.0
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPResponse {
    pub jsonrpc: String, // "2.0"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<MCPError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Métodos estándar del protocolo MCP
pub mod methods {
    pub const LIST_TOOLS: &str = "tools/list";
    pub const CALL_TOOL: &str = "tools/call";
    pub const GET_TOOL_INFO: &str = "tools/info";
}
