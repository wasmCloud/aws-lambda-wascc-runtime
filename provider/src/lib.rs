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

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use wascc_codec::capabilities::CapabilityProvider;
use wascc_codec::core::{CapabilityConfiguration, OP_BIND_ACTOR, OP_REMOVE_ACTOR};
use wascc_codec::deserialize;

use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::sync::{Arc, RwLock};
use std::thread;

use crate::dispatch::{
    Dispatcher, DispatcherError, HttpDispatcher, NotHttpRequestError, RawEventDispatcher,
};

use crate::lambda::{Client, RuntimeClient};

mod dispatch;
mod http;
mod lambda;

const CAPABILITY_ID: &str = "awslambda:runtime";

// This capability provider is designed to be statically linked into its host.

/// Represents a shared host dispatcher.
pub(crate) type HostDispatcher = Arc<RwLock<Box<dyn wascc_codec::capabilities::Dispatcher>>>;

/// Represents a shared shutdown map, module_id => shutdown_flag.
struct ShutdownMap {
    map: Arc<RwLock<HashMap<String, bool>>>,
}

impl ShutdownMap {
    /// Creates a new, empty `ShutdownMap`.
    fn new() -> Self {
        Self {
            map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns whether the shutdown flag has been set for the specified module.
    fn get(&self, module_id: &str) -> anyhow::Result<bool> {
        Ok(*self
            .map
            .read()
            .map_err(|e| anyhow!("{}", e))?
            .get(module_id)
            .ok_or_else(|| anyhow!("Unknown actor {}", module_id))?)
    }

    /// Puts the shutdown flag value for the specified module.
    /// Any previous value is overwritten.
    fn put(&self, module_id: &str, flag: bool) -> anyhow::Result<()> {
        self.map
            .write()
            .map_err(|e| anyhow!("{}", e))?
            .insert(module_id.into(), flag);

        Ok(())
    }

    /// Puts the shutdown flag value for the specified module if present.
    /// The previous value is overwritten.
    /// Returns whether a value was present.
    fn put_if_present(&self, module_id: &str, flag: bool) -> anyhow::Result<bool> {
        let mut lock = self.map.write().map_err(|e| anyhow!("{}", e))?;
        if !lock.contains_key(module_id) {
            return Ok(false);
        }
        *lock
            .get_mut(module_id)
            .ok_or_else(|| anyhow!("Unknown actor {}", module_id))? = flag;

        Ok(true)
    }

    /// Removes any flag value for the specified module.
    fn remove(&self, module_id: &str) -> anyhow::Result<()> {
        self.map
            .write()
            .map_err(|e| anyhow!("{}", e))?
            .remove(module_id);

        Ok(())
    }
}

impl Clone for ShutdownMap {
    /// Returns a copy of the value.
    fn clone(&self) -> Self {
        Self {
            map: Arc::clone(&self.map),
        }
    }
}

/// Represents a waSCC AWS Lambda runtime provider.
pub struct AwsLambdaRuntimeProvider {
    host_dispatcher: HostDispatcher,
    shutdown_map: ShutdownMap,
}

/// Polls the Lambda event machinery using the specified client.
struct Poller<C> {
    client: C,
    module_id: String,
    host_dispatcher: HostDispatcher,
    shutdown_map: ShutdownMap,
}

impl Default for AwsLambdaRuntimeProvider {
    /// Returns the default value for `AwsLambdaRuntimeProvider`.
    fn default() -> Self {
        Self {
            host_dispatcher: Arc::new(RwLock::new(Box::new(
                wascc_codec::capabilities::NullDispatcher::new(),
            ))),
            shutdown_map: ShutdownMap::new(),
        }
    }
}

impl AwsLambdaRuntimeProvider {
    /// Creates a new, empty `AwsLambdaRuntimeProvider`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts polling the Lambda event machinery.
    fn start_polling(&self, config: CapabilityConfiguration) -> anyhow::Result<()> {
        debug!("awslambda:runtime start_polling");

        let host_dispatcher = Arc::clone(&self.host_dispatcher);
        let endpoint = match config.values.get("AWS_LAMBDA_RUNTIME_API") {
            Some(ep) => String::from(ep),
            None => {
                return Err(anyhow!(
                    "Missing configuration value: AWS_LAMBDA_RUNTIME_API"
                ))
            }
        };
        let module_id = config.module;
        let shutdown_map = self.shutdown_map.clone();
        thread::spawn(move || {
            info!("Starting poller for actor {}", module_id);

            // Initialize this module's shutdown flag.
            match shutdown_map.put(&module_id, false) {
                Ok(_) => {}
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };

            let poller = Poller::new(
                &module_id,
                RuntimeClient::new(&endpoint),
                host_dispatcher,
                shutdown_map,
            );
            poller.run();
        });

        Ok(())
    }

    /// Stops any running Lambda poller.
    fn stop_polling(&self, config: CapabilityConfiguration) -> anyhow::Result<()> {
        debug!("awslambda:runtime stop_polling");

        let module_id = &config.module;
        if !self.shutdown_map.put_if_present(module_id, false)? {
            error!(
                "Received request to stop poller for unknown actor {}. Ignoring",
                module_id
            );
            return Ok(());
        }
        self.shutdown_map.remove(module_id)?;

        Ok(())
    }
}

impl CapabilityProvider for AwsLambdaRuntimeProvider {
    /// Returns the capability ID in the formated `namespace:id`.
    fn capability_id(&self) -> &'static str {
        CAPABILITY_ID
    }

    /// Called when the host runtime is ready and has configured a dispatcher.
    fn configure_dispatch(
        &self,
        dispatcher: Box<dyn wascc_codec::capabilities::Dispatcher>,
    ) -> Result<(), Box<dyn Error>> {
        debug!("awslambda:runtime configure_dispatch");

        let mut lock = self.host_dispatcher.write().unwrap();
        *lock = dispatcher;

        Ok(())
    }

    /// Called by the host runtime when an actor is requesting a command be executed.
    fn handle_call(&self, actor: &str, op: &str, msg: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        info!("awslambda:runtime handle_call `{}` from `{}`", op, actor);

        match op {
            OP_BIND_ACTOR if actor == "system" => self.start_polling(deserialize(msg)?)?,
            OP_REMOVE_ACTOR if actor == "system" => self.stop_polling(deserialize(msg)?)?,
            _ => return Err(format!("Unsupported operation: {}", op).into()),
        }

        Ok(vec![])
    }

    /// Returns the human-readable, friendly name of this capability provider.
    fn name(&self) -> &'static str {
        "waSCC AWS Lambda runtime provider"
    }
}

