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

use log::{debug, error, info, warn};
use provider::{
    default_http_request_provider, default_raw_event_provider, initerr_reporter,
    InitializationErrorReporter,
};
use wascc_codec::capabilities::CapabilityProvider;
use wascc_host::{HostManifest, NativeCapability, WasccHost};
use wascc_logging::LoggingProvider;

use std::collections::HashMap;
use std::env;

const MANIFEST_FILE: &str = "manifest.yaml";

/// Entry point.
fn main() -> anyhow::Result<()> {
    // No timestamp in the log format as CloudWatch already adds it.
    if env_logger::builder()
        .format_timestamp(None)
        .try_init()
        .is_err()
    {
        debug!("Logger already intialized");
    }

    info!(
        "AWS Lambda waSCC Runtime {} starting",
        env!("CARGO_PKG_VERSION")
    );

    let reporter = initerr_reporter(&format!("http://{}", env::var("AWS_LAMBDA_RUNTIME_API")?));

    match load_and_run() {
        Ok(_) => {}
        Err(e) => {
            error!("{}", e);
            reporter.send_initialization_error(e)?;
        }
    };

    debug!("Main thread park");
    std::thread::park();

    info!("AWS Lambda waSCC Runtime done");

    Ok(())
}

/// Loads configuration and runs the waSCC actor system.
fn load_and_run() -> anyhow::Result<()> {
    let host = WasccHost::new();

    let http_request_provider = default_http_request_provider();
    let raw_event_provider = default_raw_event_provider();
    let logging_provider = LoggingProvider::new();

    let lambda_provider_config = lambda_provider_config();
    let logging_provider_config = HashMap::new(); // No configuration.

    // All of these capabilities can be configured for any actor.
    let any_capabilities: Vec<(String, &HashMap<String, String>)> = vec![(
        logging_provider.capability_id().into(),
        &logging_provider_config,
    )];
    // Exactly one of these capabilities can be configured for a single actor.
    let exactly_one_capabilities: Vec<(String, &HashMap<String, String>)> = vec![
        (
            http_request_provider.capability_id().into(),
            &lambda_provider_config,
        ),
        (
            raw_event_provider.capability_id().into(),
            &lambda_provider_config,
        ),
    ];

    add_capability(&host, http_request_provider)?;
    add_capability(&host, raw_event_provider)?;
    add_capability(&host, logging_provider)?;

    // Load from well-known manifest file and expand any environment variables.
    if let Some(cwd) = std::env::current_dir()?.to_str() {
        info!("Loading {} from {}", MANIFEST_FILE, cwd);
    }
    let manifest = HostManifest::from_yaml(MANIFEST_FILE, true)
        .map_err(|e| anyhow!("Failed to load manifest file: {}", e))?;
    host.apply_manifest(manifest)
        .map_err(|e| anyhow!("Failed to apply manifest: {}", e))?;

    autoconfigure_actors(&host, any_capabilities, exactly_one_capabilities);

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

/// Autoconfigures actors.
/// For every actor loaded into the host
/// - Attempt to configure with each of the `any` capabilities
/// - Attempt to configure one actor with one of the `exactly_one` capabilities
fn autoconfigure_actors(
    host: &WasccHost,
    any: Vec<(String, &HashMap<String, String>)>,
    exactly_one: Vec<(String, &HashMap<String, String>)>,
) {
    for actor in host.actors() {
        for capability in &any {
            configure_actor(host, &actor.0, &capability.0, capability.1);
        }
    }

    for actor in host.actors() {
        for capability in &exactly_one {
            if configure_actor(host, &actor.0, &capability.0, capability.1) {
                return;
            }
        }
    }
}

/// Configures an actor with a capability.
/// Returns whether or not the actor was successfully configured.
fn configure_actor(
    host: &WasccHost,
    actor_id: &str,
    capability_id: &str,
    config: &HashMap<String, String>,
) -> bool {
    match host.bind_actor(actor_id, capability_id, None, config.clone()) {
        Ok(_) => {
            info!(
                "Autoconfigured actor {} for capability {}",
                actor_id, capability_id
            );
            true
        }
        Err(e) => {
            info!(
                "Autoconfiguration skipped actor {} for capability {}: {}",
                actor_id, capability_id, e
            );
            false
        }
    }
}

/// Returns the configuration for any Lambda capability provider.
fn lambda_provider_config() -> HashMap<String, String> {
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
