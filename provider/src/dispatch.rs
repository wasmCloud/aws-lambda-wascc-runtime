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
// waSCC AWS Lambda Runtime Providers
//

use aws_lambda_events::event::{alb, apigw};
use serde::{Deserialize, Serialize};
use wascc_codec::{deserialize, serialize};

use std::convert::TryInto;
use std::sync::Arc;

use crate::http::{
    AlbTargetGroupRequestWrapper, AlbTargetGroupResponseWrapper, ApiGatewayProxyRequestWrapper,
    ApiGatewayProxyResponseWrapper, ApiGatewayV2ProxyRequestWrapper,
    ApiGatewayV2ProxyResponseWrapper,
};
use crate::HostDispatcher;

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
            let host_dispatcher = self.host_dispatcher();
            let lock = host_dispatcher.read().unwrap();
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

    /// Returns a shared host dispatcher.
    fn host_dispatcher(&self) -> HostDispatcher;

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
    host_dispatcher: HostDispatcher,
}

impl HttpDispatcher {
    /// Returns a new `HttpDispatcher`.
    pub fn new(host_dispatcher: HostDispatcher) -> Self {
        Self { host_dispatcher }
    }

    /// Dispatches an ALB target group request.
    fn dispatch_alb_request(
        &self,
        actor: &str,
        request: AlbTargetGroupRequestWrapper,
    ) -> anyhow::Result<AlbTargetGroupResponseWrapper> {
        info!("HttpDispatcher dispatch ALB target group request");
        Ok(self
            .dispatch_request(actor, request.try_into()?)?
            .try_into()?)
    }

    /// Dispatches an API Gateway proxy request.
    fn dispatch_apigw_request(
        &self,
        actor: &str,
        request: ApiGatewayProxyRequestWrapper,
    ) -> anyhow::Result<ApiGatewayProxyResponseWrapper> {
        info!("HttpDispatcher dispatch API Gateway proxy request");
        Ok(self
            .dispatch_request(actor, request.try_into()?)?
            .try_into()?)
    }

    /// Dispatches an API Gateway v2 proxy request.
    fn dispatch_apigwv2_request(
        &self,
        actor: &str,
        request: ApiGatewayV2ProxyRequestWrapper,
    ) -> anyhow::Result<ApiGatewayV2ProxyResponseWrapper> {
        info!("HttpDispatcher dispatch API Gateway v2 proxy request");
        Ok(self
            .dispatch_request(actor, request.try_into()?)?
            .try_into()?)
    }
}

impl Dispatcher<'_> for HttpDispatcher {
    /// The request type.
    type T = wascc_codec::http::Request;
    /// The response type.
    type U = wascc_codec::http::Response;

    /// The operation this dispatcher dispatches.
    const OP: &'static str = wascc_codec::http::OP_HANDLE_REQUEST;

    /// Returns a shared host dispatcher.
    fn host_dispatcher(&self) -> HostDispatcher {
        Arc::clone(&self.host_dispatcher)
    }

    /// Attempts to dispatch a Lambda invocation event, returning an invocation response.
    /// The bodies of the invocation event and response are passed and returned.
    fn dispatch_invocation_event(&self, actor: &str, body: &[u8]) -> anyhow::Result<Vec<u8>> {
        let body = std::str::from_utf8(body).map_err(|e| {
            debug!("{}", e);
            NotHttpRequestError {}
        })?;

        debug!("Lambda invocation event body:\n{}", body);

        match serde_json::from_str(body) {
            Ok(request @ alb::AlbTargetGroupRequest { .. }) => {
                let response: alb::AlbTargetGroupResponse =
                    self.dispatch_alb_request(actor, request.into())?.into();
                return serde_json::to_vec(&response).map_err(|e| e.into());
            }
            _ => debug!("Not an ALB request"),
        };
        match serde_json::from_str(body) {
            Ok(request @ apigw::ApiGatewayProxyRequest { .. }) => {
                let response: apigw::ApiGatewayProxyResponse =
                    self.dispatch_apigw_request(actor, request.into())?.into();
                return serde_json::to_vec(&response).map_err(|e| e.into());
            }
            _ => debug!("Not an API Gateway proxy request"),
        };
        match serde_json::from_str(body) {
            Ok(request @ apigw::ApiGatewayV2httpRequest { .. }) => {
                let response: apigw::ApiGatewayV2httpResponse =
                    self.dispatch_apigwv2_request(actor, request.into())?.into();
                return serde_json::to_vec(&response).map_err(|e| e.into());
            }
            _ => debug!("Not an API Gateway v2 proxy request"),
        };

        Err(NotHttpRequestError {}.into())
    }
}

