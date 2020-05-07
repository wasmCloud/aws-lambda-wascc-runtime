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
// waSCC AWS Lambda Runtime Providers
//

use wascc_codec::capabilities::CapabilityProvider;
use wascc_codec::core::{CapabilityConfiguration, OP_BIND_ACTOR, OP_REMOVE_ACTOR};
use wascc_codec::deserialize;

use std::env;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};
use std::thread;

use crate::dispatch::{EventDispatcher, HttpDispatcher, InvocationEventDispatcher};
use crate::lambda::{Client, InvocationError, InvocationResponse, RuntimeClient};
use crate::{HostDispatcher, ShutdownMap};

// These capability providers are designed to be statically linked into its host.

/// Represents a waSCC AWS Lambda runtime provider.
struct LambdaProvider<T>
where
    T: InvocationEventDispatcher + From<HostDispatcher>,
{
    host_dispatcher: HostDispatcher,
    shutdown_map: ShutdownMap,
    dispatcher_type: PhantomData<T>,
}

impl<T: InvocationEventDispatcher + From<HostDispatcher>> LambdaProvider<T> {
    /// Creates a new, empty `LambdaProvider`.
    pub fn new() -> Self {
        Self {
            host_dispatcher: Arc::new(RwLock::new(Box::new(
                wascc_codec::capabilities::NullDispatcher::new(),
            ))),
            shutdown_map: ShutdownMap::new(),
            dispatcher_type: PhantomData,
        }
    }

    /// Called when the host runtime is ready and has configured a dispatcher.
    fn configure_dispatch(
        &self,
        dispatcher: Box<dyn wascc_codec::capabilities::Dispatcher>,
    ) -> anyhow::Result<()> {
        debug!("awslambda:provider configure_dispatch");

        let mut lock = self.host_dispatcher.write().unwrap();
        *lock = dispatcher;

        Ok(())
    }

    /// Called by the host runtime when an actor is requesting a command be executed.
    fn handle_call(&self, actor: &str, op: &str, msg: &[u8]) -> anyhow::Result<Vec<u8>> {
        info!("awslambda:provider handle_call `{}` from `{}`", op, actor);

        match op {
            OP_BIND_ACTOR if actor == "system" => {
                self.start_polling(deserialize(msg).map_err(|e| anyhow!("{}", e))?)?
            }
            OP_REMOVE_ACTOR if actor == "system" => {
                self.stop_polling(deserialize(msg).map_err(|e| anyhow!("{}", e))?)?
            }
            _ => return Err(anyhow!("Unsupported operation: {}", op)),
        }

        Ok(vec![])
    }

    /// Starts polling the Lambda event machinery.
    fn start_polling(&self, config: CapabilityConfiguration) -> anyhow::Result<()> {
        debug!("awslambda:provider start_polling");

        let host_dispatcher = Arc::clone(&self.host_dispatcher);
        let endpoint = match config.values.get("AWS_LAMBDA_RUNTIME_API") {
            Some(ep) => format!("http://{}", ep),
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

            let poller = Poller::new(&module_id, RuntimeClient::new(&endpoint), shutdown_map);
            poller.run(T::from(host_dispatcher));
        });

        Ok(())
    }

    /// Stops any running Lambda poller.
    fn stop_polling(&self, config: CapabilityConfiguration) -> anyhow::Result<()> {
        debug!("awslambda:provider stop_polling");

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

/// Represents a waSCC AWS Lambda event provider.
pub struct LambdaEventProvider(LambdaProvider<EventDispatcher>);

impl LambdaEventProvider {
    /// Creates a new, empty `LambdaEventProvider`.
    pub fn new() -> Self {
        Self(LambdaProvider::new())
    }
}

impl CapabilityProvider for LambdaEventProvider {
    /// Returns the capability ID in the formated `namespace:id`.
    fn capability_id(&self) -> &'static str {
        "awslambda:event"
    }

    /// Called when the host runtime is ready and has configured a dispatcher.
    fn configure_dispatch(
        &self,
        dispatcher: Box<dyn wascc_codec::capabilities::Dispatcher>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.0.configure_dispatch(dispatcher).map_err(|e| e.into())
    }

    /// Called by the host runtime when an actor is requesting a command be executed.
    fn handle_call(
        &self,
        actor: &str,
        op: &str,
        msg: &[u8],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        self.0.handle_call(actor, op, msg).map_err(|e| e.into())
    }

    /// Returns the human-readable, friendly name of this capability provider.
    fn name(&self) -> &'static str {
        "waSCC AWS Lambda event provider"
    }
}

/// Represents a waSCC AWS Lambda HTTP provider.
pub struct LambdaHttpProvider(LambdaProvider<HttpDispatcher>);

impl LambdaHttpProvider {
    /// Creates a new, empty `LambdaHttpProvider`.
    pub fn new() -> Self {
        Self(LambdaProvider::new())
    }
}

