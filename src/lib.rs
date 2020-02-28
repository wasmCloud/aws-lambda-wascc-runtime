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
    clients: Arc<RwLock<HashMap<String, thread::JoinHandle<()>>>>,
    dispatcher: Arc<RwLock<Box<dyn Dispatcher>>>,
}

impl Default for AwsLambdaRuntimeProvider {
    // Returns the default value for `AwsLambdaRuntimeProvider`.
    fn default() -> Self {
        match env_logger::try_init() {
            Ok(_) => {}
            Err(_) => info!("Logger already intialized, skipping"),
        };

        AwsLambdaRuntimeProvider {
            clients: Arc::new(RwLock::new(HashMap::new())),
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
    fn start_runtime_client(&self, config: CapabilityConfiguration) -> Result<Vec<u8>, Box<dyn Error>> {
        debug!("start_runtime_client");

        let module_id = config.module;

        let handle = thread::spawn(move || {

        });
        info!("Started runtime client for actor {}", module_id);

        self.clients.write().unwrap().insert(module_id, handle);

        Ok(vec![])
    }

    // Stops any running Lambda runtime client.
    fn stop_runtime_client(&self, config: CapabilityConfiguration) -> Result<Vec<u8>, Box<dyn Error>> {
        debug!("stop_runtime_client");

        let module_id = config.module;
        let lock = self.clients.read().unwrap();
        if lock.contains_key(&module_id) {

        } else {
            error!("Received request to stop runtime client for unknown actor {}. Ignoring", module_id);
        }

        Ok(vec![])
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

        let mut lock = match self.dispatcher.write() {
            Ok(x) => x,
            Err(e) => return Err(e.description().into()),
        };
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
            _ => Err(format!("Unsupported operation: {}", op).into()),
        }
    }

    // Returns the human-readable, friendly name of this capability provider.
    fn name(&self) -> &'static str {
        "waSCC AWS Lambda runtime provider"
    }
}
