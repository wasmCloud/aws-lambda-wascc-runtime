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

use std::any::Any;
use std::collections::HashMap;
use std::env;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};
use std::thread;

use crate::dispatch::{HttpRequestDispatcher, InvocationEventDispatcher, RawEventDispatcher};
use crate::lambda::{Client, InvocationError, InvocationResponse, RuntimeClient};
use crate::HostDispatcher;

//
// These capability providers are designed to be statically linked into its host.
//

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
struct LambdaProvider<T, CF: ClientFactory<C>, C: Client> {
    host_dispatcher: HostDispatcher,
    client_factory: CF,
    shutdown_map: ShutdownMap,
    client_type: PhantomData<C>,
    dispatcher_type: PhantomData<T>,
}

impl<
        T: InvocationEventDispatcher + From<HostDispatcher>,
        CF: ClientFactory<C>,
        C: Send + Client + 'static,
    > LambdaProvider<T, CF, C>
{
    /// Creates a new, empty `LambdaProvider`.
    pub fn new(client_factory: CF) -> Self {
        Self {
            host_dispatcher: Arc::new(RwLock::new(Box::new(
                wascc_codec::capabilities::NullDispatcher::new(),
            ))),
            shutdown_map: ShutdownMap::new(),
            client_factory,
            client_type: PhantomData,
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
        // Initialize this module's shutdown flag.
        shutdown_map.put(&module_id, false)?;

        let client = self.client_factory.new_client(&endpoint);
        let poller = Poller::new(&module_id, client, shutdown_map);

        thread::spawn(move || {
            info!("Starting poller for actor {}", module_id);

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

/// Represents a waSCC AWS Lambda raw event provider.
/// This capability provider dispatches events from
/// the AWS Lambda machinery as raw events (without translation).
struct LambdaRawEventProvider<CF: ClientFactory<C>, C: Client>(
    LambdaProvider<RawEventDispatcher, CF, C>,
);

impl<CF: ClientFactory<C>, C: Send + Client + 'static> LambdaRawEventProvider<CF, C> {
    /// Creates a new, empty `LambdaRawEventProvider`.
    pub fn new(client_factory: CF) -> Self {
        Self(LambdaProvider::new(client_factory))
    }
}

/// Returns an instance of the default raw event capability provider.
pub fn default_raw_event_provider() -> impl CapabilityProvider {
    LambdaRawEventProvider::new(RuntimeClientFactory::new())
}

impl<CF: Any + Send + Sync + ClientFactory<C>, C: Any + Send + Sync + Client> CapabilityProvider
    for LambdaRawEventProvider<CF, C>
{
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
        "waSCC AWS Lambda raw event provider"
    }
}

/// Represents a waSCC AWS Lambda HTTP request provider.
/// This capability provider dispatches events from
/// the AWS Lambda machinery as HTTP requests.
struct LambdaHttpRequestProvider<CF: ClientFactory<C>, C: Client>(
    LambdaProvider<HttpRequestDispatcher, CF, C>,
);

impl<CF: ClientFactory<C>, C: Send + Client + 'static> LambdaHttpRequestProvider<CF, C> {
    /// Creates a new, empty `LambdaHttpRequestProvider`.
    pub fn new(client_factory: CF) -> Self {
        Self(LambdaProvider::new(client_factory))
    }
}

/// Returns an instance of the default HTTP request capability provider.
pub fn default_http_request_provider() -> impl CapabilityProvider {
    LambdaHttpRequestProvider::new(RuntimeClientFactory::new())
}

impl<CF: Any + Send + Sync + ClientFactory<C>, C: Any + Send + Sync + Client> CapabilityProvider
    for LambdaHttpRequestProvider<CF, C>
{
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
        "waSCC AWS Lambda HTTP request provider"
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
        shutdown_map: ShutdownMap,
    }

    impl MockClientFactory {
        /// Creates a new `MockClientFactory`.
        fn new(event_kind: EventKind, shutdown_map: ShutdownMap) -> Self {
            Self {
                event_kind,
                shutdown_map,
            }
        }
    }

    impl ClientFactory<MockClient> for MockClientFactory {
        /// Creates a new `MockClient`.
        fn new_client(&self, _endpoint: &str) -> MockClient {
            MockClient::new(self.event_kind.clone(), self.shutdown_map.clone())
        }
    }

    /// Represents a mock Lambda runtime client that returns a single event.
    struct MockClient {
        event_kind: EventKind,
        initialization_error: RwLock<Option<InitializationError>>,
        invocation_error: RwLock<Option<InvocationError>>,
        invocation_response: RwLock<Option<InvocationResponse>>,
        shutdown_map: ShutdownMap,
    }

    impl MockClient {
        /// Returns a new `MockClient`.
        fn new(event_kind: EventKind, shutdown_map: ShutdownMap) -> Self {
            Self {
                event_kind,
                initialization_error: RwLock::new(None),
                invocation_error: RwLock::new(None),
                invocation_response: RwLock::new(None),
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
            let mut lock = self.invocation_error.write().unwrap();
            *lock = Some(error);

            Ok(())
        }

        /// Sends an invocation error to the AWS Lambda runtime.
        fn send_invocation_response(&self, response: InvocationResponse) -> anyhow::Result<()> {
            let mut lock = self.invocation_response.write().unwrap();
            *lock = Some(response);

            Ok(())
        }

        /// Sends an initialization error to the AWS Lambda runtime.
        fn send_initialization_error(&self, error: InitializationError) -> anyhow::Result<()> {
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
    fn mock_poller(event_kind: EventKind) -> Poller<MockClient> {
        let shutdown_map = ShutdownMap::new();
        let result = shutdown_map.put(MODULE_ID, false);
        assert!(result.is_ok());

        Poller::new(
            MODULE_ID,
            MockClient::new(event_kind, shutdown_map.clone()),
            shutdown_map,
        )
    }

    /// Returns a `MockClientFactory`.
    fn mock_client_factory(event_kind: EventKind) -> impl ClientFactory<MockClient> {
        let shutdown_map = ShutdownMap::new();
        shutdown_map.put(MODULE_ID, false).unwrap();
        MockClientFactory::new(event_kind, shutdown_map)
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

        let result = provider.handle_call("system", "", &[]);
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

        let result = provider.handle_call("system", OP_BIND_ACTOR, &[]);
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

        let result =
            provider.handle_call("system", OP_BIND_ACTOR, &empty_capability_configuration());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing configuration value"));
    }

    #[test]
    fn raw_event_provider_xyz() {
        let client_factory = mock_client_factory(EventKind::None);
        let provider = LambdaRawEventProvider::new(client_factory);
        let result = provider.configure_dispatch(boxed_mock_dispatcher(RESPONSE_BODY));
        assert!(result.is_ok());

        let result = provider.handle_call("system", OP_BIND_ACTOR, &capability_configuration());
        assert!(result.is_ok());
    }
}
