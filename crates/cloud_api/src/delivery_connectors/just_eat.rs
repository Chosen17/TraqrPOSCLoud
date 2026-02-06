use super::{
    ConnectorError, DeliveryConnector, DeliveryIntegrationConfig, TestResult,
    WebhookVerificationStrategy,
};
use async_trait::async_trait;

pub struct JustEatConnector;

#[async_trait]
impl DeliveryConnector for JustEatConnector {
    async fn test_connection(
        &self,
        config: &DeliveryIntegrationConfig,
    ) -> Result<TestResult, ConnectorError> {
        // Use the JE-API-KEY against a configurable Just Eat endpoint.
        // This avoids hard-coding a specific region or path; set JUST_EAT_TEST_URL
        // in the environment to enable a real round-trip test.
        let api_key = config
            .api_key
            .as_deref()
            .ok_or_else(|| ConnectorError::InvalidConfig("Just Eat API key is required".to_string()))?;

        let url = match std::env::var("JUST_EAT_TEST_URL") {
            Ok(u) if !u.is_empty() => u,
            _ => {
                // Credentials are stored and will be used for webhooks/order callbacks,
                // but no live HTTP test is configured.
                return Ok(TestResult {
                    ok: true,
                    message: "Just Eat API key stored; set JUST_EAT_TEST_URL to enable live connection test".to_string(),
                });
            }
        };

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .header("JE-API-KEY", api_key)
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ConnectorError::Auth(format!(
                "Just Eat test failed with HTTP status {}",
                resp.status()
            )));
        }

        Ok(TestResult {
            ok: true,
            message: "Just Eat API key validated against test endpoint".to_string(),
        })
    }

    async fn register_webhook(
        &self,
        _config: &DeliveryIntegrationConfig,
        _callback_url: &str,
    ) -> Result<(), ConnectorError> {
        // Webhook URL is typically configured in the Just Eat partner portal.
        Ok(())
    }

    fn webhook_verification_strategy(&self) -> WebhookVerificationStrategy {
        // Until JET Connect signature docs are finalized, do not enforce HMAC verification
        // to avoid rejecting valid callbacks.
        WebhookVerificationStrategy::None
    }
}
