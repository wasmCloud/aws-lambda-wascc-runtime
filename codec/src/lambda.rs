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
        Response {
            body: vec![],
        }
    }

    pub fn json<T>(v: &T) -> Result<Response, Box<dyn std::error::Error>> where T: serde::ser::Serialize + ?Sized {
        Ok(Response {
            body: serde_json::to_vec(v)?,
        })
    }
}
