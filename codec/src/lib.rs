//
// waSCC AWS Lambda Runtime Codec
//

pub mod aws_lambda_runtime {
    // Include generated code.
    include!(concat!(env!("OUT_DIR"), "/lambda.rs"));

    use prost::Message;

    pub const OP_HANDLE_EVENT: &str = "HandleEvent";

    impl Into<Event> for &[u8] {
        fn into(self) -> Event {
            Event::decode(self).unwrap()
        }
    }
}
