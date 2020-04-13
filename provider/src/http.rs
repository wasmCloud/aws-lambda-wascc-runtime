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

use aws_lambda_events::event::alb;

use std::collections::HashMap;
use std::convert::TryFrom;

struct AlbTargetGroupRequestWrapper(alb::AlbTargetGroupRequest);

impl TryFrom<AlbTargetGroupRequestWrapper> for wascc_codec::http::Request {
    type Error = anyhow::Error;

    /// Attempts conversion of an ALB request to an actor's HTTP request.
    fn try_from(request: AlbTargetGroupRequestWrapper) -> anyhow::Result<Self> {
        let query_string = query_string(request.0.query_string_parameters);

        Ok(wascc_codec::http::Request {
            method: request
                .0
                .http_method
                .ok_or(anyhow!("Missing method in ALB request"))?,
            path: request
                .0
                .path
                .ok_or(anyhow!("Missing path in ALB request"))?,
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

struct AlbTargetGroupResponseWrapper(alb::AlbTargetGroupResponse);

impl TryFrom<wascc_codec::http::Response> for AlbTargetGroupResponseWrapper {
    type Error = anyhow::Error;

    /// Attempts conversion of an actor's HTTP response to an ALB response.
    fn try_from(response: wascc_codec::http::Response) -> anyhow::Result<Self> {
        let (body, is_base64_encoded) = body_string(response.body);

        Ok(AlbTargetGroupResponseWrapper(alb::AlbTargetGroupResponse {
            status_code: response.status_code as i64,
            status_description: Some(response.status),
            headers: response.header,
            multi_value_headers: HashMap::new(),
            body,
            is_base64_encoded,
        }))
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
