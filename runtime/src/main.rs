//
// waSCC AWS Lambda Runtime
//

use env_logger;
use log::info;
use std::error::Error;
use wascc_host::HostManifest;

const MANIFEST_FILE: &str = "manifest.yaml";

// Entry point.
fn main() -> Result<(), Box<dyn Error>> {
    if env_logger::try_init().is_err() {
        info!("Logger already intialized");
    }

    info!("aws-lambda-wascc-runtime starting");

    if let Some(cwd) = std::env::current_dir()?.to_str() {
        info!("Loading {} from {}", MANIFEST_FILE, cwd);
    }

    HostManifest::from_yaml_with_env_expansion(MANIFEST_FILE)?;

    info!("Main thread park");
    std::thread::park();

    info!("aws-lambda-wascc-runtime ending");

    Ok(())
}
