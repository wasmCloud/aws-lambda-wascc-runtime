// Copyright 2015-2020 Capital One Services, LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//
// waSCC AWS Lambda Runtime Provider
//

use serde_json;
use std::error::Error;

// Represents an invocation event.
pub struct InvocationEvent {
    body: Vec<u8>,
    request_id: Option<String>,
    trace_id: Option<String>,
}

// Represents an invocation response.
pub struct InvocationResponse {
    body: Vec<u8>,
    request_id: Option<String>,
}

// Represents an invocation error.
pub struct InvocationError {
    error: Box<dyn Error>,
    request_id: Option<String>,
}

// Represents an AWS Lambda runtime HTTP client.
pub struct RuntimeClient {
    endpoint: String,
    http_client: reqwest::blocking::Client,
}

impl RuntimeClient {
    // Creates a new `RuntimeClient` with the specified AWS Lambda runtime API endpoint.
    pub fn new(endpoint: &str) -> Self {
        RuntimeClient {
            endpoint: endpoint.into(),
            http_client: reqwest::blocking::Client::new(),
        }
    }

    // Returns the next AWS Lambda invocation event.
    pub fn next_invocation_event(&self) -> Result<Option<InvocationEvent>, Box<dyn Error>> {
        // https://docs.aws.amazon.com/lambda/latest/dg/runtimes-api.html#runtimes-api-next
        let url = format!(
            "http://{}/2018-06-01/runtime/invocation/next",
            self.endpoint
        );
        let mut resp = self.http_client.get(&url).send()?;
        let status = resp.status();
        info!(
            "GET {} {} {}",
            url,
            status.as_str(),
            status.canonical_reason().unwrap()
        );
        if !status.is_success() {
            return Ok(None);
        }

        let mut buf: Vec<u8> = vec![];
        resp.copy_to(&mut buf)?;
        let mut event = InvocationEvent::new(buf);
        if let Some(request_id) = resp.headers().get("Lambda-Runtime-Aws-Request-Id") {
            event.request_id = Some(request_id.to_str()?.into());
        }
        if let Some(trace_id) = resp.headers().get("Lambda-Runtime-Trace-Id") {
            event.trace_id = Some(trace_id.to_str()?.into());
        }

        Ok(Some(event))
    }

    // Sends an invocation error to the AWS Lambda runtime.
    pub fn send_invocation_error(&self, error: InvocationError) -> Result<(), Box<dyn Error>> {
        if error.request_id.is_none() {
            return Err("No request ID specified".into());
        }

        // https://docs.aws.amazon.com/lambda/latest/dg/runtimes-api.html#runtimes-api-invokeerror
        let url = format!(
            "http://{}/2018-06-01/runtime/invocation/{}/error",
            self.endpoint,
            error.request_id.unwrap()
        );
        let resp = self
            .http_client
            .post(&url)
            .json(&serde_json::json!({
                "errorMessage": error.error.to_string(),
            }))
            .send()?;
        let status = resp.status();
        info!(
            "POST {} {} {}",
            url,
            status.as_str(),
            status.canonical_reason().unwrap()
        );

        Ok(())
    }

    // Sends an invocation response to the AWS Lambda runtime.
    pub fn send_invocation_response(&self, resp: InvocationResponse) -> Result<(), Box<dyn Error>> {
        if resp.request_id.is_none() {
            return Err("No request ID specified".into());
        }

        // https://docs.aws.amazon.com/lambda/latest/dg/runtimes-api.html#runtimes-api-response
        let url = format!(
            "http://{}/2018-06-01/runtime/invocation/{}/response",
            self.endpoint,
            resp.request_id.unwrap()
        );
        let resp = self.http_client.post(&url).body(resp.body).send()?;
        let status = resp.status();
        info!(
            "POST {} {} {}",
            url,
            status.as_str(),
            status.canonical_reason().unwrap()
        );

        Ok(())
    }
}

impl InvocationEvent {
    // Creates a new `InvocationEvent` with the specified body.
    pub fn new(body: Vec<u8>) -> Self {
        InvocationEvent {
            body: body,
            request_id: None,
            trace_id: None,
        }
    }

    // Returns the event body.
    pub fn body(&self) -> &Vec<u8> {
        self.body.as_ref()
    }

    // Returns any request ID.
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }

    // Returns any trace ID.
    pub fn trace_id(&self) -> Option<&str> {
        self.trace_id.as_deref()
    }
}

impl InvocationResponse {
    // Creates a new `InvocationResponse` with the specified body.
    pub fn new(body: Vec<u8>) -> Self {
        InvocationResponse {
            body: body,
            request_id: None,
        }
    }

    // Creates a new `InvocationResponse` with the specified request ID.
    pub fn request_id(self, request_id: &str) -> Self {
        InvocationResponse {
            body: self.body,
            request_id: Some(request_id.into()),
        }
    }
}

impl InvocationError {
    // Creates a new `InvocationError` with the specified error.
    pub fn new(error: Box<dyn Error>) -> Self {
        InvocationError {
            error: error,
            request_id: None,
        }
    }

    // Creates a new `InvocationError` with the specified request ID.
    pub fn request_id(self, request_id: &str) -> Self {
        InvocationError {
            error: self.error,
            request_id: Some(request_id.into()),
        }
    }
}
