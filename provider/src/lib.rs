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

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

pub use crate::lambda::{initerr_reporter, InitializationErrorReporter};
pub use crate::provider::{default_http_request_provider, default_raw_event_provider};

mod dispatch;
mod http;
mod lambda;
mod provider;

/// Represents a shared host dispatcher.
pub(crate) type HostDispatcher =
    std::sync::Arc<std::sync::RwLock<Box<dyn wascc_codec::capabilities::Dispatcher>>>;

/// This module contains code to be used by many unit tests.
#[cfg(test)]
mod tests_common {
    use aws_lambda_events::event::{alb, apigw};
    use serde::Serialize;
    use wascc_codec::serialize;

    use std::any::Any;
    use std::collections::HashMap;
    use std::error::Error;
    use std::sync::{Arc, RwLock};

    use super::HostDispatcher;

    pub(crate) const ERROR_MESSAGE: &str = "ERROR";
    pub(crate) const EVENT_BODY: &'static [u8] = b"EVENT_BODY";
    pub(crate) const MODULE_ID: &str = "MODULE_ID";
    pub(crate) const REQUEST_ID: &str = "REQUEST_ID";
    pub(crate) const RESPONSE_BODY: &'static [u8] = b"RESPONSE_BODY";
    pub(crate) const TRACE_ID: &str = "TRACE_ID";

    /// Represents a mock `wascc_codec::capabilities::Dispatcher`.
    pub(crate) struct MockWasccDispatcher<T> {
        /// The dispatcher response.
        response: T,
        /// The last dispatch's parameters.
        pub actor: RwLock<Option<String>>,
        pub op: RwLock<Option<String>>,
        pub msg: RwLock<Option<Vec<u8>>>,
    }

    impl<T> MockWasccDispatcher<T> {
        /// Returns a new `MockWasccDispatcher`.
        pub fn new(response: T) -> Self {
            Self {
                response,
                actor: RwLock::new(None),
                op: RwLock::new(None),
                msg: RwLock::new(None),
            }
        }
    }

    impl<T: Any + Serialize + Send + Sync> wascc_codec::capabilities::Dispatcher
        for MockWasccDispatcher<T>
    {
        fn dispatch(&self, actor: &str, op: &str, msg: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
            // Record the parameters.
            let mut lock = self.actor.write().unwrap();
            *lock = Some(actor.into());
            let mut lock = self.op.write().unwrap();
            *lock = Some(op.into());
            let mut lock = self.msg.write().unwrap();
            *lock = Some(msg.into());

            Ok(serialize(&self.response)?)
        }
    }

    /// Returns a boxed mock `wascc_codec::capabilities::Dispatcher`.
    pub(crate) fn boxed_mock_dispatcher<T: Any + Serialize + Send + Sync>(
        response: T,
    ) -> Box<MockWasccDispatcher<T>> {
        Box::new(MockWasccDispatcher::new(response))
    }

    /// Returns a mock `HostDispatcher`.
    pub(crate) fn mock_host_dispatcher<T: Any + Serialize + Send + Sync>(
        response: T,
    ) -> HostDispatcher {
        Arc::new(RwLock::new(boxed_mock_dispatcher(response)))
    }

    /// Represents a `wascc_codec::capabilities::Dispatcher` that returns an error.
    pub(crate) struct ErrorWasccDispatcher {}

    impl ErrorWasccDispatcher {
        /// Returns a new `ErrorWasccDispatcher`.
        pub fn new() -> Self {
            Self {}
        }
    }

    impl wascc_codec::capabilities::Dispatcher for ErrorWasccDispatcher {
        fn dispatch(
            &self,
            _actor: &str,
            _op: &str,
            _msg: &[u8],
        ) -> Result<Vec<u8>, Box<dyn Error>> {
            Err(anyhow!(ERROR_MESSAGE).into())
        }
    }

    /// Returns a boxed `wascc_codec::capabilities::Dispatcher` that returns an error.
    pub(crate) fn boxed_error_dispatcher() -> Box<dyn wascc_codec::capabilities::Dispatcher> {
        Box::new(ErrorWasccDispatcher::new())
    }