impl<T: Client> Poller<T> {
    /// Creates a new `Poller`.
    fn new(
        module_id: &str,
        client: T,
        host_dispatcher: HostDispatcher,
        shutdown_map: ShutdownMap,
    ) -> Self {
        Self {
            client,
            module_id: module_id.into(),
            host_dispatcher,
            shutdown_map,
        }
    }

    /// Runs the poller until shutdown.
    fn run(&self) {
        let http_dispatcher = HttpDispatcher::new(Arc::clone(&self.host_dispatcher));
        let raw_event_dispatcher = RawEventDispatcher::new(Arc::clone(&self.host_dispatcher));

        loop {
            if self.shutdown() {
                break;
            }

            // Get next event.
            debug!("Poller get next event");
            let event = match self.client.next_invocation_event() {
                Err(e) => {
                    error!("{}", e);
                    continue;
                }
                Ok(evt) => match evt {
                    None => {
                        warn!("No event");
                        continue;
                    }
                    Some(event) => event,
                },
            };
            let request_id = match event.request_id() {
                None => {
                    warn!("No request ID");
                    continue;
                }
                Some(request_id) => request_id,
            };

            // Set for the X-Ray SDK.
            if let Some(trace_id) = event.trace_id() {
                env::set_var("_X_AMZN_TRACE_ID", trace_id);
            }

            // Try first to dispatch as an HTTP request.
            match http_dispatcher.dispatch_invocation_event(&self.module_id, event.body()) {
                // The invocation event could be converted to an HTTP request and was dispatched succesfully.
                Ok(body) => {
                    self.send_invocation_response(body, request_id);
                    continue;
                }
                // The event couldn't be converted to an HTTP request.
                // Dispatch as a Lambda raw event.
                Err(e) if e.is::<NotHttpRequestError>() => info!("{}", e),
                Err(e) if e.is::<DispatcherError>() => {
                    match e.downcast_ref::<DispatcherError>().unwrap() {
                        // The event could be converted to an HTTP request but couldn't be serialized.
                        // Dispatch as a Lambda raw event.
                        e @ DispatcherError::RequestSerialization { .. } => warn!("{}", e),
                        // The event could be converted to an HTTP request but wasn't dispatched to the actor.
                        // Dispatch as a Lambda raw event.
                        e @ DispatcherError::NotDispatched { .. } => warn!("{}", e),
                        // The event could be converted to an HTTP request and was
                        // dispatched succesfully but there was an error after dispatch,
                        // Fail the invocation.
                        _ => {
                            error!("{}", e);
                            self.send_invocation_error(e, request_id);
                            continue;
                        }
                    }
                }
                // Some other error.
                // Fail the invocation.
                Err(e) => {
                    error!("{}", e);
                    self.send_invocation_error(e, request_id);
                    continue;
                }
            };

            // Dispatch as a Lambda raw event.
            match raw_event_dispatcher.dispatch_invocation_event(&self.module_id, event.body()) {
                Ok(body) => self.send_invocation_response(body, request_id),
                Err(e) => {
                    error!("{}", e);
                    self.send_invocation_error(e, request_id)
                }
            }
        }
    }

