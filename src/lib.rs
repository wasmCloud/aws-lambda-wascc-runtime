#[macro_use]
extern crate log;
#[macro_use]
extern crate wascc_codec as codec;

use codec::capabilities::{CapabilityProvider, Dispatcher, NullDispatcher};
use codec::core::{CapabilityConfiguration, OP_CONFIGURE, OP_REMOVE_ACTOR};
use env_logger;
use prost::Message;
use std::error::Error;
use std::sync::{Arc, RwLock};

const CAPABILITY_ID: &str = "awslambda:runtime";

capability_provider!(AwsLambdaRuntimeProvider, AwsLambdaRuntimeProvider::new);

pub struct AwsLambdaRuntimeProvider {
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
            dispatcher: Arc::new(RwLock::new(Box::new(NullDispatcher::new()))),
        }
    }
}

impl AwsLambdaRuntimeProvider {
    // Creates a new, empty `AwsLambdaRuntimeProvider`.
    pub fn new() -> Self {
        Self::default()
    }

    // Configures this provider.
    fn configure(&self, config: CapabilityConfiguration) -> Result<Vec<u8>, Box<dyn Error>> {
        debug!("configure");

        let _config = config.values;

        Ok(vec![])
    }

    // Removes an actor.
    fn remove_actor(&self, config: CapabilityConfiguration) -> Result<Vec<u8>, Box<dyn Error>> {
        debug!("remove_actor");

        let _config = config.values;

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
                self.configure(CapabilityConfiguration::decode(msg)?)
            }
            OP_REMOVE_ACTOR if actor == "system" => {
                self.remove_actor(CapabilityConfiguration::decode(msg)?)
            }
            _ => Err(format!("Unsupported operation: {}", op).into()),
        }
    }

    // Returns the human-readable, friendly name of this capability provider.
    fn name(&self) -> &'static str {
        "waSCC AWS Lambda runtime provider"
    }
}
