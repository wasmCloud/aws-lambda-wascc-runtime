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

use wascc_codec::capabilities::{
    CapabilityDescriptor, CapabilityProvider, OperationDirection, OP_GET_CAPABILITY_DESCRIPTOR,
};
use wascc_codec::core::{CapabilityConfiguration, OP_BIND_ACTOR};
use wascc_codec::{deserialize, serialize, SYSTEM_ACTOR};

use std::any::Any;
use std::env;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;

use crate::dispatch::{HttpRequestDispatcher, InvocationEventDispatcher, RawEventDispatcher};
use crate::lambda::{Client, InvocationError, InvocationResponse, RuntimeClient};
use crate::HostDispatcher;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const REVISION: u32 = 2; // Increment for each crates publish

//
// These capability providers are designed to be statically linked into its host.
//

/// Represents the "read" logic for stopping a provider.
trait StopperR {
    /// Returns whether or not to stop.
    fn stop(&self) -> anyhow::Result<bool>;
}

/// Represents the "write" logic for stopping a provider.
trait StopperW {
    /// Stops the provider.
    /// Doesn't wait for the provider to stop.
    fn stop(&self) -> anyhow::Result<()>;

    /// Waits for the provider to stop.
    fn wait(&mut self) -> anyhow::Result<()>;
}

/// Represents the "standard" provider stopper.
struct Stopper {
    stop: Arc<AtomicBool>,
}

impl Stopper {
    /// Creates a new `Stopper`.
    fn new() -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Clone for Stopper {
    /// Returns a copy of the value.
    fn clone(&self) -> Self {
        Self {
            stop: Arc::clone(&self.stop),
        }
    }
}

impl StopperR for Stopper {
    /// Returns whether or not to stop.
    fn stop(&self) -> anyhow::Result<bool> {
        Ok(self.stop.load(Ordering::Relaxed))
    }
}

impl StopperW for Stopper {
    /// Stops the provider.
    /// Doesn't wait for the provider to stop.
    fn stop(&self) -> anyhow::Result<()> {
        self.stop.store(true, Ordering::Relaxed);

        Ok(())
    }

