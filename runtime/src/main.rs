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

use env_logger;
use log::{info, warn};
use provider::AwsLambdaRuntimeProvider;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use wascc_codec::capabilities::CapabilityProvider;
use wascc_host::{HostManifest, NativeCapability, WasccHost};
use wascc_logging::LoggingProvider;

const MANIFEST_FILE: &str = "manifest.yaml";

/// Represents a waSCC capability that is statically linked into this host.
struct Capability {
    id: String,
    config: HashMap<String, String>,
}

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

    info!(
        "aws-lambda-wascc-runtime {} starting",
        env!("CARGO_PKG_VERSION")
    );

    let host = WasccHost::new();

    let runtime = AwsLambdaRuntimeProvider::new();
    let logging = LoggingProvider::new();

    let capabilities = vec![
        Capability {
            id: runtime.capability_id().into(),
            config: runtime_provider_config(),
        },
        Capability {
            id: logging.capability_id().into(),
            config: HashMap::new(), // No configuration.
        },
    ];

    add_capability(&host, runtime)?;
    add_capability(&host, logging)?;

    // Load from well-known manifest file and expand any environment variables.
    if let Some(cwd) = std::env::current_dir()?.to_str() {
        info!("Loading {} from {}", MANIFEST_FILE, cwd);
    }
    let manifest = HostManifest::from_yaml(MANIFEST_FILE, true)
        .map_err(|e| anyhow!("Failed to load manifest file: {}", e))?;
    host.apply_manifest(manifest)
        .map_err(|e| anyhow!("Failed to apply manifest: {}", e))?;

    for capability in capabilities {
        autoconfigure_actors(&host, capability);
    }

    info!("Main thread park");
    std::thread::park();

    info!("aws-lambda-wascc-runtime ending");

    Ok(())
}

/// Adds a built-in capability provider.
fn add_capability(host: &WasccHost, instance: impl CapabilityProvider) -> anyhow::Result<()> {
    let id = instance.capability_id();
    let capability = NativeCapability::from_instance(instance, None)
        .map_err(|e| anyhow!("Failed to create native capability {}: {}", id, e))?;
    host.add_native_capability(capability)
        .map_err(|e| anyhow!("Failed to load native capability {}: {}", id, e))?;

    Ok(())
}

/// Autoconfigures any actors that have the specified capability.
fn autoconfigure_actors(host: &WasccHost, capability: Capability) {
    for actor in host.actors() {
        match host.bind_actor(&actor.0, &capability.id, None, capability.config.clone()) {
            Ok(_) => info!(
                "Autoconfigured actor {} for capability {}",
                actor.0, capability.id
            ),
            Err(e) => info!(
                "Autoconfiguration skipped actor {} for capability {}: {}",
                actor.0,
                capability.id,
                e.description()
            ),
        };
    }
}

/// Returns the configuration for the Lambda runtime provider.
fn runtime_provider_config() -> HashMap<String, String> {
    let mut config = HashMap::new();
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
        if let Ok(value) = env::var(key) {
            config.insert(key.into(), value);
        } else {
            warn!("Environment variable {} not set", key);
        }
    }

    config
}
