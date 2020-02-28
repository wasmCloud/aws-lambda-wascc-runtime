use env_logger;
use lambda_runtime_core::{lambda, Context, HandlerError};
use log::info;
use std::collections::HashMap;
use std::env;
use std::error::Error;

// Entry point.
fn main() -> Result<(), Box<dyn Error>> {
    if env_logger::try_init().is_err() {
        info!("Logger already intialized");
    }

    info!("aws-lambda-wascc-runtime starting");

    let mut config = HashMap::new();
    load_function_settings(&mut config)?;

    lambda!(wascc_handler);

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
        config.insert(v.to_string(), env::var(v)?);
    }

    Ok(())
}

fn wascc_handler(_data: Vec<u8>, _ctx: Context) -> Result<Vec<u8>, HandlerError> {
    info!("wascc_handler entered");

    Ok("all good".as_bytes().to_vec())
}
