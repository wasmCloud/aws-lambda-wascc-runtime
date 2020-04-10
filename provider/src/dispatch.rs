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

/// Represents dispatching a request to an actor and returning its response.
pub trait Dispatcher<'de> {
    /// The request type.
    type T: Serialize;

    /// The response type.
    type U: Deserialize<'de>;

    /// The operation this dispatcher dispatches.
    const OP: &'static str;

    /// Dispatches a request to the specified actor using the specified dispatcher.
    fn dispatch(&self, actor: &str, request: Self::T) -> anyhow::Result<Self::U> {
        let msg = match serialize(request) {
            Ok(msg) => msg,
            Err(e) => return Err(anyhow!("Failed to serialize actor's request: {}", e)),
        };

        let handler_resp = {
            let dispatcher = self.dispatcher();
            let lock = dispatcher.read().unwrap();
            lock.dispatch(actor, Self::OP, &msg)
        };
        let resp = handler_resp
            .map_err(|e| anyhow!("Guest {} failed to handle {}: {}", actor, Self::OP, e))?;

        deserialize::<Self::U>(resp.as_slice())
            .map_err(|e| anyhow!("Failed to deserialize actor's response: {}", e))
    }

    /// Returns a dispatcher.
    fn dispatcher(&self) -> Arc<RwLock<Box<dyn wascc_codec::capabilities::Dispatcher>>>;

    /// Attempts to dispatch a Lambda invocation event, returning an invocation response.
    fn try_dispatch(&self, actor: &str, body: &[u8]) -> anyhow::Result<Vec<u8>>;
}

/// Dispatches HTTP requests.
pub struct HttpDispatcher {
    dispatcher: Arc<RwLock<Box<dyn wascc_codec::capabilities::Dispatcher>>>,
}

impl HttpDispatcher {
    fn new(dispatcher: Arc<RwLock<Box<dyn wascc_codec::capabilities::Dispatcher>>>) -> Self {
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

    fn try_dispatch(&self, actor: &str, body: &[u8]) -> anyhow::Result<Vec<u8>> {
        Ok(vec![])
    }
}

/// Dispatches Lambda raw events.
pub struct RawEventDispatcher {
    dispatcher: Arc<RwLock<Box<dyn wascc_codec::capabilities::Dispatcher>>>,
}

impl RawEventDispatcher {
    fn new(dispatcher: Arc<RwLock<Box<dyn wascc_codec::capabilities::Dispatcher>>>) -> Self {
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

    fn try_dispatch(&self, actor: &str, body: &[u8]) -> anyhow::Result<Vec<u8>> {
        let event = codec::Event {
            body: body.to_vec(),
        };

        Ok(self.dispatch(actor, event)?.body)
    }
}