    /// Returns a `HostDispatcher` that returns an error.
    pub(crate) fn error_host_dispatcher() -> HostDispatcher {
        Arc::new(RwLock::new(boxed_error_dispatcher()))
    }

    /// Returns a query string map for a request.
    fn request_query_string() -> HashMap<String, String> {
        let mut qs = HashMap::new();
        qs.insert("key1".into(), "value1".into());
        qs
    }

    /// Returns an HTTP headers map for a request.
    fn request_headers() -> HashMap<String, String> {
        let mut hdrs = HashMap::new();
        hdrs.insert("accept".into(), "application/json".into());
        hdrs
    }

    /// Returns a valid `AlbTargetGroupRequest`.
    pub(crate) fn valid_alb_target_group_request() -> alb::AlbTargetGroupRequest {
        alb::AlbTargetGroupRequest {
            http_method: Some("GET".into()),
            path: Some("/".into()),
            query_string_parameters: request_query_string(),
            multi_value_query_string_parameters: HashMap::new(),
            headers: request_headers(),
            multi_value_headers: HashMap::new(),
            request_context: alb::AlbTargetGroupRequestContext {
                elb: alb::ElbContext {
                    target_group_arn: None,
                },
            },
            is_base64_encoded: false,
            body: Some("Hello world".into()),
        }
    }

    /// Returns a valid `http::Response`.
    pub(crate) fn valid_http_response() -> wascc_codec::http::Response {
        let mut hdrs = HashMap::new();
        hdrs.insert("server".into(), "test".into());
        wascc_codec::http::Response {
            status_code: 200,
            status: "OK".into(),
            header: hdrs,
            body: vec![],
        }
    }

    /// Returns a valid `ApiGatewayProxyRequest`.
    pub(crate) fn valid_api_gateway_proxy_request() -> apigw::ApiGatewayProxyRequest {
        apigw::ApiGatewayProxyRequest {
            resource: None,
            path: Some("/".into()),
            http_method: Some("GET".into()),
            headers: request_headers(),
            multi_value_headers: HashMap::new(),
            query_string_parameters: request_query_string(),
            multi_value_query_string_parameters: HashMap::new(),
            path_parameters: HashMap::new(),
            stage_variables: HashMap::new(),
            request_context: apigw::ApiGatewayProxyRequestContext {
                account_id: None,
                resource_id: None,
                operation_name: None,
                stage: None,
                request_id: None,
                identity: apigw::ApiGatewayRequestIdentity {
                    cognito_identity_pool_id: None,
                    account_id: None,
                    cognito_identity_id: None,
                    caller: None,
                    api_key: None,
                    api_key_id: None,
                    access_key: None,
                    source_ip: None,
                    cognito_authentication_type: None,
                    cognito_authentication_provider: None,
                    user_arn: None,
                    user_agent: None,
                    user: None,
                },
                resource_path: None,
                authorizer: HashMap::new(),
                http_method: None,
                apiid: None,
            },
            body: Some("Hello world".into()),
            is_base64_encoded: Some(false),
        }
    }

    /// Returns a valid `ApiGatewayV2httpRequest`.
    pub(crate) fn valid_api_gatewayv2_proxy_request() -> apigw::ApiGatewayV2httpRequest {
        apigw::ApiGatewayV2httpRequest {
            version: Some("2.0".into()),
            route_key: None,
            raw_path: None,
            raw_query_string: None,
            cookies: None,
            headers: request_headers(),
            query_string_parameters: request_query_string(),
            path_parameters: HashMap::new(),
            request_context: apigw::ApiGatewayV2httpRequestContext {
                route_key: None,
                account_id: None,
                stage: None,
                request_id: None,
                authorizer: None,
                apiid: None,
                domain_name: None,
                domain_prefix: None,
                time: None,
                time_epoch: 0,
                http: apigw::ApiGatewayV2httpRequestContextHttpDescription {
                    method: Some("GET".into()),
                    path: Some("/".into()),
                    protocol: None,
                    source_ip: None,
                    user_agent: None,
                },
            },
            stage_variables: HashMap::new(),
            body: Some("Hello world".into()),
            is_base64_encoded: false,
        }
    }
}
