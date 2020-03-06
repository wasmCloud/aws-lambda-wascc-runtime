//
// waSCC AWS Lambda Runtime
//

use env_logger;
use log::info;
use std::collections::HashMap;
use std::error::Error;
use wascc_host::{host, HostManifest, NativeCapability};

const AWS_LAMBDA_RUNTIME_PROVIDER_FILE: &str = "libaws_lambda_runtime_provider.so";
const MANIFEST_FILE: &str = "manifest.yaml";

// Entry point.
fn main() -> Result<(), Box<dyn Error>> {
    if env_logger::try_init().is_err() {
        info!("Logger already intialized");
    }

    info!("aws-lambda-wascc-runtime starting");

    let mut config = HashMap::new();
    load_function_settings(&mut config)?;

    info!(
        "{}",
        format!(
            "Loading waSCC host manifest {:?}/{}",
            std::env::current_dir()?,
            MANIFEST_FILE
        )
    );
    let manifest = HostManifest::from_yaml(MANIFEST_FILE)?;
    host::apply_manifest(manifest)?;

    info!(
        "{}",
        format!(
            "Loading native capability provider {:?}/{}",
            std::env::current_dir()?,
            AWS_LAMBDA_RUNTIME_PROVIDER_FILE
        )
    );
    host::add_native_capability(NativeCapability::from_file(
        AWS_LAMBDA_RUNTIME_PROVIDER_FILE,
    )?)?;
    host::configure(
        "MB4OLDIC3TCZ4Q4TGGOVAZC43VXFE2JQVRAXQMQFXUCREOOFEKOKZTY2",
        "wascc:http_server",
        config,
    )?;

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
