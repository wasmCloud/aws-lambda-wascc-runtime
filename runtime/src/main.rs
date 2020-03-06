//
// waSCC AWS Lambda Runtime
//

use env_logger;
use log::info;
use std::collections::HashMap;
use std::error::Error;
use wascc_host::{host, Actor, NativeCapability};

// Entry point.
fn main() -> Result<(), Box<dyn Error>> {
    if env_logger::try_init().is_err() {
        info!("Logger already intialized");
    }

    info!("aws-lambda-wascc-runtime starting");

    let mut config = HashMap::new();
    load_function_settings(&mut config)?;

    if let Some(cwd) = std::env::current_dir()?.to_str() {
        info!("Loading actor and capabiity provider from {}", cwd);
    }

    host::add_actor(Actor::from_file("wascc_actor_hello_lambda_signed.wasm")?)?;
    host::add_native_capability(NativeCapability::from_file(
        "libaws_lambda_runtime_provider.so",
    )?)?;

    // TODO
    // TODO When applying the manifest, expand environment variables.
    // TODO
    host::configure(
        "MBN36NJGPJMD3ECFRX2UZOFXDOCBBU3JWRILAOBKRN7SURHYBRKCADIT",
        "awslambda:runtime",
        config,
    )?;

    info!("Main thread park");
    std::thread::park();

    Ok(())
}

// Loads the function settings from the Lambda environment variables:
// https://docs.aws.amazon.com/lambda/latest/dg/current-supported-versions.html
fn load_function_settings(config: &mut HashMap<String, String>) -> Result<(), Box<dyn Error>> {
    for v in vec![
        "AWS_LAMBDA_FUNCTION_NAME",
        "AWS_LAMBDA_FUNCTION_VERSION",
        "AWS_LAMBDA_LOG_GROUP_NAME",
        "AWS_LAMBDA_LOG_STREAM_NAME",
        "AWS_LAMBDA_RUNTIME_API",
        "LAMBDA_RUNTIME_DIR",
        "LAMBDA_TASK_ROOT",
    ]
    .iter()
    {
        config.insert(v.to_string(), std::env::var(v)?);
    }

    Ok(())
}
