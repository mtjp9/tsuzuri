pub mod lambda;
pub mod local;
// pub mod streams;

pub use lambda::process_kinesis_lambda_event;
