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

use env_logger;
use log::info;
use std::error::Error;
use wascc_host::{host, HostManifest};

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

    // Load from well-known manifest file and expand any environment variables.
    let manifest = HostManifest::from_yaml(MANIFEST_FILE, true)?;
    host::apply_manifest(manifest)?;

    info!("Main thread park");
    std::thread::park();

    info!("aws-lambda-wascc-runtime ending");

    Ok(())
}
