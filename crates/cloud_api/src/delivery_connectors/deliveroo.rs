use super::{
    ConnectorError, DeliveryConnector, DeliveryIntegrationConfig, TestResult,
    WebhookVerificationStrategy,
};
use async_trait::async_trait;

pub struct DeliverooConnector;

#[async_trait]
impl DeliveryConnector for DeliverooConnector {
    async fn test_connection(
        &self,
        _config: &DeliveryIntegrationConfig,
    ) -> Result<TestResult, ConnectorError> {
        // TODO: Call Deliveroo API with provided credentials and verify access.
        Ok(TestResult {
            ok: true,
            message: "Deliveroo connector stubbed OK".to_string(),
        })
    }

    async fn register_webhook(
        &self,
        _config: &DeliveryIntegrationConfig,
        _callback_url: &str,
    ) -> Result<(), ConnectorError> {
        // Webhook URL is configured in Deliveroo Developer Portal.
        Ok(())
    }

    fn webhook_verification_strategy(&self) -> WebhookVerificationStrategy {
        WebhookVerificationStrategy::DeliverooHmacSha256
    }
}