    /// Sends an invocation error.
    fn send_invocation_error(&self, e: anyhow::Error, request_id: &str) {
        let err = lambda::InvocationError::new(e, request_id);
        debug!("Poller send error");
        match self.client.send_invocation_error(err) {
            Ok(_) => {}
            Err(e) => error!("Unable to send invocation error: {}", e),
        }
    }

    /// Sends an invocation response.
    fn send_invocation_response(&self, body: Vec<u8>, request_id: &str) {
        let resp = lambda::InvocationResponse::new(body, request_id);
        debug!("Poller send response");
        match self.client.send_invocation_response(resp) {
            Ok(_) => {}
            Err(e) => error!("Unable to send invocation response: {}", e),
        }
    }

    /// Returns whether the shutdown flag is set.
    fn shutdown(&self) -> bool {
        match self.shutdown_map.get(&self.module_id) {
            Ok(f) => f,
            Err(e) => {
                error!("{}", e);
                true
            }
        }
    }
}

/// This module contains code to be used by many unit tests.
#[cfg(test)]
mod tests_common {
    use aws_lambda_events::event::{alb, apigw};
    use serde::Serialize;
    use wascc_codec::serialize;

    use std::any::Any;
    use std::collections::HashMap;
    use std::error::Error;

    pub(crate) const ERROR_MESSAGE: &str = "ERROR";
    pub(crate) const EVENT_BODY: &'static [u8] = b"EVENT BODY";
    pub(crate) const MODULE_ID: &str = "MODULE ID";
    pub(crate) const REQUEST_ID: &str = "REQUEST ID";
    pub(crate) const RESPONSE_BODY: &'static [u8] = b"RESPONSE BODY";

    /// Represents a mock `HostDispatcher`
    pub(crate) struct MockHostDispatcher<T> {
        /// The dispatcher response.
        response: T,
    }

    impl<T> MockHostDispatcher<T> {
        /// Returns a new `MockHostDispatcher`.
        pub fn new(response: T) -> Self {
            Self { response }
        }
    }

    impl<T: Any + Serialize + Send + Sync> wascc_codec::capabilities::Dispatcher
        for MockHostDispatcher<T>
    {
        fn dispatch(
            &self,
            _actor: &str,
            _op: &str,
            _msg: &[u8],
        ) -> Result<Vec<u8>, Box<dyn Error>> {
            Ok(serialize(&self.response)?)
        }
    }

    /// Represents a `HostDispatcher` that returns an error.
    pub(crate) struct ErrorHostDispatcher {}

    impl ErrorHostDispatcher {
        /// Returns a new `ErrorHostDispatcher`.
        pub fn new() -> Self {
            Self {}
        }
    }

    impl wascc_codec::capabilities::Dispatcher for ErrorHostDispatcher {
        fn dispatch(
            &self,
            _actor: &str,
            _op: &str,
            _msg: &[u8],
        ) -> Result<Vec<u8>, Box<dyn Error>> {
            Err(anyhow!(ERROR_MESSAGE).into())
        }
    }

    /// Returns a query string map for a request.
    fn request_query_string() -> HashMap<String, String> {
        let mut qs = HashMap::new();
        qs.insert("key1".into(), "value1".into());
        qs
    }

    /// Returns an HTTP headers map for a request.
    fn request_headers() -> HashMap<String, String> {
        let mut hdrs = HashMap::new();
        hdrs.insert("accept".into(), "application/json".into());
        hdrs
    }

    /// Returns a valid `AlbTargetGroupRequest`.
    pub(crate) fn valid_alb_target_group_request() -> alb::AlbTargetGroupRequest {
        alb::AlbTargetGroupRequest {
            http_method: Some("GET".into()),
            path: Some("/".into()),
            query_string_parameters: request_query_string(),
            multi_value_query_string_parameters: HashMap::new(),
            headers: request_headers(),
            multi_value_headers: HashMap::new(),
            request_context: alb::AlbTargetGroupRequestContext {
                elb: alb::ElbContext {
                    target_group_arn: None,
                },
            },
            is_base64_encoded: false,
            body: Some("Hello world".into()),
        }
    }

