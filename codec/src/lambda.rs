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
// waSCC AWS Lambda Runtime Codec
//

use serde_json;

pub const OP_HANDLE_EVENT: &str = "HandleEvent";

// Describes an event received from AWS Lambda.
#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Event {
    pub body: Vec<u8>,
}

// Describes a response to AWS Lambda.
#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Response {
    pub body: Vec<u8>,
}

impl Response {
    pub fn empty() -> Response {
        Response { body: vec![] }
    }

    pub fn json<T>(v: &T) -> Result<Response, Box<dyn std::error::Error>>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        Ok(Response {
            body: serde_json::to_vec(v)?,
        })
    }
}
