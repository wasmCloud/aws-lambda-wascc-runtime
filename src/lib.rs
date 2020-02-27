#[macro_use]
extern crate log;

use env_logger;

const CAPABILITY_ID: &str = "awslambda::runtime";

pub struct AwsLambdaRuntimeProvider {}

impl Default for AwsLambdaRuntimeProvider {
    fn default() -> Self {
        env_logger::init();

        info!("hello");

        AwsLambdaRuntimeProvider {}
    }
}

impl AwsLambdaRuntimeProvider {
    pub fn new() -> Self {
        Self::default()
    }
}
