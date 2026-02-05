use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivateDeviceRequest {
    pub local_device_id: String,
    pub activation_key: String,
    pub store_hint: Option<Uuid>,
    /// Display name for this device (e.g. "Till 1", "Kitchen screen"). From POS Setup.
    #[serde(default)]
    pub device_name: Option<String>,
    /// True if this device is the primary (authority) for the store.
    #[serde(default)]
    pub is_primary: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivateDeviceResponse {
    pub device_id: Uuid,
    pub org_id: Uuid,
    pub store_id: Uuid,
    pub device_token: String, // returned once
    pub polling_interval_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceEventIn {
    pub event_id: Uuid,
    pub seq: Option<i64>,
    pub event_type: String,
    pub occurred_at: String,
    pub event_body: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEventsRequest {
    pub last_ack_seq: Option<i64>,
    pub events: Vec<DeviceEventIn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEventsResponse {
    pub ack_seq: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCommandOut {
    pub command_id: Uuid,
    pub command_type: String,
    pub sensitive: bool,
    pub command_body: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCommandsResponse {
    pub commands: Vec<DeviceCommandOut>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandAckRequest {
    pub command_id: Uuid,
    pub status: String, // "acked" | "failed"
    pub result: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub ok: bool,
    pub message: String,
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

/// Request to create an activation key. Either (org_id + store_id) or (org_name + org_slug + store_name).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateActivationKeyRequest {
    /// Use existing org (with store_id).
    pub org_id: Option<Uuid>,
    /// Use existing store; required if org_id set. For scope_type "store", scope_id will equal this.
    pub store_id: Option<Uuid>,
    /// Create org by name (with org_slug and store_name).
    pub org_name: Option<String>,
    pub org_slug: Option<String>,
    pub store_name: Option<String>,
    /// Scope: "store" | "franchise" | "org". For "store", scope_id must be the store_id.
    pub scope_type: String,
    pub scope_id: Option<Uuid>,
    pub max_uses: Option<i32>,
    /// RFC3339 or null for no expiry.
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateActivationKeyResponse {
    /// Raw key — show once to operator; never stored. POS uses this in Settings → Cloud.
    pub activation_key: String,
    pub key_id: Uuid,
    pub org_id: Uuid,
    pub store_id: Uuid,
    pub scope_type: String,
    pub scope_id: Option<Uuid>,
    pub max_uses: Option<i32>,
    pub expires_at: Option<String>,
}
