//
// waSCC AWS Lambda Runtime Provider
//

use reqwest::StatusCode;
use std::error::Error;

pub struct LambdaRuntimeClient {
    endpoint: String,
    http_client: reqwest::blocking::Client,
}

impl LambdaRuntimeClient {
    // Creates a new `LambdaRuntimeClient` with the specified AWS Lambda runtime API endpoint.
    pub fn with_endpoint(endpoint: &str) -> Self {
        LambdaRuntimeClient {
            endpoint: endpoint.into(),
            http_client: reqwest::blocking::Client::new(),
        }
    }

    // Returns the next AWS Lambda invocation event.
    pub fn next_invocation_event(&self) -> Result<(), Box<dyn Error>> {
        let url = format!("http://{}/2018-06-01/runtime/invocation/next", self.endpoint);
        let resp = self.http_client.get(&url).send()?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("Erro from {}: {} {}", url, status.as_str(), status.canonical_reason().unwrap()).into());
        }

        Ok(())
    }
}
