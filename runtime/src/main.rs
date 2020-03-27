// Copyright 2015-2019 Capital One Services, LLC
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
// waSCC AWS Lambda Runtime
//

extern crate aws_lambda_runtime_provider as runtime_provider;

use env_logger;
use log::{info, warn};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use wascc_host::{host, HostManifest, NativeCapability};

const MANIFEST_FILE: &str = "manifest.yaml";

/// Entry point.
fn main() -> Result<(), Box<dyn Error>> {
    // No timestamp in the log format as CloudWatch already adds it.
    if env_logger::builder().format_timestamp(None).try_init().is_err() {
        info!("Logger already intialized");
    }

    info!("aws-lambda-wascc-runtime starting");

    let rt = runtime_provider::AwsLambdaRuntimeProvider::new();
    host::add_native_capability(NativeCapability::from_instance(rt)?)?;

    if let Some(cwd) = std::env::current_dir()?.to_str() {
        info!("Loading {} from {}", MANIFEST_FILE, cwd);
    }

    // Load from well-known manifest file and expand any environment variables.
    let manifest = HostManifest::from_yaml(MANIFEST_FILE, true)?;
    host::apply_manifest(manifest)?;

    autoconfigure_runtime()?;

    info!("Main thread park");
    std::thread::park();

    info!("aws-lambda-wascc-runtime ending");

    Ok(())
}

/// Autoconfigures any actors that have the awslambda:runtime capability.
fn autoconfigure_runtime() -> Result<(), Box<dyn Error>> {
    let mut values = HashMap::new();
    let keys = vec![
        "AWS_LAMBDA_FUNCTION_NAME",
        "AWS_LAMBDA_FUNCTION_VERSION",
        "AWS_LAMBDA_LOG_GROUP_NAME",
        "AWS_LAMBDA_LOG_STREAM_NAME",
        "AWS_LAMBDA_RUNTIME_API",
        "LAMBDA_RUNTIME_DIR",
        "LAMBDA_TASK_ROOT",
    ];
    for key in keys {
        if let Ok(val) = env::var(key) {
            values.insert(key.into(), val);
        } else {
            warn!("Environment variable {} not set", key);
        }
    }

    for actor in host::actors() {
        match host::configure(&actor.0, runtime_provider::CAPABILITY_ID, values.clone()) {
            Ok(_) => info!("Autoconfigured actor {}", actor.0),
            Err(e) => info!(
                "Autoconfiguration skipped actor {}: {}",
                actor.0,
                e.description()
            ),
        };
    }

    Ok(())
}
