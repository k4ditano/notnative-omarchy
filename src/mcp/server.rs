use anyhow::Result;
use axum::{
    Router,
    extract::State,
    http::Method,
    response::Json,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tower_http::cors::{Any, CorsLayer};

use crate::core::database::NotesDatabase;
use crate::core::note_file::NotesDirectory;
use crate::mcp::{MCPToolCall, MCPToolExecutor};

/// Señaliza cambios en las notas para que la UI se actualice
fn signal_notes_changed() {
    let signal_path = std::env::temp_dir().join("notnative_mcp_update.signal");
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let _ = fs::write(&signal_path, timestamp.to_string());
}

/// Estado compartido del servidor MCP (thread-safe)
#[derive(Clone)]
pub struct MCPServerState {
    notes_dir: NotesDirectory,
    notes_db: Arc<Mutex<NotesDatabase>>,
}

/// Request para listar herramientas
#[derive(Debug, Deserialize)]
pub struct ListToolsRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
}

/// Request para llamar una herramienta
#[derive(Debug, Deserialize)]
pub struct CallToolRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: CallToolParams,
}

#[derive(Debug, Deserialize)]
pub struct CallToolParams {
    pub tool: String,
    pub args: Value,
}

/// Response JSON-RPC genérico
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

/// Respuesta de list_tools
#[derive(Debug, Serialize)]
pub struct ListToolsResponse {
    pub tools: Vec<Value>,
}

/// Inicia el servidor MCP en segundo plano
pub async fn start_mcp_server(
    notes_dir: NotesDirectory,
    notes_db: Arc<Mutex<NotesDatabase>>,
) -> Result<()> {
    let state = MCPServerState {
        notes_dir,
        notes_db,
    };

    // Configurar CORS para permitir requests desde cualquier origen
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/mcp/list_tools", post(list_tools))
        .route("/mcp/call_tool", post(call_tool))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8788));
    println!("🚀 Servidor MCP escuchando en http://{}", addr);
    println!("   - GET  /health");
    println!("   - POST /mcp/list_tools");
    println!("   - POST /mcp/call_tool");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Health check endpoint
async fn health_check() -> Json<Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "NotNative MCP Server",
        "version": "1.0.0"
    }))
}

/// Lista todas las herramientas disponibles
async fn list_tools(
    State(state): State<MCPServerState>,
    Json(request): Json<ListToolsRequest>,
) -> Json<JsonRpcResponse<ListToolsResponse>> {
    let tools = crate::mcp::tool_schemas::get_core_tool_definitions();

    Json(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: request.id,
        result: Some(ListToolsResponse { tools }),
        error: None,
    })
}

/// Ejecuta una herramienta específica
async fn call_tool(
    State(state): State<MCPServerState>,
    Json(request): Json<CallToolRequest>,
) -> Json<JsonRpcResponse<Value>> {
    // Crear executor (necesitamos convertir Arc<Mutex> a Rc<RefCell> temporalmente)
    // Por simplicidad, clonamos la DB para evitar problemas de lifetime
    let notes_db_clone = {
        let db = state.notes_db.lock().unwrap();
        db.clone_connection()
    };

    let executor = MCPToolExecutor::new(
        state.notes_dir.clone(),
        std::rc::Rc::new(std::cell::RefCell::new(notes_db_clone)),
    );

    // Intentar parsear la llamada a herramienta
    let tool_call_json = serde_json::json!({
        "tool": request.params.tool,
        "args": request.params.args
    });

    match serde_json::from_value::<MCPToolCall>(tool_call_json) {
        Ok(tool_call) => {
            // Verificar si es una herramienta que modifica archivos
            let modifies_files = matches!(
                tool_call,
                MCPToolCall::CreateNote { .. }
                    | MCPToolCall::UpdateNote { .. }
                    | MCPToolCall::AppendToNote { .. }
                    | MCPToolCall::DeleteNote { .. }
                    | MCPToolCall::RenameNote { .. }
                    | MCPToolCall::DuplicateNote { .. }
                    | MCPToolCall::MoveNote { .. }
                    | MCPToolCall::CreateFolder { .. }
            );

            // Ejecutar la herramienta
            match executor.execute(tool_call) {
                Ok(result) => {
                    // Si modificó archivos, señalizar cambio
                    if modifies_files && result.success {
                        signal_notes_changed();
                    }

                    Json(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: Some(serde_json::to_value(result).unwrap_or(serde_json::json!({}))),
                        error: None,
                    })
                }
                Err(e) => Json(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32603,
                        message: format!("Error ejecutando herramienta: {}", e),
                    }),
                }),
            }
        }
        Err(e) => Json(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: None,
            error: Some(JsonRpcError {
                code: -32602,
                message: format!("Parámetros inválidos: {}", e),
            }),
        }),
    }
}
