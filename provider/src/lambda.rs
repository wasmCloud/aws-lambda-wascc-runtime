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

use reqwest::header::USER_AGENT;

const REQUEST_ID_HEADER_NAME: &str = "Lambda-Runtime-Aws-Request-Id";
const TRACE_ID_HEADER_NAME: &str = "Lambda-Runtime-Trace-Id";

/// Represents an AWS Lambda runtime client.
pub(crate) trait Client {
    /// Returns the next AWS Lambda invocation event.
    fn next_invocation_event(&self) -> anyhow::Result<Option<InvocationEvent>>;

    /// Sends an invocation error to the AWS Lambda runtime.
    fn send_invocation_error(&self, error: InvocationError) -> anyhow::Result<()>;

    /// Sends an invocation error to the AWS Lambda runtime.
    fn send_invocation_response(&self, resp: InvocationResponse) -> anyhow::Result<()>;

    /// Sends an initialization error to the AWS Lambda runtime.
    fn send_initialization_error(&self, error: InitializationError) -> anyhow::Result<()>;
}

/// Represents an AWS Lambda initialization error reporter.
pub trait InitializationErrorReporter {
    /// Sends an initialization error to the AWS Lambda runtime.
    fn send_initialization_error(&self, error: anyhow::Error) -> anyhow::Result<()>;
}

/// Represents an AWS Lambda runtime client.
pub(crate) struct RuntimeClient {
    endpoint: String,
    http_client: reqwest::blocking::Client,
    user_agent: String,
}

impl RuntimeClient {
    /// Creates a new `RuntimeClient` with the specified AWS Lambda runtime API endpoint.
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.into(),
            http_client: reqwest::blocking::Client::new(),
            user_agent: format!("AWS_Lambda_waSCC/{}", env!("CARGO_PKG_VERSION")),
        }
    }

    fn next_invocation_event_path() -> String {
        "2018-06-01/runtime/invocation/next".into()
    }

    fn invocation_error_path(request_id: &str) -> String {
        format!("2018-06-01/runtime/invocation/{}/error", request_id)
    }

    fn invocation_response_path(request_id: &str) -> String {
        format!("2018-06-01/runtime/invocation/{}/response", request_id)
    }

    fn initialization_error_path() -> String {
        "2018-06-01/runtime/init/error".into()
    }
}

impl Client for RuntimeClient {
    /// Returns the next AWS Lambda invocation event.
    fn next_invocation_event(&self) -> anyhow::Result<Option<InvocationEvent>> {
        // https://docs.aws.amazon.com/lambda/latest/dg/runtimes-api.html#runtimes-api-next
        let url = format!("{}/{}", self.endpoint, Self::next_invocation_event_path());
        let mut resp = self
            .http_client
            .get(&url)
            .header(USER_AGENT, self.user_agent.clone())
            .send()?;
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

        let mut builder = InvocationEventBuilder::new(buf);
        if let Some(request_id) = resp.headers().get(REQUEST_ID_HEADER_NAME) {
            builder = builder.request_id(request_id.to_str()?);
        }
        if let Some(trace_id) = resp.headers().get(TRACE_ID_HEADER_NAME) {
            builder = builder.trace_id(trace_id.to_str()?);
        }

        Ok(Some(builder.build()))
    }