    /// Waits for the provider to stop.
    fn wait(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Represents a waSCC AWS Lambda runtime provider.
struct LambdaProvider<S, CF, C, DF, D> {
    host_dispatcher: HostDispatcher,
    stopper: S,
    client_factory: CF,
    client_type: PhantomData<C>,
    dispatcher_factory: DF,
    dispatcher_type: PhantomData<D>,
}

impl<
        S: Clone + Send + StopperR + 'static,
        CF: ClientFactory<C>,
        C: Send + Client + 'static,
        DF: DispatcherFactory<D>,
        D: Send + InvocationEventDispatcher + 'static,
    > LambdaProvider<S, CF, C, DF, D>
{
    /// Creates a new, empty `LambdaProvider`.
    pub fn new(stopper: S, client_factory: CF, dispatcher_factory: DF) -> Self {
        Self {
            host_dispatcher: Arc::new(RwLock::new(Box::new(
                wascc_codec::capabilities::NullDispatcher::new(),
            ))),
            stopper,
            client_factory,
            client_type: PhantomData,
            dispatcher_factory,
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
            OP_BIND_ACTOR if actor == SYSTEM_ACTOR => {
                self.start_polling(deserialize(msg).map_err(|e| anyhow!("{}", e))?)?
            }
            _ => return Err(anyhow!("Unsupported operation: {}/{}", op, actor)),
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
        let stopper = self.stopper.clone();

        let client = self.client_factory.new_client(&endpoint);
        let poller = Poller::new(&module_id, client, stopper);

        let dispatcher = self.dispatcher_factory.new_dispatcher(host_dispatcher);

        thread::spawn(move || {
            info!("Starting poller for actor {}", module_id);

            poller.run(dispatcher);
        });

        Ok(())
    }
}

/// Represents a waSCC AWS Lambda raw event provider.
/// This capability provider dispatches events from
/// the AWS Lambda machinery as raw events (without translation).
struct LambdaRawEventProvider<S, CF: ClientFactory<C>, C: Client>(
    LambdaProvider<S, CF, C, RawEventDispatcherFactory, RawEventDispatcher>,
);

impl<S: Clone + Send + StopperR + 'static, CF: ClientFactory<C>, C: Send + Client + 'static>
    LambdaRawEventProvider<S, CF, C>
{
    /// Creates a new, empty `LambdaRawEventProvider`.
    pub fn new(stopper: S, client_factory: CF) -> Self {
        Self(LambdaProvider::new(
            stopper,
            client_factory,
            RawEventDispatcherFactory::new(),
        ))
    }

    /// Returns a serialized capability descriptor for this provider.
    fn get_descriptor(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serialize(
            CapabilityDescriptor::builder()
                .id("awslambda:event")
                .name("waSCC AWS Lambda raw event provider")
                .long_description("A capability provider that handles AWS Lambda events")
                .version(VERSION)
                .revision(REVISION)
                .with_operation(
                    codec::OP_HANDLE_EVENT,
                    OperationDirection::ToActor,
                    "Delivers a JSON event to an actor and expects a JSON response in return",
                )
                .build(),
        )
        .map_err(|e| anyhow!("{}", e))?)
    }
}

/// Returns an instance of the default raw event capability provider.
pub fn default_raw_event_provider() -> impl CapabilityProvider {
    LambdaRawEventProvider::new(Stopper::new(), RuntimeClientFactory::new())
}

impl<
        S: Clone + Send + Sync + StopperR + 'static,
        CF: Any + Send + Sync + ClientFactory<C>,
        C: Any + Send + Sync + Client,
    > CapabilityProvider for LambdaRawEventProvider<S, CF, C>
{
    /// Called when the host runtime is ready and has configured a dispatcher.
    fn configure_dispatch(
        &self,
        dispatcher: Box<dyn wascc_codec::capabilities::Dispatcher>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0.configure_dispatch(dispatcher).map_err(|e| e.into())
    }

    /// Called by the host runtime when an actor is requesting a command be executed.
    fn handle_call(
        &self,
        actor: &str,
        op: &str,
        msg: &[u8],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error+ Send + Sync>> {
        match op {
            OP_GET_CAPABILITY_DESCRIPTOR if actor == SYSTEM_ACTOR => self.get_descriptor(),
            _ => self.0.handle_call(actor, op, msg),
        }
        .map_err(|e| e.into())
    }
}

/// Represents a waSCC AWS Lambda HTTP request provider.
/// This capability provider dispatches events from
/// the AWS Lambda machinery as HTTP requests.
struct LambdaHttpRequestProvider<S, CF: ClientFactory<C>, C: Client>(
    LambdaProvider<S, CF, C, HttpRequestDispatcherFactory, HttpRequestDispatcher>,
);

impl<S: Clone + Send + StopperR + 'static, CF: ClientFactory<C>, C: Send + Client + 'static>
    LambdaHttpRequestProvider<S, CF, C>
{
    /// Creates a new, empty `LambdaHttpRequestProvider`.
    pub fn new(stopper: S, client_factory: CF) -> Self {
        Self(LambdaProvider::new(
            stopper,
            client_factory,
            HttpRequestDispatcherFactory::new(),
        ))
    }

    /// Returns a serialized capability descriptor for this provider.
    fn get_descriptor(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serialize(
            CapabilityDescriptor::builder()
                .id("wascc:http_server")
                .name("waSCC AWS Lambda HTTP request provider")
                .long_description("A capability provider that handles AWS Lambda HTTP requests")
                .version(VERSION)
                .revision(REVISION)
                .with_operation(
                    wascc_codec::http::OP_HANDLE_REQUEST,
                    OperationDirection::ToActor,
                    "Delivers an HTTP request to an actor and expects a HTTP response in return",
                )
                .build(),
        )
        .map_err(|e| anyhow!("{}", e))?)
    }
}

/// Returns an instance of the default HTTP request capability provider.
pub fn default_http_request_provider() -> impl CapabilityProvider {
    LambdaHttpRequestProvider::new(Stopper::new(), RuntimeClientFactory::new())
}

impl<
        S: Clone + Send + Sync + StopperR + 'static,
        CF: Any + Send + Sync + ClientFactory<C>,
        C: Any + Send + Sync + Client,
    > CapabilityProvider for LambdaHttpRequestProvider<S, CF, C>
{
    /// Called when the host runtime is ready and has configured a dispatcher.
    fn configure_dispatch(
        &self,
        dispatcher: Box<dyn wascc_codec::capabilities::Dispatcher>,
    ) -> Result<(), Box<dyn std::error::Error+ Send + Sync>> {
        self.0.configure_dispatch(dispatcher).map_err(|e| e.into())
    }

    /// Called by the host runtime when an actor is requesting a command be executed.
    fn handle_call(
        &self,
        actor: &str,
        op: &str,
        msg: &[u8],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error+ Send + Sync>> {
        match op {
            OP_GET_CAPABILITY_DESCRIPTOR if actor == SYSTEM_ACTOR => self.get_descriptor(),
            _ => self.0.handle_call(actor, op, msg),
        }
        .map_err(|e| e.into())
    }
}

/// Creates `Client` instances.
trait ClientFactory<C> {
    /// Creates a new `Client`.
    fn new_client(&self, endpoint: &str) -> C;
}

/// Creates `RuntimeClient` instances.
struct RuntimeClientFactory;

impl RuntimeClientFactory {
    /// Returns new `RuntimeClientFactory` instances.
    fn new() -> Self {
        Self
    }
}

impl ClientFactory<RuntimeClient> for RuntimeClientFactory {
    /// Creates a new `RuntimeClient`.
    fn new_client(&self, endpoint: &str) -> RuntimeClient {
        RuntimeClient::new(endpoint)
    }
}

/// Creates `Dispatcher` instances.
trait DispatcherFactory<D> {
    /// Creates a new `Dispatcher`.
    fn new_dispatcher(&self, host_dispatcher: HostDispatcher) -> D;
}

/// Creates `HttpRequestDispatcher` instances.
struct HttpRequestDispatcherFactory;

impl HttpRequestDispatcherFactory {
    /// Returns new `HttpRequestDispatcherFactory` instances.
    fn new() -> Self {
        Self
    }
}

impl DispatcherFactory<HttpRequestDispatcher> for HttpRequestDispatcherFactory {
    /// Creates a new `HttpRequestDispatcher`.
    fn new_dispatcher(&self, host_dispatcher: HostDispatcher) -> HttpRequestDispatcher {
        HttpRequestDispatcher::new(host_dispatcher)
    }
}

/// Creates `RawEventDispatcher` instances.
struct RawEventDispatcherFactory;

impl RawEventDispatcherFactory {
    /// Returns new `RawEventDispatcherFactory` instances.
    fn new() -> Self {
        Self
    }
}

impl DispatcherFactory<RawEventDispatcher> for RawEventDispatcherFactory {
    /// Creates a new `RawEventDispatcher`.
    fn new_dispatcher(&self, host_dispatcher: HostDispatcher) -> RawEventDispatcher {
        RawEventDispatcher::new(host_dispatcher)
    }
}

/// Polls the Lambda event machinery using the specified client.
struct Poller<C, S> {
    client: C,
    module_id: String,
    stopper: S,
}

impl<C: Client, S: StopperR> Poller<C, S> {
    /// Creates a new `Poller`.
    fn new(module_id: &str, client: C, stopper: S) -> Self {
        Self {
            client,
            module_id: module_id.into(),
            stopper,
        }
    }

    /// Runs the poller until shutdown.
    fn run(&self, dispatcher: impl InvocationEventDispatcher) {
        loop {
            match self.stopper.stop() {
                Err(e) => {
                    error!("{}", e);
                    break;
                }
                Ok(stop) if stop => break,
                _ => {}
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lambda::{
        InitializationError, InvocationError, InvocationEvent, InvocationEventBuilder,
        InvocationResponse,
    };
    use std::collections::HashMap;
    use wascc_codec::serialize;

    use crate::tests_common::*;

    /// Represents the event kind to return.
    #[derive(Clone)]
    enum EventKind {
        /// No event.
        None,
        /// An invocation event.
        Event(InvocationEvent),
        /// An error.
        Error,
    }

    /// Creates `MockClient` instances.
    struct MockClientFactory {
        event_kind: EventKind,
        stopper: Stopper,
    }

    impl MockClientFactory {
        /// Creates a new `MockClientFactory`.
        fn new(event_kind: EventKind, stopper: Stopper) -> Self {
            Self {
                event_kind,
                stopper,
            }
        }
    }

    impl ClientFactory<MockClient> for MockClientFactory {
        /// Creates a new `MockClient`.
        fn new_client(&self, _endpoint: &str) -> MockClient {
            MockClient::new(self.event_kind.clone(), self.stopper.clone())
        }
    }

    /// Represents a mock Lambda runtime client that returns a single event.
    struct MockClient {
        event_kind: EventKind,
        initialization_error: RwLock<Option<InitializationError>>,
        invocation_error: RwLock<Option<InvocationError>>,
        invocation_response: RwLock<Option<InvocationResponse>>,
        stopper: Stopper,
    }

    impl MockClient {
        /// Returns a new `MockClient`.
        fn new(event_kind: EventKind, stopper: Stopper) -> Self {
            Self {
                event_kind,
                initialization_error: RwLock::new(None),
                invocation_error: RwLock::new(None),
                invocation_response: RwLock::new(None),
                stopper,
            }
        }
    }

    impl Client for MockClient {
        /// Returns the next AWS Lambda invocation event.
        fn next_invocation_event(&self) -> anyhow::Result<Option<InvocationEvent>> {
            // Shutdown after one event.
            <Stopper as StopperW>::stop(&self.stopper)?;

            match &self.event_kind {
                EventKind::None => Ok(None),
                EventKind::Event(event) => Ok(Some(event.clone())),
                EventKind::Error => Err(anyhow!(ERROR_MESSAGE)),
            }
        }

        /// Sends an invocation error to the AWS Lambda runtime.
        fn send_invocation_error(&self, error: InvocationError) -> anyhow::Result<()> {
            // Record the parameters.
            let mut lock = self.invocation_error.write().unwrap();
            *lock = Some(error);

            Ok(())
        }

        /// Sends an invocation error to the AWS Lambda runtime.
        fn send_invocation_response(&self, response: InvocationResponse) -> anyhow::Result<()> {
            // Record the parameters.
            let mut lock = self.invocation_response.write().unwrap();
            *lock = Some(response);

            Ok(())
        }

        /// Sends an initialization error to the AWS Lambda runtime.
        fn send_initialization_error(&self, error: InitializationError) -> anyhow::Result<()> {
            // Record the parameters.
            let mut lock = self.initialization_error.write().unwrap();
            *lock = Some(error);

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
        let host_dispatcher = mock_host_dispatcher(response);
        RawEventDispatcher::new(host_dispatcher)
    }

    /// Returns a test event dispatcher that errors.
    fn error_dispatcher() -> impl InvocationEventDispatcher {
        let host_dispatcher = error_host_dispatcher();
        RawEventDispatcher::new(host_dispatcher)
    }

    /// Returns a mock poller.
    fn mock_poller(event_kind: EventKind) -> Poller<MockClient, Stopper> {
        let stopper = Stopper::new();

        Poller::new(
            MODULE_ID,
            MockClient::new(event_kind, stopper.clone()),
            stopper,
        )
    }

    /// Returns a `MockClientFactory`.
    fn mock_client_factory(event_kind: EventKind) -> impl ClientFactory<MockClient> {
        MockClientFactory::new(event_kind, Stopper::new())
    }

    /// Returns a serialized test capability configuration.
    fn capability_configuration() -> Vec<u8> {
        let mut values = HashMap::new();
        values.insert("AWS_LAMBDA_RUNTIME_API".into(), "localhost:8080".into());
        serialize(CapabilityConfiguration {
            module: MODULE_ID.into(),
            values,
        })
        .unwrap()
    }

    /// Returns a serialized empty capability configuration.
    fn empty_capability_configuration() -> Vec<u8> {
        serialize(CapabilityConfiguration {
            module: MODULE_ID.into(),
            values: HashMap::new(),
        })
        .unwrap()
    }

    /// Tests that receiving an empty event sends no response or error.
    #[test]
    fn poller_event_kind_none() {
        let poller = mock_poller(EventKind::None);
        poller.run(dispatcher());

        assert!(poller.client.initialization_error.read().unwrap().is_none());
        assert!(poller.client.invocation_response.read().unwrap().is_none());
        assert!(poller.client.invocation_error.read().unwrap().is_none());
    }

    /// Tests that receiving an event without a request ID sends no response or error.
    #[test]
    fn poller_event_kind_event_no_request_id() {
        let poller = mock_poller(EventKind::Event(InvocationEvent::no_request_id()));
        poller.run(dispatcher());

        assert!(poller.client.initialization_error.read().unwrap().is_none());
        assert!(poller.client.invocation_response.read().unwrap().is_none());
        assert!(poller.client.invocation_error.read().unwrap().is_none());
    }

    /// Tests that receiving an event with a request ID sends a response.
    #[test]
    fn poller_event_kind_event_with_request_id() {
        let poller = mock_poller(EventKind::Event(InvocationEvent::with_request_id()));
        poller.run(dispatcher());

        assert!(poller.client.initialization_error.read().unwrap().is_none());
        assert!(poller.client.invocation_response.read().unwrap().is_some());
        assert_eq!(
            REQUEST_ID,
            poller
                .client
                .invocation_response
                .read()
                .unwrap()
                .as_ref()
                .unwrap()
                .request_id()
        );
        assert_eq!(
            RESPONSE_BODY,
            poller
                .client
                .invocation_response
                .read()
                .unwrap()
                .as_ref()
                .unwrap()
                .body()
        );
        assert!(poller.client.invocation_error.read().unwrap().is_none());
    }

    /// Tests that receiving an event with a request ID and dispatching with an error sends an error.
    #[test]
    fn poller_event_kind_event_with_request_id_error_dispatcher() {
        let poller = mock_poller(EventKind::Event(InvocationEvent::with_request_id()));
        poller.run(error_dispatcher());

        assert!(poller.client.initialization_error.read().unwrap().is_none());
        assert!(poller.client.invocation_response.read().unwrap().is_none());
        assert!(poller.client.invocation_error.read().unwrap().is_some());
        assert_eq!(
            REQUEST_ID,
            poller
                .client
                .invocation_error
                .read()
                .unwrap()
                .as_ref()
                .unwrap()
                .request_id()
        );
    }

    /// Tests that receiving an error sends no response or error.
    #[test]
    fn poller_event_kind_error() {
        let poller = mock_poller(EventKind::Error);
        poller.run(dispatcher());

        assert!(poller.client.initialization_error.read().unwrap().is_none());
        assert!(poller.client.invocation_response.read().unwrap().is_none());
        assert!(poller.client.invocation_error.read().unwrap().is_none());
    }

    #[test]
    fn raw_event_provider_unsupported_operation() {
        let provider = default_raw_event_provider();
        let result = provider.configure_dispatch(boxed_mock_dispatcher(RESPONSE_BODY));
        assert!(result.is_ok());

        let result = provider.handle_call("", OP_BIND_ACTOR, &[]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unsupported operation"));

        let result = provider.handle_call(SYSTEM_ACTOR, "", &[]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unsupported operation"));
    }

    #[test]
    fn raw_event_provider_no_config() {
        let provider = default_raw_event_provider();
        let result = provider.configure_dispatch(boxed_mock_dispatcher(RESPONSE_BODY));
        assert!(result.is_ok());

        let result = provider.handle_call(SYSTEM_ACTOR, OP_BIND_ACTOR, &[]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to de-serialize"));
    }

    #[test]
    fn raw_event_provider_no_endpoint() {
        let provider = default_raw_event_provider();
        let result = provider.configure_dispatch(boxed_mock_dispatcher(RESPONSE_BODY));
        assert!(result.is_ok());

        let result = provider.handle_call(
            SYSTEM_ACTOR,
            OP_BIND_ACTOR,
            &empty_capability_configuration(),
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing configuration value"));
    }

    #[test]
    fn raw_event_provider_ok() {
        let client_factory =
            mock_client_factory(EventKind::Event(InvocationEvent::with_request_id()));
        let mut stopper = Stopper::new();
        let provider = LambdaRawEventProvider::new(stopper.clone(), client_factory);
        let mock_dispatcher = boxed_mock_dispatcher(RESPONSE_BODY);
        let result = provider.configure_dispatch(mock_dispatcher);
        assert!(result.is_ok());

        let result = provider.handle_call(SYSTEM_ACTOR, OP_BIND_ACTOR, &capability_configuration());
        assert!(result.is_ok());

        let result = stopper.wait();
        assert!(result.is_ok());
    }
}
