use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{DeliveryOrderNormalized, DeliveryProvider};
use serde::Serialize;

pub mod just_eat;
pub mod deliveroo;
pub mod uber_eats;

#[derive(Debug)]
pub enum ConnectorError {
    Http(String),
    Auth(String),
    InvalidConfig(String),
    Other(String),
}

#[derive(Debug, Serialize)]
pub struct TestResult {
    pub ok: bool,
    pub message: String,
}

/// How incoming webhooks for this provider should be verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebhookVerificationStrategy {
    /// Traqr's own shared-secret HMAC over the raw request body, hex-encoded.
    TraqrHmacSha256Hex,
    /// Deliveroo: HMAC-SHA256(webhook_secret, sequence_guid + " " + raw_body), hex in X-Deliveroo-Hmac-Sha256.
    DeliverooHmacSha256,
    /// Uber Eats: HMAC-SHA256(client_secret, raw_body), lowercase hex in X-Uber-Signature.
    UberEatsHmacSha256Hex,
    /// Use the provider's official verification scheme (timestamped signatures, JWTs, etc.).
    ProviderOfficial,
    /// No signature verification (only for providers that cannot sign webhooks).
    None,
}

#[derive(Debug, Clone)]
pub struct DeliveryIntegrationConfig {
    pub org_id: String,
    pub store_id: String,
    pub provider: DeliveryProvider,
    pub api_key: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub webhook_secret: Option<String>,
    pub provider_store_reference: Option<String>,
}

#[async_trait]
pub trait DeliveryConnector: Send + Sync {
    async fn test_connection(
        &self,
        config: &DeliveryIntegrationConfig,
    ) -> Result<TestResult, ConnectorError>;

    async fn register_webhook(
        &self,
        config: &DeliveryIntegrationConfig,
        callback_url: &str,
    ) -> Result<(), ConnectorError>;

    /// Which webhook verification strategy should be applied for this provider.
    /// Default: Traqr's internal HMAC scheme.
    fn webhook_verification_strategy(&self) -> WebhookVerificationStrategy {
        WebhookVerificationStrategy::TraqrHmacSha256Hex
    }

    async fn refresh_token_if_needed(
        &self,
        _config: &mut DeliveryIntegrationConfig,
    ) -> Result<(), ConnectorError> {
        Ok(())
    }

    async fn fetch_orders(
        &self,
        _config: &DeliveryIntegrationConfig,
        _since: Option<DateTime<Utc>>,
    ) -> Result<Vec<DeliveryOrderNormalized>, ConnectorError> {
        Ok(vec![])
    }
}

pub fn connector_for(provider: &DeliveryProvider) -> Box<dyn DeliveryConnector> {
    match provider {
        DeliveryProvider::JustEat => Box::new(just_eat::JustEatConnector),
        DeliveryProvider::Deliveroo => Box::new(deliveroo::DeliverooConnector),
        DeliveryProvider::UberEats => Box::new(uber_eats::UberEatsConnector),
    }
}

