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
// waSCC AWS Lambda Runtime Provider
//

use serde::{Deserialize, Serialize};
use wascc_codec::{deserialize, serialize};

use std::sync::{Arc, RwLock};

/// A dispatcher error.
#[derive(thiserror::Error, Debug)]
pub(crate) enum DispatcherError {
    /// Request was not dispatched.
    #[error("Guest {} failed to handle {}: {}", actor, op, source)]
    NotDispatched {
        actor: String,
        op: String,
        #[source]
        source: anyhow::Error,
    },

    /// Request serialization error.
    #[error("Failed to serialize actor's request: {}", source)]
    RequestSerialization {
        #[source]
        source: anyhow::Error,
    },

    /// Response deserialization error.
    #[error("Failed to deserialize actor's response: {}", source)]
    ResponseDeserialization {
        #[source]
        source: anyhow::Error,
    },
}

/// Represents dispatching a request to an actor and returning its response.
pub(crate) trait Dispatcher<'de> {
    /// The request type.
    type T: Serialize;

    /// The response type.
    type U: Deserialize<'de>;

    /// The operation this dispatcher dispatches.
    const OP: &'static str;

    /// Dispatches a request to the specified actor using our dispatcher.
    fn dispatch_request(&self, actor: &str, request: Self::T) -> anyhow::Result<Self::U> {
        let input = serialize(request).map_err(|e| DispatcherError::RequestSerialization {
            source: anyhow!("{}", e),
        })?;

        let handler_resp = {
            let dispatcher = self.dispatcher();
            let lock = dispatcher.read().unwrap();
            lock.dispatch(actor, Self::OP, &input)
        };
        let output = handler_resp.map_err(|e| DispatcherError::NotDispatched {
            actor: actor.into(),
            op: Self::OP.into(),
            source: anyhow!("{}", e),
        })?;

        let response = deserialize::<Self::U>(output.as_slice()).map_err(|e| {
            DispatcherError::ResponseDeserialization {
                source: anyhow!("{}", e),
            }
        })?;

        Ok(response)
    }

    /// Returns a dispatcher.
    fn dispatcher(&self) -> Arc<RwLock<Box<dyn wascc_codec::capabilities::Dispatcher>>>;

    /// Attempts to dispatch a Lambda invocation event, returning an invocation response.
    /// The bodies of the invocation event and response are passed and returned.
    fn dispatch_invocation_event(&self, actor: &str, event: &[u8]) -> anyhow::Result<Vec<u8>>;
}

/// The invocation request is not an HTTP request.
#[derive(thiserror::Error, Debug)]
#[error("Not an HTTP request")]
pub(crate) struct NotHttpRequestError;

/// Dispatches HTTP requests.
pub(crate) struct HttpDispatcher {
    dispatcher: Arc<RwLock<Box<dyn wascc_codec::capabilities::Dispatcher>>>,
}

impl HttpDispatcher {
    pub fn new(dispatcher: Arc<RwLock<Box<dyn wascc_codec::capabilities::Dispatcher>>>) -> Self {
        HttpDispatcher { dispatcher }
    }
}

impl Dispatcher<'_> for HttpDispatcher {
    type T = wascc_codec::http::Request;
    type U = wascc_codec::http::Response;

    const OP: &'static str = wascc_codec::http::OP_HANDLE_REQUEST;

    fn dispatcher(&self) -> Arc<RwLock<Box<dyn wascc_codec::capabilities::Dispatcher>>> {
        Arc::clone(&self.dispatcher)
    }

    fn dispatch_invocation_event(&self, actor: &str, body: &[u8]) -> anyhow::Result<Vec<u8>> {
        // match serde_json::from_slice(body) {
        //     Ok(request) => {
        //         let response = self.dispatch_alb_http_request(request, request_id)?;
        //         return serde_json::to_vec(&response)
        //             .map_err(|e| anyhow!("Failed to serialize ALB response: {}", e));
        //     }
        //     _ => {}
        // };
        Err(NotHttpRequestError {}.into())
    }
}

/// Dispatches Lambda raw events.
pub(crate) struct RawEventDispatcher {
    dispatcher: Arc<RwLock<Box<dyn wascc_codec::capabilities::Dispatcher>>>,
}

impl RawEventDispatcher {
    pub fn new(dispatcher: Arc<RwLock<Box<dyn wascc_codec::capabilities::Dispatcher>>>) -> Self {
        RawEventDispatcher { dispatcher }
    }
}

impl Dispatcher<'_> for RawEventDispatcher {
    type T = codec::Event;
    type U = codec::Response;

    const OP: &'static str = codec::OP_HANDLE_EVENT;

    fn dispatcher(&self) -> Arc<RwLock<Box<dyn wascc_codec::capabilities::Dispatcher>>> {
        Arc::clone(&self.dispatcher)
    }

    fn dispatch_invocation_event(&self, actor: &str, body: &[u8]) -> anyhow::Result<Vec<u8>> {
        let raw_event = codec::Event {
            body: body.to_vec(),
        };

        Ok(self.dispatch_request(actor, raw_event)?.body)
    }
}
