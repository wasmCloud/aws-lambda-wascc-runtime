//
// waSCC AWS Lambda Runtime Codec
//

pub mod aws_lambda_runtime {
    include!(concat!(env!("OUT_DIR"), "/lambda.rs"));

    pub const OP_HANDLE_EVENT: &str = "HandleEvent";
}