/// Dispatches Lambda raw events.
pub(crate) struct RawEventDispatcher {
    host_dispatcher: HostDispatcher,
}

impl RawEventDispatcher {
    /// Returns a new `RawEventDispatcher`.
    pub fn new(host_dispatcher: HostDispatcher) -> Self {
        Self { host_dispatcher }
    }
}

impl Dispatcher<'_> for RawEventDispatcher {
    /// The request type.
    type T = codec::Event;
    /// The response type.
    type U = codec::Response;

    /// The operation this dispatcher dispatches.
    const OP: &'static str = codec::OP_HANDLE_EVENT;

    /// Returns a shared host dispatcher.
    fn host_dispatcher(&self) -> HostDispatcher {
        Arc::clone(&self.host_dispatcher)
    }

    /// Attempts to dispatch a Lambda invocation event, returning an invocation response.
    /// The bodies of the invocation event and response are passed and returned.
    fn dispatch_invocation_event(&self, actor: &str, body: &[u8]) -> anyhow::Result<Vec<u8>> {
        let raw_event = codec::Event {
            body: body.to_vec(),
        };

        Ok(self.dispatch_request(actor, raw_event)?.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_common::*;
    use std::sync::RwLock;

    /// Tests successfully dispatching a raw event.
    #[test]
    fn dispatch_raw_event_ok() {
        let response = codec::Response {
            body: RESPONSE_BODY.to_vec(),
        };
        let host_dispatcher: HostDispatcher =
            Arc::new(RwLock::new(Box::new(MockHostDispatcher::new(response))));
        let dispatcher = RawEventDispatcher::new(host_dispatcher);

        let result = dispatcher.dispatch_invocation_event(MODULE_ID, EVENT_BODY);
        assert!(result.is_ok());
        assert_eq!(RESPONSE_BODY, result.unwrap().as_slice());
    }

    /// Tests failing to dispatch an event.
    #[test]
    fn dispatch_raw_event_not_dispatched_error() {
        let host_dispatcher: HostDispatcher =
            Arc::new(RwLock::new(Box::new(ErrorHostDispatcher::new())));
        let dispatcher = RawEventDispatcher::new(host_dispatcher);

        let result = dispatcher.dispatch_invocation_event(MODULE_ID, EVENT_BODY);
        assert!(result.is_err());

        let e = result.unwrap_err();
        assert!(e.is::<DispatcherError>());
        match e.downcast_ref::<DispatcherError>().unwrap() {
            DispatcherError::NotDispatched { .. } => assert!(true),
            _ => assert!(false),
        }
    }

    /// Tests failing to deserialize an event response.
    #[test]
    fn dispatch_raw_event_response_deserialization_error() {
        let host_dispatcher: HostDispatcher = Arc::new(RwLock::new(Box::new(
            MockHostDispatcher::new(RESPONSE_BODY),
        )));
        let dispatcher = RawEventDispatcher::new(host_dispatcher);

        let result = dispatcher.dispatch_invocation_event(MODULE_ID, EVENT_BODY);
        assert!(result.is_err());

        let e = result.unwrap_err();
        assert!(e.is::<DispatcherError>());
        match e.downcast_ref::<DispatcherError>().unwrap() {
            DispatcherError::ResponseDeserialization { .. } => assert!(true),
            _ => assert!(false),
        }
    }

    /// Tests successfully dispatching an ALB target group request.
    #[test]
    fn dispatch_alb_target_group_request_ok() {
        let response = valid_http_response();
        let host_dispatcher: HostDispatcher =
            Arc::new(RwLock::new(Box::new(MockHostDispatcher::new(response))));
        let dispatcher = HttpDispatcher::new(host_dispatcher);

        let result =
            dispatcher.dispatch_alb_request(MODULE_ID, valid_alb_target_group_request().into());
        assert!(result.is_ok());
    }

    /// Tests failing to dispatch an ALB target group request.
    #[test]
    fn dispatch_alb_target_group_request_not_dispatched_error() {
        let host_dispatcher: HostDispatcher =
            Arc::new(RwLock::new(Box::new(ErrorHostDispatcher::new())));
        let dispatcher = HttpDispatcher::new(host_dispatcher);

        let result =
            dispatcher.dispatch_alb_request(MODULE_ID, valid_alb_target_group_request().into());
        assert!(result.is_err());

        let e = result.unwrap_err();
        assert!(e.is::<DispatcherError>());
        match e.downcast_ref::<DispatcherError>().unwrap() {
            DispatcherError::NotDispatched { .. } => assert!(true),
            _ => assert!(false),
        }
    }

    /// Tests failing to deserialize an ALB target group request.
    #[test]
    fn dispatch_alb_target_group_deserialization_error() {
        let host_dispatcher: HostDispatcher = Arc::new(RwLock::new(Box::new(
            MockHostDispatcher::new(RESPONSE_BODY),
        )));
        let dispatcher = HttpDispatcher::new(host_dispatcher);

        let result =
            dispatcher.dispatch_alb_request(MODULE_ID, valid_alb_target_group_request().into());
        assert!(result.is_err());

        let e = result.unwrap_err();
        assert!(e.is::<DispatcherError>());
        match e.downcast_ref::<DispatcherError>().unwrap() {
            DispatcherError::ResponseDeserialization { .. } => assert!(true),
            _ => assert!(false),
        }
    }

    /// Tests successfully dispatching an API Gateway proxy request.
    #[test]
    fn dispatch_api_gateway_proxy_request_ok() {
        let response = valid_http_response();
        let host_dispatcher: HostDispatcher =
            Arc::new(RwLock::new(Box::new(MockHostDispatcher::new(response))));
        let dispatcher = HttpDispatcher::new(host_dispatcher);

        let result =
            dispatcher.dispatch_apigw_request(MODULE_ID, valid_api_gateway_proxy_request().into());
        assert!(result.is_ok());
    }

    /// Tests failing to dispatch an API Gateway proxy request.
    #[test]
    fn dispatch_api_gateway_proxy_request_not_dispatched_error() {
        let host_dispatcher: HostDispatcher =
            Arc::new(RwLock::new(Box::new(ErrorHostDispatcher::new())));
        let dispatcher = HttpDispatcher::new(host_dispatcher);

        let result =
            dispatcher.dispatch_apigw_request(MODULE_ID, valid_api_gateway_proxy_request().into());
        assert!(result.is_err());

        let e = result.unwrap_err();
        assert!(e.is::<DispatcherError>());
        match e.downcast_ref::<DispatcherError>().unwrap() {
            DispatcherError::NotDispatched { .. } => assert!(true),
            _ => assert!(false),
        }
    }

    /// Tests failing to deserialize an API Gateway proxy request.
    #[test]
    fn dispatch_api_gateway_proxy_request_deserialization_error() {
        let host_dispatcher: HostDispatcher = Arc::new(RwLock::new(Box::new(
            MockHostDispatcher::new(RESPONSE_BODY),
        )));
        let dispatcher = HttpDispatcher::new(host_dispatcher);

        let result =
            dispatcher.dispatch_apigw_request(MODULE_ID, valid_api_gateway_proxy_request().into());
        assert!(result.is_err());

        let e = result.unwrap_err();
        assert!(e.is::<DispatcherError>());
        match e.downcast_ref::<DispatcherError>().unwrap() {
            DispatcherError::ResponseDeserialization { .. } => assert!(true),
            _ => assert!(false),
        }
    }

    /// Tests successfully dispatching an API Gateway v2 proxy request.
    #[test]
    fn dispatch_api_gatewayv2_proxy_request_ok() {
        let response = valid_http_response();
        let host_dispatcher: HostDispatcher =
            Arc::new(RwLock::new(Box::new(MockHostDispatcher::new(response))));
        let dispatcher = HttpDispatcher::new(host_dispatcher);

        let result = dispatcher
            .dispatch_apigwv2_request(MODULE_ID, valid_api_gatewayv2_proxy_request().into());
        assert!(result.is_ok());
    }

    /// Tests failing to dispatch an API Gateway v2 proxy request.
    #[test]
    fn dispatch_api_gatewayv2_proxy_request_not_dispatched_error() {
        let host_dispatcher: HostDispatcher =
            Arc::new(RwLock::new(Box::new(ErrorHostDispatcher::new())));
        let dispatcher = HttpDispatcher::new(host_dispatcher);

        let result = dispatcher
            .dispatch_apigwv2_request(MODULE_ID, valid_api_gatewayv2_proxy_request().into());
        assert!(result.is_err());

        let e = result.unwrap_err();
        assert!(e.is::<DispatcherError>());
        match e.downcast_ref::<DispatcherError>().unwrap() {
            DispatcherError::NotDispatched { .. } => assert!(true),
            _ => assert!(false),
        }
    }

    /// Tests failing to deserialize an API Gateway v2 proxy request.
    #[test]
    fn dispatch_api_gatewayv2_proxy_request_deserialization_error() {
        let host_dispatcher: HostDispatcher = Arc::new(RwLock::new(Box::new(
            MockHostDispatcher::new(RESPONSE_BODY),
        )));
        let dispatcher = HttpDispatcher::new(host_dispatcher);

        let result = dispatcher
            .dispatch_apigwv2_request(MODULE_ID, valid_api_gatewayv2_proxy_request().into());
        assert!(result.is_err());

        let e = result.unwrap_err();
        assert!(e.is::<DispatcherError>());
        match e.downcast_ref::<DispatcherError>().unwrap() {
            DispatcherError::ResponseDeserialization { .. } => assert!(true),
            _ => assert!(false),
        }
    }

    /// Tests successfully dispatching an ALB target group request encoded as JSON.
    #[test]
    fn dispatch_alb_target_group_request_json_ok() {
        let response = valid_http_response();
        let host_dispatcher: HostDispatcher =
            Arc::new(RwLock::new(Box::new(MockHostDispatcher::new(response))));
        let dispatcher = HttpDispatcher::new(host_dispatcher);

        let result = serde_json::to_vec(&valid_alb_target_group_request());
        assert!(result.is_ok());
        let body = result.unwrap();

        let result = dispatcher.dispatch_invocation_event(MODULE_ID, &body);
        assert!(result.is_ok());
    }

    /// Tests successfully dispatching an API Gateway proxy request encoded as JSON.
    #[test]
    fn dispatch_api_gateway_proxy_request_json_ok() {
        let response = valid_http_response();
        let host_dispatcher: HostDispatcher =
            Arc::new(RwLock::new(Box::new(MockHostDispatcher::new(response))));
        let dispatcher = HttpDispatcher::new(host_dispatcher);

        let result = serde_json::to_vec(&valid_api_gateway_proxy_request());
        assert!(result.is_ok());
        let body = result.unwrap();

        let result = dispatcher.dispatch_invocation_event(MODULE_ID, &body);
        assert!(result.is_ok());
    }

    /// Tests successfully dispatching an API Gateway v2 proxy request encoded as JSON.
    #[test]
    fn dispatch_api_gatewayv2_proxy_request_json_ok() {
        let response = valid_http_response();
        let host_dispatcher: HostDispatcher =
            Arc::new(RwLock::new(Box::new(MockHostDispatcher::new(response))));
        let dispatcher = HttpDispatcher::new(host_dispatcher);

        let result = serde_json::to_vec(&valid_api_gatewayv2_proxy_request());
        assert!(result.is_ok());
        let body = result.unwrap();

        let result = dispatcher.dispatch_invocation_event(MODULE_ID, &body);
        assert!(result.is_ok());
    }

    /// Tests failing to dispatch a raw event as an HTTP-like request.
    #[test]
    fn dispatch_raw_event_json_not_http_error() {
        let response = valid_http_response();
        let host_dispatcher: HostDispatcher =
            Arc::new(RwLock::new(Box::new(MockHostDispatcher::new(response))));
        let dispatcher = HttpDispatcher::new(host_dispatcher);

        let result = serde_json::to_vec(EVENT_BODY);
        assert!(result.is_ok());
        let body = result.unwrap();

        let result = dispatcher.dispatch_invocation_event(MODULE_ID, &body);
        assert!(result.is_err());

        let e = result.unwrap_err();
        assert!(e.is::<NotHttpRequestError>());
    }
}
