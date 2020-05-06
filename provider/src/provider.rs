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

use std::collections::HashMap;
use std::env;
use std::sync::{Arc, RwLock};
use std::thread;

use crate::dispatch::{
    Dispatcher, DispatcherError, EventDispatcher, HttpDispatcher, NotHttpRequestError,
};
use crate::lambda::{Client, InvocationError, InvocationResponse, RuntimeClient};

// These capability providers are designed to be statically linked into its host.

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
pub(crate) struct AwsLambdaEventProvider {
    host_dispatcher: HostDispatcher,
    shutdown_map: ShutdownMap,
}

impl Default for AwsLambdaEventProvider {
    /// Returns the default value for `AwsLambdaEventProvider`.
    fn default() -> Self {
        Self {
            host_dispatcher: Arc::new(RwLock::new(Box::new(
                wascc_codec::capabilities::NullDispatcher::new(),
            ))),
            shutdown_map: ShutdownMap::new(),
        }
    }
}

impl AwsLambdaEventProvider {
    /// Creates a new, empty `AwsLambdaEventProvider`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts polling the Lambda event machinery.
    fn start_polling(&self, config: CapabilityConfiguration) -> anyhow::Result<()> {
        debug!("awslambda:event-provider start_polling");

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
        debug!("awslambda:event-provider stop_polling");

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

impl CapabilityProvider for AwsLambdaEventProvider {
    /// Returns the capability ID in the formated `namespace:id`.
    fn capability_id(&self) -> &'static str {
        "awslambda:event"
    }

    /// Called when the host runtime is ready and has configured a dispatcher.
    fn configure_dispatch(
        &self,
        dispatcher: Box<dyn wascc_codec::capabilities::Dispatcher>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        debug!("awslambda:runtime configure_dispatch");

        let mut lock = self.host_dispatcher.write().unwrap();
        *lock = dispatcher;

        Ok(())
    }

    /// Called by the host runtime when an actor is requesting a command be executed.
    fn handle_call(
        &self,
        actor: &str,
        op: &str,
        msg: &[u8],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
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
        "waSCC AWS Lambda event provider"
    }
}

/// Polls the Lambda event machinery using the specified client.
struct Poller<C> {
    client: C,
    module_id: String,
    host_dispatcher: HostDispatcher,
    shutdown_map: ShutdownMap,
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
        let raw_event_dispatcher = EventDispatcher::new(Arc::clone(&self.host_dispatcher));

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
