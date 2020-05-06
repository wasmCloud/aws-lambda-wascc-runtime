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
// waSCC AWS Lambda Event Codec
//

pub const OP_HANDLE_EVENT: &str = "HandleEvent";

/// Describes an event received from AWS Lambda.
#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Event {
    /// The raw JSON bytes of the event body.
    #[serde(with = "serde_bytes")]
    #[serde(default)]
    pub body: Vec<u8>,
}

/// Describes a response to AWS Lambda.
#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Response {
    /// The raw JSON bytes of the response body.
    #[serde(with = "serde_bytes")]
    #[serde(default)]
    pub body: Vec<u8>,
}

impl Response {
    /// Returns an empty response.
    pub fn empty() -> Response {
        Self::default()
    }

    /// Returns a response that contains the JSON serialization of an object.
    pub fn json<T>(t: &T) -> Result<Response, Box<dyn std::error::Error>>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        Ok(Response {
            body: serde_json::to_vec(t)?,
        })
    }
}

impl Default for Response {
    /// Returns the default value for `Response`.
    /// The default Response is empty.
    fn default() -> Self {
        Response { body: vec![] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i32_json_response() {
        let i: i32 = 42;
        let result = Response::json(&i);
        assert!(result.is_ok());
    }
}
