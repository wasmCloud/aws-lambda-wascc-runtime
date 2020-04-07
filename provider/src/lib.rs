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

use aws_lambda_events::event::alb::AlbTargetGroupRequest;
use env_logger;
use serde_json;
use std::collections::HashMap;
use std::env;
use wascc_codec::capabilities::{CapabilityProvider, Dispatcher, NullDispatcher};
use wascc_codec::core::{CapabilityConfiguration, OP_BIND_ACTOR, OP_REMOVE_ACTOR};
use wascc_codec::{deserialize, serialize};

use std::error::Error;
use std::sync::{Arc, RwLock};
use std::thread;

mod lambda;

const CAPABILITY_ID: &str = "awslambda:runtime";

/// Represents a waSCC AWS Lambda runtime provider.
pub struct AwsLambdaRuntimeProvider {
    dispatcher: Arc<RwLock<Box<dyn Dispatcher>>>,
    shutdown: Arc<RwLock<HashMap<String, bool>>>,
}

/// Polls the Lambda event machinery.
struct Poller {
    client: lambda::Client,
    dispatcher: Arc<RwLock<Box<dyn Dispatcher>>>,
    module_id: String,
    shutdown: Arc<RwLock<HashMap<String, bool>>>,
}

impl Default for AwsLambdaRuntimeProvider {
    // Returns the default value for `AwsLambdaRuntimeProvider`.
    fn default() -> Self {
        if env_logger::try_init().is_err() {
            debug!("Logger already intialized");
        }

        AwsLambdaRuntimeProvider {
            dispatcher: Arc::new(RwLock::new(Box::new(NullDispatcher::new()))),
            shutdown: Arc::new(RwLock::new(HashMap::new())),
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

        let dispatcher = Arc::clone(&self.dispatcher);
        let endpoint = match config.values.get("AWS_LAMBDA_RUNTIME_API") {
            Some(ep) => String::from(ep),
            None => {
                return Err(anyhow!(
                    "Missing configuration value: AWS_LAMBDA_RUNTIME_API"
                ))
            }
        };
        let module_id = config.module;
        let shutdown = Arc::clone(&self.shutdown);
        thread::spawn(move || {
            info!("Starting poller for actor {}", module_id);

            // Initialize this poller's shutdown flag.
            shutdown.write().unwrap().insert(module_id.clone(), false);

            let poller = Poller::new(&module_id, &endpoint, dispatcher, shutdown);
            poller.run();
        });

        Ok(())
    }

    /// Stops any running Lambda poller.
    fn stop_polling(&self, config: CapabilityConfiguration) -> anyhow::Result<()> {
        debug!("awslambda:runtime stop_polling");

        let module_id = &config.module;
        {
            let mut lock = self.shutdown.write().unwrap();
            if !lock.contains_key(module_id) {
                error!(
                    "Received request to stop poller for unknown actor {}. Ignoring",
                    module_id
                );
                return Ok(());
            }
            *lock.get_mut(module_id).unwrap() = true;
        }
        {
            let mut lock = self.shutdown.write().unwrap();
            lock.remove(module_id).unwrap();
        }

        Ok(())
    }
}

impl CapabilityProvider for AwsLambdaRuntimeProvider {
    /// Returns the capability ID in the formated `namespace:id`.
    fn capability_id(&self) -> &'static str {
        CAPABILITY_ID
    }

    /// Called when the host runtime is ready and has configured a dispatcher.
    fn configure_dispatch(&self, dispatcher: Box<dyn Dispatcher>) -> Result<(), Box<dyn Error>> {
        debug!("awslambda:runtime configure_dispatch");

        let mut lock = self.dispatcher.write().unwrap();
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

impl Poller {
    /// Creates a new `Poller`.
    fn new(
        module_id: &str,
        endpoint: &str,
        dispatcher: Arc<RwLock<Box<dyn Dispatcher>>>,
        shutdown: Arc<RwLock<HashMap<String, bool>>>,
    ) -> Self {
        Poller {
            client: lambda::Client::new(endpoint),
            dispatcher,
            module_id: module_id.into(),
            shutdown,
        }
    }

    /// Runs the poller until shutdown.
    fn run(&self) {
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
            if self.try_dispatch_http_request(event.body(), request_id) {
                continue;
            }

            self.dispatch_lambda_event(event.body(), request_id);
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
        *self.shutdown.read().unwrap().get(&self.module_id).unwrap()
    }

    /// Attempts to dispatch an HTTP request to an actor.
    fn try_dispatch_http_request(&self, body: &Vec<u8>, request_id: &str) -> bool {
        match serde_json::from_slice(body) {
            Ok(request) => return self.dispatch_alb_http_request(request, request_id),
            _ => {}
        }

        false
    }

    /// Dispatches a Lambda event to an actor.
    fn dispatch_lambda_event(&self, body: &Vec<u8>, request_id: &str) {
        let event = codec::Event {
            body: body.to_vec(),
        };
        let msg = match serialize(event) {
            Ok(msg) => msg,
            Err(e) => {
                let err = anyhow!("Failed to serialize actor message: {}", e);
                error!("{}", err);
                self.send_invocation_error(err, request_id);

                return;
            }
        };

        // Call handler.
        debug!("Poller call handler");
        let handler_resp = {
            let lock = self.dispatcher.read().unwrap();
            lock.dispatch(&self.module_id, codec::OP_HANDLE_EVENT, &msg)
        };
        // Handle response or error.
        match handler_resp {
            Ok(r) => {
                let resp = deserialize::<codec::Response>(r.as_slice()).unwrap();
                self.send_invocation_response(resp.body, request_id);
            }
            Err(e) => {
                let err = anyhow!("Guest failed to handle Lambda event: {}", e);
                error!("{}", err);
                self.send_invocation_error(err, request_id);
            }
        }
    }

    /// Dispatches an ALB HTTP request to an actor.
    fn dispatch_alb_http_request(&self, request: AlbTargetGroupRequest, request_id: &str) -> bool {
        true
    }

    /// Dispatches an HTTP request to an actor.
    fn dispatch_http_request(&self, request: wascc_codec::http::Request, request_id: &str) -> bool {
        true
    }
}
