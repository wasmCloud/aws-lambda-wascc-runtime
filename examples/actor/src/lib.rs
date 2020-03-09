//
// waSCC AWS Lambda Actor
//

extern crate aws_lambda_runtime_codec as runtime_codec;
extern crate wascc_actor as actor;

use actor::prelude::*;
use aws_lambda_events::event::apigw;
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

    // Is this a request from API Gateway?
    //ApiGatewayProxyRequest
    match serde_json::from_slice(&body) {
        Ok(r) => return handle_apigw_proxy_request(ctx, r),
        _ => {}
    }

    handle_custom_event(ctx, body)
}

fn handle_apigw_proxy_request(ctx: &CapabilitiesContext, _request: apigw::ApiGatewayProxyRequest) -> ReceiveResult {
    ctx.log("Handle API Gateway proxy event");

    Ok(serialize(runtime_codec::lambda::Response::empty())?)
}

fn handle_custom_event(ctx: &CapabilitiesContext, body: Vec<u8>) -> ReceiveResult {
    ctx.log("Handle custom event");

    let output: String = match serde_json::from_slice(&body)? {
        serde_json::Value::Object(m) => {
            let mut output: String = "Unknown".into();
            if let Some(input) = m.get("input") {
                if input.is_string() {
                    output = input.as_str().unwrap().to_uppercase();
                }
            }
            output
        }
        _ => "Unknown".into(),
    };
    let response = json!({
        "output": output,
    });

    ctx.log(&format!("Output: {}", &output));

    Ok(serialize(runtime_codec::lambda::Response::json(
        &response,
    )?)?)
}
