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

use log::{debug, trace};
use serde::Serialize;
use serde_json::Value;
use wascc_host::{Invocation, InvocationResponse, InvocationTarget, Middleware};
use xray::{Annotation, Cause, Client, Exception, Segment, Service};

use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, Mutex},
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
    segment: Segment,
}

/// Represents a completed invocation.
struct CompletedInvocation {
    segment: Segment,
}

impl InFlightInvocation {
    /// Returns a new `InFlightInvocation` with start_time initialized to the current time.
    fn new(name: &str, version: &str, inv: &Invocation) -> Self {
        let mut segment = Segment::begin(name);
        segment.service = Some(Service {
            version: Some(version.into()),
        });

        let mut annotations = HashMap::new();
        annotations.insert(
            "operation".into(),
            Annotation::String(inv.operation.clone()),
        );
        annotations.insert("origin".into(), Annotation::String(inv.origin.clone()));
        match &inv.target {
            InvocationTarget::Actor(a) => {
                annotations.insert("target.actor".into(), Annotation::String(a.clone()));
            }
            InvocationTarget::Capability {
                capid: c,
                binding: b,
            } => {
                annotations.insert("target.capid".into(), Annotation::String(c.clone()));
                annotations.insert("target.binding".into(), Annotation::String(b.clone()));
            }
        };
        segment.annotations = Some(annotations);

        let mut metadata = HashMap::new();
        metadata.insert("invocation.id".into(), Value::String(inv.id.clone()));
        segment.metadata = Some(metadata);

        InFlightInvocation { segment }
    }

    /// Completes an in-flight invocation.
    fn complete(self, response: &InvocationResponse) -> CompletedInvocation {
        let mut segment = self.segment;
        segment.end();

        if let Some(error) = &response.error {
            segment.cause = Some(Cause::Description {
                exceptions: vec![Exception {
                    id: "1234567890ABCDEF".into(),
                    messages: Some(error.clone()),
                    remote: None,
                    truncated: None,
                    skipped: None,
                    cause: None,
                    stack: Vec::new(),
                }],
                working_directory: "".into(),
                paths: Vec::new(),
            });
        }

        CompletedInvocation { segment }
    }
}

/// Represents communication with the AWS X-Ray daemon.
pub struct XRayMiddleware {
    client: Client,
    metrics: Arc<Mutex<Metrics>>,
    name: String,
    version: String,
}

impl XRayMiddleware {
    /// Returns a new `XRayMiddleware` for the specified X-Ray daemon.
    pub fn new(daemon_address: &str, name: &str, version: &str) -> anyhow::Result<Self> {
        let client = Client::from_str(daemon_address)?;
        Ok(XRayMiddleware {
            client,
            metrics: Arc::new(Mutex::new(Metrics::default())),
            name: name.into(),
            version: version.into(),
        })
    }

    /// Sends a segment to the X-Ray daemon.
    fn send_to_daemon<S>(&self, data: &S) -> anyhow::Result<()>
    where
        S: Serialize,
    {
        self.client.send(data)?;

        Ok(())
    }

    /// Starts an invocation timer.
    fn start_timer(&self, inv: &Invocation) -> anyhow::Result<()> {
        let mut metrics = self.metrics.lock().map_err(|e| anyhow!("{}", e))?;
        let state = InFlightInvocation::new(&self.name, &self.version, &inv);

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

        self.start_timer(&inv).map_err(to_host_error)?;

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

        let completed_invocation = self.stop_timer(&response).map_err(to_host_error)?;
        debug!("Sending segment to X-Ray daemon");
        self.send_to_daemon(&completed_invocation.segment)
            .map_err(to_host_error)?;

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

        self.start_timer(&inv).map_err(to_host_error)?;

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

        let completed_invocation = self.stop_timer(&response).map_err(to_host_error)?;
        debug!("Sending segment to X-Ray daemon");
        self.send_to_daemon(&completed_invocation.segment)
            .map_err(to_host_error)?;

        Ok(response)
    }
}

/// Returns a waSCC host error.
fn to_host_error(source: anyhow::Error) -> wascc_host::errors::Error {
    wascc_host::errors::new(wascc_host::errors::ErrorKind::Middleware(
        source.to_string(),
    ))
}
