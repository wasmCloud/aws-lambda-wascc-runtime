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
// waSCC AWS Lambda Runtime AWS X-Ray Middleware
//

use log::trace;
use wascc_host::{Middleware, Invocation, InvocationResponse};

use std::str::FromStr;

pub struct XRayMiddleware {
    xray_client: xray::Client,
}

impl XRayMiddleware {
    pub fn new(daemon_address: &str) -> anyhow::Result<Self> {
        let xray_client = xray::Client::from_str(daemon_address)?;
        Ok(XRayMiddleware{xray_client})
    }
}

impl Middleware for XRayMiddleware {
    /// Called by the host before invoking an actor.
    fn actor_pre_invoke(&self, inv: Invocation) -> wascc_host::Result<Invocation> {
        trace!("XRayMiddleware::actor_pre_invoke: {} {} {} {:?}", inv.id, inv.operation, inv.origin, inv.target);
        Ok(inv)
    }

    /// Called by the host after invoking an actor.
    fn actor_post_invoke(&self, response: InvocationResponse) -> wascc_host::Result<InvocationResponse> {
        trace!("XRayMiddleware::actor_post_invoke: {} {:?} ", response.invocation_id, response.error);
        Ok(response)
    }

    /// Called by the host before invoking a capability.
    fn capability_pre_invoke(&self, inv: Invocation) -> wascc_host::Result<Invocation> {
        trace!("XRayMiddleware::capability_pre_invoke: {} {} {} {:?}", inv.id, inv.operation, inv.origin, inv.target);
        Ok(inv)
    }

    /// Called by the host after invoking a capability.
    fn capability_post_invoke(&self, response: InvocationResponse) -> wascc_host::Result<InvocationResponse> {
        trace!("XRayMiddleware::capability_post_invoke: {} {:?} ", response.invocation_id, response.error);
        Ok(response)
    }
}


