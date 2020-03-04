//
// waSCC AWS Lambda Actor
//

extern crate wascc_actor as actor;

use actor::prelude::*;

pub fn receive(ctx: &CapabilitiesContext, operation: &str, msg: &[u8]) -> ReceiveResult {
    match operation {
        core::OP_HEALTH_REQUEST => Ok(vec![]),
        _ => Err("Unknown operation".into()),
    }
}
