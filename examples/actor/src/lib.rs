//
// waSCC AWS Lambda Actor
//

extern crate aws_lambda_runtime_codec as runtime_codec;
extern crate wascc_actor as actor;

use actor::prelude::*;

actor_handlers! {runtime_codec::lambda::OP_HANDLE_EVENT => hello_world, core::OP_HEALTH_REQUEST => health}

fn health(_ctx: &CapabilitiesContext, _req: core::HealthRequest) -> ReceiveResult {
    Ok(vec![])
}

fn hello_world(
    _ctx: &CapabilitiesContext,
    _evt: aws_lambda_runtime_codec::lambda::Event,
) -> ReceiveResult {
    Ok(vec![])
}
