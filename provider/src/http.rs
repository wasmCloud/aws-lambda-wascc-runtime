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

use aws_lambda_events::event::{alb, apigw};

use std::collections::HashMap;
use std::convert::TryFrom;

#[derive(Debug)]
pub(crate) struct AlbTargetGroupRequestWrapper(alb::AlbTargetGroupRequest);

impl From<alb::AlbTargetGroupRequest> for AlbTargetGroupRequestWrapper {
    /// Converts an ALB target group request to an instance of the wrapper type.
    fn from(request: alb::AlbTargetGroupRequest) -> Self {
        AlbTargetGroupRequestWrapper(request)
    }
}

impl TryFrom<AlbTargetGroupRequestWrapper> for wascc_codec::http::Request {
    type Error = anyhow::Error;

    /// Attempts conversion of an ALB target group request to an actor's HTTP request.
    fn try_from(request: AlbTargetGroupRequestWrapper) -> anyhow::Result<Self> {
        let query_string = query_string(request.0.query_string_parameters);

        Ok(wascc_codec::http::Request {
            method: request
                .0
                .http_method
                .ok_or_else(|| anyhow!("Missing method in ALB target group request"))?,
            path: request
                .0
                .path
                .ok_or_else(|| anyhow!("Missing path in ALB target group request"))?,
            query_string,
            header: request.0.headers,
            body: match request.0.body {
                Some(s) if request.0.is_base64_encoded => base64::decode(s)?,
                Some(s) => s.into_bytes(),
                None => vec![],
            },
        })
    }
}

#[derive(Debug)]
pub(crate) struct AlbTargetGroupResponseWrapper(alb::AlbTargetGroupResponse);

impl From<AlbTargetGroupResponseWrapper> for alb::AlbTargetGroupResponse {
    /// Converts instance of the wrapper type to an ALB response.
    fn from(response: AlbTargetGroupResponseWrapper) -> Self {
        response.0
    }
}

impl From<alb::AlbTargetGroupResponse> for AlbTargetGroupResponseWrapper {
    /// Converts instance of an ALB response to the wrapper type.
    fn from(response: alb::AlbTargetGroupResponse) -> Self {
        AlbTargetGroupResponseWrapper(response)
    }
}

impl TryFrom<wascc_codec::http::Response> for AlbTargetGroupResponseWrapper {
    type Error = anyhow::Error;

    /// Attempts conversion of an actor's HTTP response to an ALB response.
    fn try_from(response: wascc_codec::http::Response) -> anyhow::Result<Self> {
        let (body, is_base64_encoded) = body_string(response.body);

        Ok(alb::AlbTargetGroupResponse {
            status_code: response.status_code as i64,
            status_description: Some(response.status),
            headers: response.header,
            multi_value_headers: HashMap::new(),
            body,
            is_base64_encoded,
        }
        .into())
    }
}

#[derive(Debug)]
pub(crate) struct ApiGatewayProxyRequestWrapper(apigw::ApiGatewayProxyRequest);

impl From<apigw::ApiGatewayProxyRequest> for ApiGatewayProxyRequestWrapper {
    /// Converts an API Gateway proxy request to an instance of the wrapper type.
    fn from(request: apigw::ApiGatewayProxyRequest) -> Self {
        ApiGatewayProxyRequestWrapper(request)
    }
}

impl TryFrom<ApiGatewayProxyRequestWrapper> for wascc_codec::http::Request {
    type Error = anyhow::Error;

    /// Attempts conversion of an API Gateway proxy request to an actor's HTTP request.
    fn try_from(request: ApiGatewayProxyRequestWrapper) -> anyhow::Result<Self> {
        let query_string = query_string(request.0.query_string_parameters);

        Ok(wascc_codec::http::Request {
            method: request
                .0
                .http_method
                .ok_or_else(|| anyhow!("Missing method in API Gateway proxy request"))?,
            path: request
                .0
                .path
                .ok_or_else(|| anyhow!("Missing path in API Gateway proxy request"))?,
            query_string,
            header: request.0.headers,
            body: match request.0.body {
                Some(s) if request.0.is_base64_encoded.unwrap_or(false) => base64::decode(s)?,
                Some(s) => s.into_bytes(),
                None => vec![],
            },
        })
    }
}

