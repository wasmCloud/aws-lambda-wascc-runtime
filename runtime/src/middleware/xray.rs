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
use wascc_host::{Invocation, InvocationResponse, InvocationTarget, Middleware};

use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Represents the metrics collected by the middleware.
struct Metrics {
    in_flight_invocations: HashMap<String, InFlightInvocation>,
}

impl Default for Metrics {
    /// Returns a new, default `Metrics` structure.
    fn default() -> Self {
        Metrics {
            in_flight_invocations: HashMap::new(),
        }
    }
}

/// Represents an in-flight invocation.
struct InFlightInvocation {
    start_time: Instant,
    operation: String,
    target: InvocationTarget,
}

/// Represents a completed invocation.
struct CompletedInvocation {
    duration: Duration,
    error: Option<String>,
    operation: String,
    target: InvocationTarget,
}

impl InFlightInvocation {
    /// Returns a new `InFlightInvocation` with start_time initialized to the current time.
    fn new(operation: &str, target: InvocationTarget) -> Self {
        InFlightInvocation {
            start_time: Instant::now(),
            operation: operation.into(),
            target,
        }
    }

    /// Completes an in-flight invocation.
    fn complete(self, response: &InvocationResponse) -> CompletedInvocation {
        CompletedInvocation {
            duration: self.start_time.elapsed(),
            error: response.error.clone(),
            operation: self.operation,
            target: self.target,
        }
    }
}

/// Represents communication with the AWS X-Ray daemon.
pub struct XRayMiddleware {
    metrics: Arc<Mutex<Metrics>>,
    xray_client: xray::Client,
}

impl XRayMiddleware {
    /// Returns a new `XRayMiddleware` for the specified X-Ray daemon.
    pub fn new(daemon_address: &str) -> anyhow::Result<Self> {
        let xray_client = xray::Client::from_str(daemon_address)?;
        Ok(XRayMiddleware {
            metrics: Arc::new(Mutex::new(Metrics::default())),
            xray_client,
        })
    }

    /// Sends a segment to the X-Ray daemon.
    fn send_to_daemon(&self) {
    }

    /// Starts an invocation timer.
    fn start_timer(&self, inv: &Invocation) -> anyhow::Result<()> {
        let mut metrics = self.metrics.lock().map_err(|e| anyhow!("{}", e))?;
        let state = InFlightInvocation::new(&inv.operation, inv.target.clone());

        if metrics
            .in_flight_invocations
            .insert(inv.id.clone(), state)
            .is_none()
        {
            Ok(())
        } else {
            Err(anyhow!("Invocation already in-flight for {}", inv.id))
        }
    }

    /// Stops an invocation timer.
    /// Returns a `CompletedInvocation` structure.
    fn stop_timer(&self, response: &InvocationResponse) -> anyhow::Result<CompletedInvocation> {
        let mut metrics = self.metrics.lock().map_err(|e| anyhow!("{}", e))?;

        if let Some(state) = metrics
            .in_flight_invocations
            .remove(&response.invocation_id)
        {
            Ok(state.complete(response))
        } else {
            Err(anyhow!(
                "No in-flight invocation for {}",
                response.invocation_id
            ))
        }
    }
}

impl Middleware for XRayMiddleware {
    /// Called by the host before invoking an actor.
    fn actor_pre_invoke(&self, inv: Invocation) -> wascc_host::Result<Invocation> {
        trace!(
            "XRayMiddleware::actor_pre_invoke: {} {} {} {:?}",
            inv.id,
            inv.operation,
            inv.origin,
            inv.target
        );

        self.start_timer(&inv).map_err(|e| to_host_error(e))?;

        Ok(inv)
    }

    /// Called by the host after invoking an actor.
    fn actor_post_invoke(
        &self,
        response: InvocationResponse,
    ) -> wascc_host::Result<InvocationResponse> {
        trace!(
            "XRayMiddleware::actor_post_invoke: {} {:?} ",
            response.invocation_id,
            response.error
        );

        self.stop_timer(&response).map_err(|e| to_host_error(e))?;

        Ok(response)
    }

    /// Called by the host before invoking a capability.
    fn capability_pre_invoke(&self, inv: Invocation) -> wascc_host::Result<Invocation> {
        trace!(
            "XRayMiddleware::capability_pre_invoke: {} {} {} {:?}",
            inv.id,
            inv.operation,
            inv.origin,
            inv.target
        );

        self.start_timer(&inv).map_err(|e| to_host_error(e))?;

        Ok(inv)
    }

    /// Called by the host after invoking a capability.
    fn capability_post_invoke(
        &self,
        response: InvocationResponse,
    ) -> wascc_host::Result<InvocationResponse> {
        trace!(
            "XRayMiddleware::capability_post_invoke: {} {:?} ",
            response.invocation_id,
            response.error
        );

        self.stop_timer(&response).map_err(|e| to_host_error(e))?;

        Ok(response)
    }
}

/// Returns a waSCC host error.
fn to_host_error(source: anyhow::Error) -> wascc_host::errors::Error {
    wascc_host::errors::new(wascc_host::errors::ErrorKind::Middleware(
        source.to_string(),
    ))
}
