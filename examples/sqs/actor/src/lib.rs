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
// waSCC AWS Lambda Actor
//

extern crate aws_lambda_runtime_codec as runtime_codec;
extern crate wascc_actor as actor;

use actor::prelude::*;
use aws_lambda_events::event::sqs;
use serde_json;
use serde_json::json;

actor_handlers! {runtime_codec::lambda::OP_HANDLE_EVENT => handle_event, core::OP_HEALTH_REQUEST => health}

fn health(ctx: &CapabilitiesContext, _req: core::HealthRequest) -> ReceiveResult {
    ctx.log("Actor health");

    Ok(vec![])
}

fn handle_event(
    ctx: &CapabilitiesContext,
    event: aws_lambda_runtime_codec::lambda::Event,
) -> ReceiveResult {
    ctx.log("Actor handle event");

    let body = event.body;

    // Is this a batch of messages from SQS?
    match serde_json::from_slice(&body) {
        Ok(r) => handle_sqs_event(ctx, r),
        _ => Err("Unsupported Lambda event".into()),
    }
}

fn handle_sqs_event(ctx: &CapabilitiesContext, event: sqs::SqsEvent) -> ReceiveResult {
    ctx.log("Handle SQS event");

    for record in event.records.iter() {
        match &record.message_id {
            Some(message_id) => ctx.log(&format!("Message ID: {}", message_id)),
            None => ctx.log("No message ID"),
        };

        if let Some(body) = &record.body {
            ctx.log(&format!("Body: {}", body));
            let payload = body.to_uppercase();

            if let Some(attr) = record.message_attributes.get("ReplyQueueUrl") {
                if let Some(url) = &attr.string_value {
                    ctx.log(&format!("Reply queue URL: {}", url));
                    ctx.log(&format!("Response payload: {}", payload));

                    ctx.msg().publish(url, None, payload.as_bytes())?;
                } else {
                    ctx.log("Missing reply queue URL");
                }
            } else {
                ctx.log("No reply queue URL");
            }
        } else {
            ctx.log("No body");
        }
    }

    Ok(serialize(runtime_codec::lambda::Response::empty())?)
}