#[derive(Debug)]
pub(crate) struct ApiGatewayProxyResponseWrapper(apigw::ApiGatewayProxyResponse);

impl From<ApiGatewayProxyResponseWrapper> for apigw::ApiGatewayProxyResponse {
    /// Converts instance of the wrapper type to an API Gateway proxy response.
    fn from(response: ApiGatewayProxyResponseWrapper) -> Self {
        response.0
    }
}

impl From<apigw::ApiGatewayProxyResponse> for ApiGatewayProxyResponseWrapper {
    /// Converts instance of an API Gateway proxy response to the wrapper type.
    fn from(response: apigw::ApiGatewayProxyResponse) -> Self {
        ApiGatewayProxyResponseWrapper(response)
    }
}

impl TryFrom<wascc_codec::http::Response> for ApiGatewayProxyResponseWrapper {
    type Error = anyhow::Error;

    /// Attempts conversion of an actor's HTTP response to an API Gateway proxy response.
    fn try_from(response: wascc_codec::http::Response) -> anyhow::Result<Self> {
        let (body, is_base64_encoded) = body_string(response.body);

        Ok(apigw::ApiGatewayProxyResponse {
            status_code: response.status_code as i64,
            headers: response.header,
            multi_value_headers: HashMap::new(),
            body,
            is_base64_encoded: Some(is_base64_encoded),
        }
        .into())
    }
}

#[derive(Debug)]
pub(crate) struct ApiGatewayV2ProxyRequestWrapper(apigw::ApiGatewayV2httpRequest);

impl From<apigw::ApiGatewayV2httpRequest> for ApiGatewayV2ProxyRequestWrapper {
    /// Converts an API Gateway v2 proxy request to an instance of the wrapper type.
    fn from(request: apigw::ApiGatewayV2httpRequest) -> Self {
        ApiGatewayV2ProxyRequestWrapper(request)
    }
}

impl TryFrom<ApiGatewayV2ProxyRequestWrapper> for wascc_codec::http::Request {
    type Error = anyhow::Error;

    /// Attempts conversion of an API Gateway v2 proxy request to an actor's HTTP request.
    fn try_from(request: ApiGatewayV2ProxyRequestWrapper) -> anyhow::Result<Self> {
        let query_string = query_string(request.0.query_string_parameters);

        Ok(wascc_codec::http::Request {
            method: request
                .0
                .request_context
                .http
                .method
                .ok_or_else(|| anyhow!("Missing method in API Gateway v2 proxy request"))?,
            path: request
                .0
                .request_context
                .http
                .path
                .ok_or_else(|| anyhow!("Missing path in API Gateway v2 proxy request"))?,
            query_string,
            header: request.0.headers,
            body: match request.0.body {
                Some(s) if request.0.is_base64_encoded => base64::decode(s)?,
                Some(s) => s.into_bytes(),
                None => vec![],
            },
        })
    }
}

#[derive(Debug)]
pub(crate) struct ApiGatewayV2ProxyResponseWrapper(apigw::ApiGatewayV2httpResponse);

impl From<ApiGatewayV2ProxyResponseWrapper> for apigw::ApiGatewayV2httpResponse {
    /// Converts instance of the wrapper type to an API Gateway v2 proxy response.
    fn from(response: ApiGatewayV2ProxyResponseWrapper) -> Self {
        response.0
    }
}

impl From<apigw::ApiGatewayV2httpResponse> for ApiGatewayV2ProxyResponseWrapper {
    /// Converts instance of an API Gateway v2 proxy response to the wrapper type.
    fn from(response: apigw::ApiGatewayV2httpResponse) -> Self {
        ApiGatewayV2ProxyResponseWrapper(response)
    }
}

