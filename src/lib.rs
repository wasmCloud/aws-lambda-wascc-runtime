#[macro_use]
extern crate log;
#[macro_use]
extern crate wascc_codec as codec;

use codec::capabilities::{CapabilityProvider, Dispatcher, NullDispatcher};
use codec::core::{CapabilityConfiguration, OP_CONFIGURE, OP_REMOVE_ACTOR};
use env_logger;
use prost::Message;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, RwLock};
use std::thread;

const CAPABILITY_ID: &str = "awslambda:runtime";

capability_provider!(AwsLambdaRuntimeProvider, AwsLambdaRuntimeProvider::new);

pub struct AwsLambdaRuntimeProvider {
    client_shutdown: Arc<RwLock<HashMap<String, bool>>>,
    dispatcher: Arc<RwLock<Box<dyn Dispatcher>>>,
}

impl Default for AwsLambdaRuntimeProvider {
    // Returns the default value for `AwsLambdaRuntimeProvider`.
    fn default() -> Self {
        if env_logger::try_init().is_err() {
            info!("Logger already intialized");
        }

        AwsLambdaRuntimeProvider {
            client_shutdown: Arc::new(RwLock::new(HashMap::new())),
            dispatcher: Arc::new(RwLock::new(Box::new(NullDispatcher::new()))),
        }
    }
}

impl AwsLambdaRuntimeProvider {
    // Creates a new, empty `AwsLambdaRuntimeProvider`.
    pub fn new() -> Self {
        Self::default()
    }

    // Starts the Lambda runtime client.
    fn start_runtime_client(&self, config: CapabilityConfiguration) {
        debug!("start_runtime_client");

        let module_id = config.module;
        let client_shutdown = self.client_shutdown.clone();
        thread::spawn(move || {
            info!("Starting runtime client for actor {}", module_id);

            // Initialize this client's shutdown flag.
            client_shutdown
                .write()
                .unwrap()
                .insert(module_id.clone(), false);

            loop {
                if *client_shutdown.read().unwrap().get(&module_id).unwrap() {
                    break;
                }
            }
        });
    }

    // Stops any running Lambda runtime client.
    fn stop_runtime_client(&self, config: CapabilityConfiguration) {
        debug!("stop_runtime_client");

        let module_id = &config.module;
        {
            let mut lock = self.client_shutdown.write().unwrap();
            if !lock.contains_key(module_id) {
                error!(
                    "Received request to stop runtime client for unknown actor {}. Ignoring",
                    module_id
                );
                return;
            }
            *lock.get_mut(module_id).unwrap() = true;
        }
        {
            let mut lock = self.client_shutdown.write().unwrap();
            lock.remove(module_id).unwrap();
        }
    }
}

impl CapabilityProvider for AwsLambdaRuntimeProvider {
    // Returns the capability ID in the formated `namespace:id`.
    fn capability_id(&self) -> &'static str {
        CAPABILITY_ID
    }

    // Called when the host runtime is ready and has configured a dispatcher.
    fn configure_dispatch(&self, dispatcher: Box<dyn Dispatcher>) -> Result<(), Box<dyn Error>> {
        info!("Dispatcher received");

        let mut lock = self.dispatcher.write().unwrap();
        *lock = dispatcher;

        Ok(())
    }

    // Called by the host runtime when an actor is requesting a command be executed.
    fn handle_call(&self, actor: &str, op: &str, msg: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        info!("Handling operation `{}` from `{}`", op, actor);

        match op {
            OP_CONFIGURE if actor == "system" => {
                self.start_runtime_client(CapabilityConfiguration::decode(msg)?)
            }
            OP_REMOVE_ACTOR if actor == "system" => {
                self.stop_runtime_client(CapabilityConfiguration::decode(msg)?)
            }
            _ => return Err(format!("Unsupported operation: {}", op).into()),
        }

        Ok(vec![])
    }

    // Returns the human-readable, friendly name of this capability provider.
    fn name(&self) -> &'static str {
        "waSCC AWS Lambda runtime provider"
    }
}