    /// Returns a valid `http::Response`.
    pub(crate) fn valid_http_response() -> wascc_codec::http::Response {
        let mut hdrs = HashMap::new();
        hdrs.insert("server".into(), "test".into());
        wascc_codec::http::Response {
            status_code: 200,
            status: "OK".into(),
            header: hdrs,
            body: vec![],
        }
    }

    /// Returns a valid `ApiGatewayProxyRequest`.
    pub(crate) fn valid_api_gateway_proxy_request() -> apigw::ApiGatewayProxyRequest {
        apigw::ApiGatewayProxyRequest {
            resource: None,
            path: Some("/".into()),
            http_method: Some("GET".into()),
            headers: request_headers(),
            multi_value_headers: HashMap::new(),
            query_string_parameters: request_query_string(),
            multi_value_query_string_parameters: HashMap::new(),
            path_parameters: HashMap::new(),
            stage_variables: HashMap::new(),
            request_context: apigw::ApiGatewayProxyRequestContext {
                account_id: None,
                resource_id: None,
                operation_name: None,
                stage: None,
                request_id: None,
                identity: apigw::ApiGatewayRequestIdentity {
                    cognito_identity_pool_id: None,
                    account_id: None,
                    cognito_identity_id: None,
                    caller: None,
                    api_key: None,
                    api_key_id: None,
                    access_key: None,
                    source_ip: None,
                    cognito_authentication_type: None,
                    cognito_authentication_provider: None,
                    user_arn: None,
                    user_agent: None,
                    user: None,
                },
                resource_path: None,
                authorizer: HashMap::new(),
                http_method: None,
                apiid: None,
            },
            body: Some("Hello world".into()),
            is_base64_encoded: Some(false),
        }
    }

