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
                .ok_or(anyhow!("Missing method in ALB target group request"))?,
            path: request
                .0
                .path
                .ok_or(anyhow!("Missing path in ALB target group request"))?,
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
                .ok_or(anyhow!("Missing method in API Gateway proxy request"))?,
            path: request
                .0
                .path
                .ok_or(anyhow!("Missing path in API Gateway proxy request"))?,
            query_string,
            header: request.0.headers,
            body: match request.0.body {
                Some(s) if request.0.is_base64_encoded.is_some() => base64::decode(s)?,
                Some(s) => s.into_bytes(),
                None => vec![],
            },
        })
    }
}

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
                .ok_or(anyhow!("Missing method in API Gateway v2 proxy request"))?,
            path: request
                .0
                .request_context
                .http
                .path
                .ok_or(anyhow!("Missing path in API Gateway v2 proxy request"))?,
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
