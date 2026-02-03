use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivateDeviceRequest {
    pub local_device_id: String,
    pub activation_key: String,
    pub store_hint: Option<Uuid>,
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
