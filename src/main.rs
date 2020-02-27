use env_logger;
use lambda_runtime_core::{lambda, Context, HandlerError};
use log::info;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    lambda!(wascc_handler);

    Ok(())
}

fn wascc_handler(_data: Vec<u8>, _ctx: Context) -> Result<Vec<u8>, HandlerError> {
    info!("wascc_handler entered");

    Ok("all good".as_bytes().to_vec())
}