impl TryFrom<wascc_codec::http::Response> for ApiGatewayV2ProxyResponseWrapper {
    type Error = anyhow::Error;

    /// Attempts conversion of an actor's HTTP response to an API Gateway v2 proxy response.
    fn try_from(response: wascc_codec::http::Response) -> anyhow::Result<Self> {
        let (body, is_base64_encoded) = body_string(response.body);

        Ok(apigw::ApiGatewayV2httpResponse {
            status_code: response.status_code as i64,
            headers: response.header,
            multi_value_headers: HashMap::new(),
            body,
            is_base64_encoded: Some(is_base64_encoded),
            cookies: Vec::new(),
        }
        .into())
    }
}

/// Returns a string representation of the specified bytes and
/// a flag indicating whether or not the string is base64 encoded.
fn body_string(bytes: Vec<u8>) -> (Option<String>, bool) {
    if bytes.is_empty() {
        return (None, false);
    }

    match std::str::from_utf8(&bytes) {
        Ok(s) => (Some(s.into()), false),
        Err(_) => (Some(base64::encode(bytes)), true),
    }
}

/// Returns a string representation of the specified query string parameters.
fn query_string(qs: HashMap<String, String>) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(qs.iter())
        .finish()
}

// TODO Handle multi_value_query_string_parameters.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_common::*;
    use std::convert::TryInto;

    /// Returns an empty `AlbTargetGroupRequest`.
    fn empty_alb_target_group_request() -> alb::AlbTargetGroupRequest {
        alb::AlbTargetGroupRequest {
            http_method: None,
            path: None,
            query_string_parameters: HashMap::new(),
            multi_value_query_string_parameters: HashMap::new(),
            headers: HashMap::new(),
            multi_value_headers: HashMap::new(),
            request_context: alb::AlbTargetGroupRequestContext {
                elb: alb::ElbContext {
                    target_group_arn: None,
                },
            },
            is_base64_encoded: false,
            body: None,
        }
    }

    /// Returns an empty `ApiGatewayProxyRequest`.
    fn empty_api_gateway_proxy_request() -> apigw::ApiGatewayProxyRequest {
        apigw::ApiGatewayProxyRequest {
            resource: None,
            path: None,
            http_method: None,
            headers: HashMap::new(),
            multi_value_headers: HashMap::new(),
            query_string_parameters: HashMap::new(),
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
            body: None,
            is_base64_encoded: None,
        }
    }

    /// Returns an empty `ApiGatewayV2httpRequest`.
    fn empty_api_gatewayv2_proxy_request() -> apigw::ApiGatewayV2httpRequest {
        apigw::ApiGatewayV2httpRequest {
            version: None,
            route_key: None,
            raw_path: None,
            raw_query_string: None,
            cookies: None,
            headers: HashMap::new(),
            query_string_parameters: HashMap::new(),
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
                    method: None,
                    path: None,
                    protocol: None,
                    source_ip: None,
                    user_agent: None,
                },
            },
            stage_variables: HashMap::new(),
            body: None,
            is_base64_encoded: false,
        }
    }

    /// Validates an `http::Request`.
    fn validate_http_request(request: &wascc_codec::http::Request) {
        assert_eq!("GET", request.method);
        assert_eq!("/", request.path);
        assert_eq!("key1=value1", request.query_string);
        assert_eq!(1, request.header.len());
        assert!(request.header.contains_key("accept"));
        assert_eq!("application/json", request.header.get("accept").unwrap());
        assert_eq!("Hello world".as_bytes().to_vec(), request.body);
    }

    #[test]
    fn alb_target_group_request_wrapper_empty() {
        let result: Result<wascc_codec::http::Request, _> =
            AlbTargetGroupRequestWrapper::from(empty_alb_target_group_request()).try_into();
        assert!(result.is_err());
    }

    #[test]
    fn alb_target_group_request_wrapper_good() {
        let result: Result<wascc_codec::http::Request, _> =
            AlbTargetGroupRequestWrapper::from(valid_alb_target_group_request()).try_into();
        assert!(result.is_ok());

        validate_http_request(&result.unwrap());
    }

    #[test]
    fn alb_target_group_response_wrapper_good() {
        let result: Result<AlbTargetGroupResponseWrapper, _> = valid_http_response().try_into();
        assert!(result.is_ok());

        let result = result.unwrap().0;
        assert_eq!(200, result.status_code);
        assert_eq!(Some("OK".into()), result.status_description);
        assert_eq!(1, result.headers.len());
        assert!(result.headers.contains_key("server"));
        assert_eq!("test", result.headers.get("server").unwrap());
        assert_eq!(None, result.body);
        assert!(!result.is_base64_encoded);
    }

    #[test]
    fn api_gateway_proxy_request_wrapper_empty() {
        let result: Result<wascc_codec::http::Request, _> =
            ApiGatewayProxyRequestWrapper::from(empty_api_gateway_proxy_request()).try_into();
        assert!(result.is_err());
    }

    #[test]
    fn api_gateway_proxy_request_wrapper_good() {
        let result: Result<wascc_codec::http::Request, _> =
            ApiGatewayProxyRequestWrapper::from(valid_api_gateway_proxy_request()).try_into();
        assert!(result.is_ok());

        validate_http_request(&result.unwrap());
    }

    #[test]
    fn api_gateway_proxy_response_wrapper_good() {
        let result: Result<ApiGatewayProxyResponseWrapper, _> = valid_http_response().try_into();
        assert!(result.is_ok());

        let result = result.unwrap().0;
        assert_eq!(200, result.status_code);
        assert_eq!(1, result.headers.len());
        assert!(result.headers.contains_key("server"));
        assert_eq!("test", result.headers.get("server").unwrap());
        assert_eq!(None, result.body);
        assert_eq!(Some(false), result.is_base64_encoded);
    }

    #[test]
    fn api_gatewayv2_proxy_request_wrapper_empty() {
        let result: Result<wascc_codec::http::Request, _> =
            ApiGatewayV2ProxyRequestWrapper::from(empty_api_gatewayv2_proxy_request()).try_into();
        assert!(result.is_err());
    }

    #[test]
    fn api_gatewayv2_proxy_request_wrapper_good() {
        let result: Result<wascc_codec::http::Request, _> =
            ApiGatewayV2ProxyRequestWrapper::from(valid_api_gatewayv2_proxy_request()).try_into();
        assert!(result.is_ok());

        validate_http_request(&result.unwrap());
    }

    #[test]
    fn api_gatewayv2_proxy_response_wrapper_good() {
        let result: Result<ApiGatewayV2ProxyResponseWrapper, _> = valid_http_response().try_into();
        assert!(result.is_ok());

        let result = result.unwrap().0;
        assert_eq!(200, result.status_code);
        assert_eq!(1, result.headers.len());
        assert!(result.headers.contains_key("server"));
        assert_eq!("test", result.headers.get("server").unwrap());
        assert_eq!(None, result.body);
        assert_eq!(Some(false), result.is_base64_encoded);
    }

    #[test]
    fn body_string_empty() {
        assert_eq!((None, false), body_string(vec![]));
    }

    #[test]
    fn body_string_text() {
        assert_eq!(
            (Some("abc".into()), false),
            body_string(vec![b'a', b'b', b'c'])
        );
    }

    #[test]
    fn body_string_base64() {
        assert_eq!(
            (Some("gYKD".into()), true),
            body_string(vec![0x81, 0x82, 0x83])
        );
    }

    #[test]
    fn query_string_empty() {
        assert_eq!("", query_string(HashMap::new()));
    }

    #[test]
    fn query_string_single() {
        let mut m = HashMap::new();
        m.insert("key1".into(), "value1".into());
        assert_eq!("key1=value1", query_string(m));
    }

    #[test]
    fn query_string_single_with_special() {
        let mut m = HashMap::new();
        m.insert("key1".into(), "val e1".into());
        assert_eq!("key1=val+e1", query_string(m));
    }
}
