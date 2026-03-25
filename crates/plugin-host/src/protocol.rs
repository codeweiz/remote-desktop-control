//! JSON-RPC 2.0 protocol types for plugin communication.
//!
//! Implements the JSON-RPC 2.0 specification for bidirectional communication
//! between the RTB host and plugin subprocesses over stdin/stdout.

use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 version string.
pub const JSONRPC_VERSION: &str = "2.0";

/// A JSON-RPC 2.0 request ID. Can be a number or a string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    String(String),
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestId::Number(n) => write!(f, "{n}"),
            RequestId::String(s) => write!(f, "{s}"),
        }
    }
}

/// A JSON-RPC 2.0 request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    pub id: RequestId,
}

impl JsonRpcRequest {
    /// Create a new JSON-RPC request.
    pub fn new(id: RequestId, method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
            id,
        }
    }
}

/// A JSON-RPC 2.0 successful response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: RequestId,
}

impl JsonRpcResponse {
    /// Create a successful response.
    pub fn success(id: RequestId, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response.
    pub fn error(id: RequestId, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: None,
            error: Some(error),
            id,
        }
    }

    /// Returns true if this is an error response.
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

/// Standard JSON-RPC 2.0 error codes.
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    // RTB-specific error codes (application-defined range: -32000 to -32099)
    pub const PLUGIN_NOT_READY: i32 = -32000;
    pub const PLUGIN_TIMEOUT: i32 = -32001;
    pub const PLUGIN_CRASHED: i32 = -32002;
    pub const MESSAGE_TOO_LARGE: i32 = -32003;
}

/// A JSON-RPC 2.0 notification (request without an id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcNotification {
    /// Create a new notification.
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC message that can be any of request, response, or notification.
/// Used for parsing incoming messages from the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Response(JsonRpcResponse),
    Notification(JsonRpcNotification),
}

impl JsonRpcMessage {
    /// Parse a JSON-RPC message from a JSON string.
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        // We need custom logic because serde untagged can be ambiguous.
        // A request has id + method, a response has id + (result|error),
        // a notification has method but no id.
        let value: serde_json::Value = serde_json::from_str(json)?;

        if let Some(obj) = value.as_object() {
            let has_id = obj.contains_key("id");
            let has_method = obj.contains_key("method");
            let has_result = obj.contains_key("result");
            let has_error = obj.contains_key("error");

            if has_id && has_method {
                let req: JsonRpcRequest = serde_json::from_value(value)?;
                return Ok(JsonRpcMessage::Request(req));
            }
            if has_id && (has_result || has_error) {
                let resp: JsonRpcResponse = serde_json::from_value(value)?;
                return Ok(JsonRpcMessage::Response(resp));
            }
            if has_method && !has_id {
                let notif: JsonRpcNotification = serde_json::from_value(value)?;
                return Ok(JsonRpcMessage::Notification(notif));
            }
        }

        // Fallback: try untagged deserialization
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_request() {
        let req = JsonRpcRequest::new(
            RequestId::Number(1),
            "initialize",
            Some(serde_json::json!({"version": "1.0"})),
        );
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"initialize\""));
    }

    #[test]
    fn test_parse_response() {
        let json = r#"{"jsonrpc":"2.0","result":{"ok":true},"id":1}"#;
        let msg = JsonRpcMessage::parse(json).unwrap();
        match msg {
            JsonRpcMessage::Response(resp) => {
                assert_eq!(resp.id, RequestId::Number(1));
                assert!(!resp.is_error());
            }
            _ => panic!("expected response"),
        }
    }

    #[test]
    fn test_parse_notification() {
        let json = r#"{"jsonrpc":"2.0","method":"on_message","params":{"text":"hello"}}"#;
        let msg = JsonRpcMessage::parse(json).unwrap();
        match msg {
            JsonRpcMessage::Notification(n) => {
                assert_eq!(n.method, "on_message");
            }
            _ => panic!("expected notification"),
        }
    }

    #[test]
    fn test_error_response() {
        let err = JsonRpcError::new(error_codes::METHOD_NOT_FOUND, "Method not found");
        let resp = JsonRpcResponse::error(RequestId::String("abc".into()), err);
        assert!(resp.is_error());
        assert_eq!(resp.error.as_ref().unwrap().code, -32601);
    }
}