    /// Returns a valid `ApiGatewayV2httpRequest`.
    pub(crate) fn valid_api_gatewayv2_proxy_request() -> apigw::ApiGatewayV2httpRequest {
        apigw::ApiGatewayV2httpRequest {
            version: Some("2.0".into()),
            route_key: None,
            raw_path: None,
            raw_query_string: None,
            cookies: None,
            headers: request_headers(),
            query_string_parameters: request_query_string(),
            path_parameters: HashMap::new(),
            request_context: apigw::ApiGatewayV2httpRequestContext {
                route_key: None,
                account_id: None,
                stage: None,
                request_id: None,
                authorizer: None,
                apiid: None,
                domain_name: None,
                domain_prefix: None,
                time: None,
                time_epoch: 0,
                http: apigw::ApiGatewayV2httpRequestContextHttpDescription {
                    method: Some("GET".into()),
                    path: Some("/".into()),
                    protocol: None,
                    source_ip: None,
                    user_agent: None,
                },
            },
            stage_variables: HashMap::new(),
            body: Some("Hello world".into()),
            is_base64_encoded: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lambda::{
        InvocationError, InvocationEvent, InvocationEventBuilder, InvocationResponse,
    };
    use std::cell::RefCell;

    use crate::tests_common::*;

    /// Represents the event kind to return.
    enum EventKind {
        /// No event.
        None,
        /// An invocation event.
        Event(InvocationEvent),
        /// An error.
        Error,
    }

    /// Represents a mock Lambda runtime client that returns a single event.
    struct MockClient {
        event_kind: EventKind,
        invocation_error: RefCell<Option<InvocationError>>,
        invocation_response: RefCell<Option<InvocationResponse>>,
        shutdown_map: ShutdownMap,
    }

    impl MockClient {
        /// Returns a new `MockClient`.
        fn new(event_kind: EventKind, shutdown_map: ShutdownMap) -> Self {
            Self {
                event_kind,
                invocation_error: RefCell::new(None),
                invocation_response: RefCell::new(None),
                shutdown_map,
            }
        }
    }

    impl Client for MockClient {
        /// Returns the next AWS Lambda invocation event.
        fn next_invocation_event(&self) -> anyhow::Result<Option<InvocationEvent>> {
            // Shutdown after one event.
            self.shutdown_map.put(MODULE_ID, true)?;

            match &self.event_kind {
                EventKind::None => Ok(None),
                EventKind::Event(event) => Ok(Some(event.clone())),
                EventKind::Error => Err(anyhow!(ERROR_MESSAGE)),
            }
        }

        /// Sends an invocation error to the AWS Lambda runtime.
        fn send_invocation_error(&self, error: InvocationError) -> anyhow::Result<()> {
            *self.invocation_error.borrow_mut() = Some(error);
            Ok(())
        }

        /// Sends an invocation error to the AWS Lambda runtime.
        fn send_invocation_response(&self, response: InvocationResponse) -> anyhow::Result<()> {
            *self.invocation_response.borrow_mut() = Some(response);
            Ok(())
        }
    }

    impl InvocationEvent {
        /// Returns an `InvocationEvent` with no request ID.
        fn no_request_id() -> Self {
            InvocationEventBuilder::new(EVENT_BODY.to_vec()).build()
        }

        /// Return an `InvocationEvent` with a request ID.
        fn with_request_id() -> Self {
            let mut builder = InvocationEventBuilder::new(EVENT_BODY.to_vec());
            builder = builder.request_id(REQUEST_ID);
            builder.build()
        }
    }

    /// Returns a mock poller.
    fn mock_poller(event_kind: EventKind) -> Poller<MockClient> {
        let response = codec::Response {
            body: RESPONSE_BODY.to_vec(),
        };
        let host_dispatcher: HostDispatcher =
            Arc::new(RwLock::new(Box::new(MockHostDispatcher::new(response))));
        let shutdown_map = ShutdownMap::new();
        Poller::new(
            MODULE_ID,
            MockClient::new(event_kind, shutdown_map.clone()),
            host_dispatcher,
            shutdown_map,
        )
    }

    /// Returns a mock poller with a host dispatcher that returns errors.
    fn mock_poller_with_error_dispatcher(event_kind: EventKind) -> Poller<MockClient> {
        let host_dispatcher: HostDispatcher =
            Arc::new(RwLock::new(Box::new(ErrorHostDispatcher::new())));
        let shutdown_map = ShutdownMap::new();
        Poller::new(
            MODULE_ID,
            MockClient::new(event_kind, shutdown_map.clone()),
            host_dispatcher,
            shutdown_map,
        )
    }

    /// Tests that receiving an empty event sends no response or error.
    #[test]
    fn event_kind_none() {
        let poller = mock_poller(EventKind::None);
        let result = poller.shutdown_map.put(MODULE_ID, false);
        assert!(result.is_ok());

        poller.run();

        assert!(poller.client.invocation_response.borrow().is_none());
        assert!(poller.client.invocation_error.borrow().is_none());
    }

    /// Tests that receiving an event without a request ID sends no response or error.
    #[test]
    fn event_kind_event_no_request_id() {
        let poller = mock_poller(EventKind::Event(InvocationEvent::no_request_id()));
        let result = poller.shutdown_map.put(MODULE_ID, false);
        assert!(result.is_ok());

        poller.run();

        assert!(poller.client.invocation_response.borrow().is_none());
        assert!(poller.client.invocation_error.borrow().is_none());
    }

    /// Tests that receiving an event with a request ID sends a response.
    #[test]
    fn event_kind_event_with_request_id() {
        let poller = mock_poller(EventKind::Event(InvocationEvent::with_request_id()));
        let result = poller.shutdown_map.put(MODULE_ID, false);
        assert!(result.is_ok());

        poller.run();

        assert!(poller.client.invocation_response.borrow().is_some());
        assert_eq!(
            REQUEST_ID,
            poller
                .client
                .invocation_response
                .borrow()
                .as_ref()
                .unwrap()
                .request_id()
        );
        assert_eq!(
            RESPONSE_BODY,
            poller
                .client
                .invocation_response
                .borrow()
                .as_ref()
                .unwrap()
                .body()
        );
        assert!(poller.client.invocation_error.borrow().is_none());
    }

    /// Tests that receiving an event with a request ID and dispatching with an error sends an error.
    #[test]
    fn event_kind_event_with_request_id_error_dispatcher() {
        let poller =
            mock_poller_with_error_dispatcher(EventKind::Event(InvocationEvent::with_request_id()));
        let result = poller.shutdown_map.put(MODULE_ID, false);
        assert!(result.is_ok());

        poller.run();

        assert!(poller.client.invocation_response.borrow().is_none());
        assert!(poller.client.invocation_error.borrow().is_some());
        assert_eq!(
            REQUEST_ID,
            poller
                .client
                .invocation_error
                .borrow()
                .as_ref()
                .unwrap()
                .request_id()
        );
    }

    /// Tests that receiving an error sends no response or error.
    #[test]
    fn event_kind_error() {
        let poller = mock_poller(EventKind::Error);
        let result = poller.shutdown_map.put(MODULE_ID, false);
        assert!(result.is_ok());

        poller.run();

        assert!(poller.client.invocation_response.borrow().is_none());
        assert!(poller.client.invocation_error.borrow().is_none());
    }
}
