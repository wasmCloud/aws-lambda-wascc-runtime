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
// Sample actor that uses the AWS Lambda runtime capability provider
//

#[macro_use]
extern crate log;

// To avoid conflict with wascc_codec which is aliased as codec in the actor SDK prelude.
extern crate codec as lambda_codec;

use serde_json::json;
use wascc_actor::prelude::*;

actor_handlers! {lambda_codec::OP_HANDLE_EVENT => handle_event, codec::core::OP_HEALTH_REQUEST => health}

fn health(_req: codec::core::HealthRequest) -> HandlerResult<()> {
    info!("Actor health");

    Ok(())
}

fn handle_event(event: lambda_codec::Event) -> HandlerResult<lambda_codec::Response> {
    info!("Actor handle event");

    const DEFAULT: &'static str = "**UNKNOWN**";

    let output: String = match serde_json::from_slice(event.body.as_slice())? {
        serde_json::Value::Object(m) => {
            let mut output: String = DEFAULT.into();
            if let Some(input) = m.get("input") {
                if input.is_string() {
                    output = input.as_str().unwrap().to_uppercase();
                }
            }
            output
        }
        _ => DEFAULT.into(),
    };
    let response = json!({
        "output": output,
    });

    info!("Output: {}", &output);

    Ok(lambda_codec::Response::json(&response)?)
}
