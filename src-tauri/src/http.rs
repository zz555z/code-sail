use reqwest::Client;
use std::sync::OnceLock;
use std::time::Duration;

/// Shared HTTP client for health checks and model fetching.
/// Uses OnceLock to ensure the client is created only once and reused across requests.
static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

/// Get or initialize the shared HTTP client with default timeouts.
/// Connect timeout: 8 seconds, Request timeout: 30 seconds.
pub fn shared_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build shared HTTP client")
    })
}

/// Get or initialize the shared HTTP client with shorter timeouts for health checks.
/// Connect timeout: 5 seconds, Request timeout: 10 seconds.
pub fn health_check_client() -> &'static Client {
    static HEALTH_CLIENT: OnceLock<Client> = OnceLock::new();
    HEALTH_CLIENT.get_or_init(|| {
        Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build health check HTTP client")
    })
}