    /// Sends an invocation error to the AWS Lambda runtime.
    fn send_invocation_error(&self, error: InvocationError) -> anyhow::Result<()> {
        // https://docs.aws.amazon.com/lambda/latest/dg/runtimes-api.html#runtimes-api-invokeerror
        let url = format!(
            "{}/{}",
            self.endpoint,
            Self::invocation_error_path(error.request_id())
        );
        let resp = self
            .http_client
            .post(&url)
            .header(USER_AGENT, self.user_agent.clone())
            .json(&serde_json::json!({
                "errorMessage": error.error_message(),
                "errorType": "InvocationError",
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

    /// Sends an invocation response to the AWS Lambda runtime.
    fn send_invocation_response(&self, resp: InvocationResponse) -> anyhow::Result<()> {
        // https://docs.aws.amazon.com/lambda/latest/dg/runtimes-api.html#runtimes-api-response
        let url = format!(
            "{}/{}",
            self.endpoint,
            Self::invocation_response_path(resp.request_id())
        );
        let resp = self
            .http_client
            .post(&url)
            .header(USER_AGENT, self.user_agent.clone())
            .body(resp.body().to_owned())
            .send()?;
        let status = resp.status();
        info!(
            "POST {} {} {}",
            url,
            status.as_str(),
            status.canonical_reason().unwrap_or("Unknown")
        );

        Ok(())
    }

    /// Sends an initialization error to the AWS Lambda runtime.
    fn send_initialization_error(&self, error: InitializationError) -> anyhow::Result<()> {
        // https://docs.aws.amazon.com/lambda/latest/dg/runtimes-api.html#runtimes-api-initerror
        let url = format!("{}/{}", self.endpoint, Self::initialization_error_path());
        let resp = self
            .http_client
            .post(&url)
            .header(USER_AGENT, self.user_agent.clone())
            .json(&serde_json::json!({
                "errorMessage": error.error_message(),
                "errorType": "InitializationError",
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
}

impl InitializationErrorReporter for RuntimeClient {
    /// Sends an initialization error to the AWS Lambda runtime.
    fn send_initialization_error(&self, error: anyhow::Error) -> anyhow::Result<()> {
        <Self as Client>::send_initialization_error(&self, InitializationError::new(error))
    }
}

/// Returns a new `InitializationErrorReporter` implementation.
pub fn initerr_reporter(endpoint: &str) -> Box<dyn InitializationErrorReporter> {
    Box::new(RuntimeClient::new(endpoint))
}

/// Represents an invocation event.
#[derive(Clone)]
pub(crate) struct InvocationEvent {
    body: Vec<u8>,
    request_id: Option<String>,
    trace_id: Option<String>,
}

impl InvocationEvent {
    /// Returns the event body.
    pub fn body(&self) -> &Vec<u8> {
        self.body.as_ref()
    }

    /// Returns any request ID.
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }

    /// Returns any trace ID.
    pub fn trace_id(&self) -> Option<&str> {
        self.trace_id.as_deref()
    }
}

/// Builds an `InvocationEvent`.
pub(crate) struct InvocationEventBuilder {
    body: Vec<u8>,
    request_id: Option<String>,
    trace_id: Option<String>,
}

impl InvocationEventBuilder {
    /// Creates a new `InvocationEventBuilder` with the specified body.
    pub fn new(body: Vec<u8>) -> Self {
        Self {
            body,
            request_id: None,
            trace_id: None,
        }
    }

    /// Adds the specified request ID to the builder.
    pub fn request_id(mut self, request_id: &str) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Adds the specified trace ID to the builder.
    pub fn trace_id(mut self, trace_id: &str) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    /// Builds an `InvocationEvent`, consuming this builder.
    pub fn build(self) -> InvocationEvent {
        InvocationEvent {
            body: self.body,
            request_id: self.request_id,
            trace_id: self.trace_id,
        }
    }
}

/// Represents an invocation response.
pub(crate) struct InvocationResponse {
    body: Vec<u8>,
    request_id: String,
}

impl InvocationResponse {
    /// Creates a new `InvocationResponse` with the specified body and request ID.
    pub fn new(body: Vec<u8>, request_id: &str) -> Self {
        Self {
            body,
            request_id: request_id.into(),
        }
    }

    /// Returns the body.
    pub fn body(&self) -> &[u8] {
        self.body.as_slice()
    }

    /// Returns the request ID.
    pub fn request_id(&self) -> &str {
        self.request_id.as_str()
    }
}

/// Represents an invocation error.
pub(crate) struct InvocationError {
    error: anyhow::Error,
    request_id: String,
}

impl InvocationError {
    /// Creates a new `InvocationError` with the specified error and request ID.
    pub fn new(error: anyhow::Error, request_id: &str) -> Self {
        Self {
            error,
            request_id: request_id.into(),
        }
    }

    /// Returns the error message.
    pub fn error_message(&self) -> String {
        self.error.to_string()
    }

    /// Returns the request ID.
    pub fn request_id(&self) -> &str {
        self.request_id.as_str()
    }
}

/// Represents an initialization error.
pub(crate) struct InitializationError {
    error: anyhow::Error,
}

impl InitializationError {
    /// Creates a new `InitializationError` with the specified error.
    pub fn new(error: anyhow::Error) -> Self {
        Self { error }
    }

    /// Returns the error message.
    pub fn error_message(&self) -> String {
        self.error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_common::*;
    use httptest::{matchers::*, responders::*, Expectation, Server};

    /// Returns the server's endpoint.
    fn endpoint(server: &Server) -> String {
        let ep = server.url_str("");
        ep.trim_end_matches('/').into()
    }

    #[test]
    fn runtime_client_next_invocation_event_error() {
        let server = Server::run();
        server.expect(
            Expectation::matching(all_of![
                request::method("GET"),
                request::path(format!("/{}", RuntimeClient::next_invocation_event_path()))
            ])
            .respond_with(status_code(400)),
        );

        let client = RuntimeClient::new(&endpoint(&server));
        let result = client.next_invocation_event();
        assert!(result.is_ok());
        let event = result.unwrap();
        assert!(event.is_none());
    }

    #[test]
    fn runtime_client_next_invocation_event_empty() {
        let server = Server::run();
        server.expect(
            Expectation::matching(all_of![
                request::method("GET"),
                request::path(format!("/{}", RuntimeClient::next_invocation_event_path()))
            ])
            .respond_with(status_code(200)),
        );

        let client = RuntimeClient::new(&endpoint(&server));
        let result = client.next_invocation_event();
        assert!(result.is_ok());
        let event = result.unwrap();
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(0, event.body().len());
        assert!(event.request_id().is_none());
        assert!(event.trace_id().is_none());
    }

    #[test]
    fn runtime_client_next_invocation_event_with_body() {
        let server = Server::run();
        server.expect(
            Expectation::matching(all_of![
                request::method("GET"),
                request::path(format!("/{}", RuntimeClient::next_invocation_event_path()))
            ])
            .respond_with(status_code(200).body(EVENT_BODY)),
        );

        let client = RuntimeClient::new(&endpoint(&server));
        let result = client.next_invocation_event();
        assert!(result.is_ok());
        let event = result.unwrap();
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(EVENT_BODY, event.body().as_slice());
        assert!(event.request_id().is_none());
        assert!(event.trace_id().is_none());
    }

    #[test]
    fn runtime_client_next_invocation_event_with_body_and_request_and_trace_ids() {
        let server = Server::run();
        server.expect(
            Expectation::matching(all_of![
                request::method("GET"),
                request::path(format!("/{}", RuntimeClient::next_invocation_event_path()))
            ])
            .respond_with(
                status_code(200)
                    .body(EVENT_BODY)
                    .append_header(REQUEST_ID_HEADER_NAME, REQUEST_ID)
                    .append_header(TRACE_ID_HEADER_NAME, TRACE_ID),
            ),
        );

        let client = RuntimeClient::new(&endpoint(&server));
        let result = client.next_invocation_event();
        assert!(result.is_ok());
        let event = result.unwrap();
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(EVENT_BODY, event.body().as_slice());
        assert!(event.request_id().is_some());
        assert_eq!(REQUEST_ID, event.request_id().unwrap());
        assert!(event.trace_id().is_some());
        assert_eq!(TRACE_ID, event.trace_id().unwrap());
    }

    #[test]
    fn runtime_client_send_invocation_response() {
        let server = Server::run();
        server.expect(
            Expectation::matching(all_of![
                request::method("POST"),
                request::path(format!(
                    "/{}",
                    RuntimeClient::invocation_response_path(REQUEST_ID)
                )),
                request::body(RESPONSE_BODY),
            ])
            .respond_with(status_code(200)),
        );

        let client = RuntimeClient::new(&endpoint(&server));
        let result = client
            .send_invocation_response(InvocationResponse::new(RESPONSE_BODY.to_vec(), REQUEST_ID));
        assert!(result.is_ok());
    }

    #[test]
    fn runtime_client_send_invocation_error() {
        let server = Server::run();
        server.expect(
            Expectation::matching(all_of![
                request::method("POST"),
                request::path(format!(
                    "/{}",
                    RuntimeClient::invocation_error_path(REQUEST_ID)
                )),
            ])
            .respond_with(status_code(200)),
        );

        let client = RuntimeClient::new(&endpoint(&server));
        let result =
            client.send_invocation_error(InvocationError::new(anyhow!(ERROR_MESSAGE), REQUEST_ID));
        assert!(result.is_ok());
    }

    #[test]
    fn runtime_client_send_initialization_error() {
        let server = Server::run();
        server.expect(
            Expectation::matching(all_of![
                request::method("POST"),
                request::path(format!("/{}", RuntimeClient::initialization_error_path())),
            ])
            .respond_with(status_code(200)),
        );

        let client = RuntimeClient::new(&endpoint(&server));
        let result = <RuntimeClient as Client>::send_initialization_error(
            &client,
            InitializationError::new(anyhow!(ERROR_MESSAGE)),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn initialization_error_reporter_send_initialization_error() {
        let server = Server::run();
        server.expect(
            Expectation::matching(all_of![
                request::method("POST"),
                request::path(format!("/{}", RuntimeClient::initialization_error_path())),
            ])
            .respond_with(status_code(200)),
        );

        let reporter = initerr_reporter(&endpoint(&server));
        let result = reporter.send_initialization_error(anyhow!(ERROR_MESSAGE));
        assert!(result.is_ok());
    }
}
