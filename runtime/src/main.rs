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
// waSCC AWS Lambda Runtime
//

#[macro_use]
extern crate anyhow;

extern crate provider;

use env_logger;
use log::{info, warn};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use wascc_host::{host, HostManifest, NativeCapability};

const MANIFEST_FILE: &str = "manifest.yaml";

/// Entry point.
fn main() -> anyhow::Result<()> {
    // No timestamp in the log format as CloudWatch already adds it.
    if env_logger::builder()
        .format_timestamp(None)
        .try_init()
        .is_err()
    {
        info!("Logger already intialized");
    }

    info!("aws-lambda-wascc-runtime starting");

    let rt = provider::AwsLambdaRuntimeProvider::new();
    let cap = NativeCapability::from_instance(rt)
        .map_err(|e| anyhow!("Failed to create Lambda runtime provider: {}", e))?;
    host::add_native_capability(cap)
        .map_err(|e| anyhow!("Failed to load Lambda runtime provider: {}", e))?;

    // Load from well-known manifest file and expand any environment variables.
    if let Some(cwd) = std::env::current_dir()?.to_str() {
        info!("Loading {} from {}", MANIFEST_FILE, cwd);
    }
    let manifest = HostManifest::from_yaml(MANIFEST_FILE, true)
        .map_err(|e| anyhow!("Failed to load manifest file: {}", e))?;
    host::apply_manifest(manifest).map_err(|e| anyhow!("Failed to apply manifest: {}", e))?;

    autoconfigure_runtime()?;

    info!("Main thread park");
    std::thread::park();

    info!("aws-lambda-wascc-runtime ending");

    Ok(())
}

/// Autoconfigures any actors that have the awslambda:runtime capability.
fn autoconfigure_runtime() -> anyhow::Result<()> {
    let mut values = HashMap::new();
    // https://docs.aws.amazon.com/lambda/latest/dg/configuration-envvars.html#configuration-envvars-runtime
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
        match host::configure(&actor.0, provider::CAPABILITY_ID, values.clone()) {
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