impl CapabilityProvider for LambdaHttpProvider {
    /// Returns the capability ID in the formated `namespace:id`.
    fn capability_id(&self) -> &'static str {
        "wascc:http_server"
    }

    /// Called when the host runtime is ready and has configured a dispatcher.
    fn configure_dispatch(
        &self,
        dispatcher: Box<dyn wascc_codec::capabilities::Dispatcher>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.0.configure_dispatch(dispatcher).map_err(|e| e.into())
    }

    /// Called by the host runtime when an actor is requesting a command be executed.
    fn handle_call(
        &self,
        actor: &str,
        op: &str,
        msg: &[u8],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        self.0.handle_call(actor, op, msg).map_err(|e| e.into())
    }

    /// Returns the human-readable, friendly name of this capability provider.
    fn name(&self) -> &'static str {
        "waSCC AWS Lambda HTTP provider"
    }
}

/// Polls the Lambda event machinery using the specified client.
struct Poller<C> {
    client: C,
    module_id: String,
    shutdown_map: ShutdownMap,
}

impl<C: Client> Poller<C> {
    /// Creates a new `Poller`.
    fn new(module_id: &str, client: C, shutdown_map: ShutdownMap) -> Self {
        Self {
            client,
            module_id: module_id.into(),
            shutdown_map,
        }
    }

    /// Runs the poller until shutdown.
    fn run(&self, dispatcher: impl InvocationEventDispatcher) {
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

            match dispatcher.dispatch_invocation_event(&self.module_id, event.body()) {
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
        let err = InvocationError::new(e, request_id);
        debug!("Poller send error");
        match self.client.send_invocation_error(err) {
            Ok(_) => {}
            Err(e) => error!("Unable to send invocation error: {}", e),
        }
    }

    /// Sends an invocation response.
    fn send_invocation_response(&self, body: Vec<u8>, request_id: &str) {
        let resp = InvocationResponse::new(body, request_id);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lambda::{
        InitializationError, InvocationError, InvocationEvent, InvocationEventBuilder,
        InvocationResponse,
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
        initialization_error: RefCell<Option<InitializationError>>,
        invocation_error: RefCell<Option<InvocationError>>,
        invocation_response: RefCell<Option<InvocationResponse>>,
        shutdown_map: ShutdownMap,
    }

    impl MockClient {
        /// Returns a new `MockClient`.
        fn new(event_kind: EventKind, shutdown_map: ShutdownMap) -> Self {
            Self {
                event_kind,
                initialization_error: RefCell::new(None),
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

        /// Sends an initialization error to the AWS Lambda runtime.
        fn send_initialization_error(&self, error: InitializationError) -> anyhow::Result<()> {
            *self.initialization_error.borrow_mut() = Some(error);
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

    /// Returns a test event dispatcher.
    fn dispatcher() -> impl InvocationEventDispatcher {
        let response = codec::Response {
            body: RESPONSE_BODY.to_vec(),
        };
        let host_dispatcher: HostDispatcher =
            Arc::new(RwLock::new(Box::new(MockHostDispatcher::new(response))));
        EventDispatcher::new(host_dispatcher)
    }

    /// Returns a test event dispatcher that errors.
    fn error_dispatcher() -> impl InvocationEventDispatcher {
        let host_dispatcher: HostDispatcher =
            Arc::new(RwLock::new(Box::new(ErrorHostDispatcher::new())));
        EventDispatcher::new(host_dispatcher)
    }

    /// Returns a mock poller.
    fn mock_poller(event_kind: EventKind) -> Poller<MockClient> {
        let shutdown_map = ShutdownMap::new();
        Poller::new(
            MODULE_ID,
            MockClient::new(event_kind, shutdown_map.clone()),
            shutdown_map,
        )
    }

    /// Tests that receiving an empty event sends no response or error.
    #[test]
    fn event_kind_none() {
        let poller = mock_poller(EventKind::None);
        let result = poller.shutdown_map.put(MODULE_ID, false);
        assert!(result.is_ok());

        poller.run(dispatcher());

        assert!(poller.client.initialization_error.borrow().is_none());
        assert!(poller.client.invocation_response.borrow().is_none());
        assert!(poller.client.invocation_error.borrow().is_none());
    }

    /// Tests that receiving an event without a request ID sends no response or error.
    #[test]
    fn event_kind_event_no_request_id() {
        let poller = mock_poller(EventKind::Event(InvocationEvent::no_request_id()));
        let result = poller.shutdown_map.put(MODULE_ID, false);
        assert!(result.is_ok());

        poller.run(dispatcher());

        assert!(poller.client.initialization_error.borrow().is_none());
        assert!(poller.client.invocation_response.borrow().is_none());
        assert!(poller.client.invocation_error.borrow().is_none());
    }

    /// Tests that receiving an event with a request ID sends a response.
    #[test]
    fn event_kind_event_with_request_id() {
        let poller = mock_poller(EventKind::Event(InvocationEvent::with_request_id()));
        let result = poller.shutdown_map.put(MODULE_ID, false);
        assert!(result.is_ok());

        poller.run(dispatcher());

        assert!(poller.client.initialization_error.borrow().is_none());
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
        let poller = mock_poller(EventKind::Event(InvocationEvent::with_request_id()));
        let result = poller.shutdown_map.put(MODULE_ID, false);
        assert!(result.is_ok());

        poller.run(error_dispatcher());

        assert!(poller.client.initialization_error.borrow().is_none());
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

        poller.run(dispatcher());

        assert!(poller.client.initialization_error.borrow().is_none());
        assert!(poller.client.invocation_response.borrow().is_none());
        assert!(poller.client.invocation_error.borrow().is_none());
    }
}
