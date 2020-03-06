//
// waSCC AWS Lambda Actor
//

extern crate aws_lambda_runtime_codec as runtime_codec;
extern crate wascc_actor as actor;

use actor::prelude::*;

actor_handlers! {runtime_codec::lambda::OP_HANDLE_EVENT => handle_event, core::OP_HEALTH_REQUEST => health}

fn health(ctx: &CapabilitiesContext, _req: core::HealthRequest) -> ReceiveResult {
    ctx.log("Actor health");

    Ok(vec![])
}

fn handle_event(
    ctx: &CapabilitiesContext,
    _evt: aws_lambda_runtime_codec::lambda::Event,
) -> ReceiveResult {
    ctx.log("Actor handle event");

    Ok(serialize(runtime_codec::lambda::Response::empty())?)
}
