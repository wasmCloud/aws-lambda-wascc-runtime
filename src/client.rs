//
// waSCC AWS Lambda Runtime Provider
//

use reqwest::StatusCode;
use std::error::Error;

// Represents an invocation event.
pub struct LambdaInvocationEvent {
    body: Vec<u8>,
    trace_id: String,
}

// Represents an AWS Lambda runtime HTTP client.
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
    pub fn next_invocation_event(&self) -> Result<Option<LambdaInvocationEvent>, Box<dyn Error>> {
        // https://docs.aws.amazon.com/lambda/latest/dg/runtimes-api.html#runtimes-api-next
        let url = format!("http://{}/2018-06-01/runtime/invocation/next", self.endpoint);
        let mut resp = self.http_client.get(&url).send()?;
        let status = resp.status();
        info!("GET {} {} {}", url, status.as_str(), status.canonical_reason().unwrap());
        if !status.is_success() {
            return Ok(None);
        }

        let mut event = LambdaInvocationEvent{
            body: vec![],
            trace_id: String::new(),
        };

        if let Some(trace_id) = resp.headers().get("Lambda-Runtime-Trace-Id") {
            event.trace_id = trace_id.to_str()?.into();
        }

        let mut buf: Vec<u8> = vec![];
        resp.copy_to(&mut buf)?;
        event.body = buf;

        Ok(Some(event))
    }
}
