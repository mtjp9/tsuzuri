use aws_sdk_dynamodb::operation::describe_table::DescribeTableError;
// use aws_sdk_dynamodbstreams::operation::{
//     describe_stream::DescribeStreamError, get_records::GetRecordsError, get_shard_iterator::GetShardIteratorError,
// };
use tsuzuri::{integration::error::IntegrationError, projection::error::ProjectionError};

#[derive(thiserror::Error, Debug)]
pub enum StreamProcessorError {
    // #[error("Configuration error: {0}")]
    // Config(String),
    #[error("DynamoDB error: {0}")]
    DynamoDB(#[from] Box<aws_sdk_dynamodb::error::SdkError<DescribeTableError>>),

    #[error("DynamoDB Streams error: {0}")]
    DynamoDBStreams(String),

    #[error("Kinesis Data Streams error: {0}")]
    KinesisDataStreams(String),

    #[error("Tsuzuri projection error: {0}")]
    Projection(#[from] ProjectionError),

    #[error("Tsuzuri integration error: {0}")]
    Integration(#[from] IntegrationError),

    #[error("Invalid data: {0}")]
    InvalidData(String),
}

// impl From<aws_sdk_dynamodbstreams::error::SdkError<DescribeStreamError>> for StreamProcessorError {
//     fn from(err: aws_sdk_dynamodbstreams::error::SdkError<DescribeStreamError>) -> Self {
//         StreamProcessorError::DynamoDBStreams(err.to_string())
//     }
// }

// impl From<aws_sdk_dynamodbstreams::error::SdkError<GetShardIteratorError>> for StreamProcessorError {
//     fn from(err: aws_sdk_dynamodbstreams::error::SdkError<GetShardIteratorError>) -> Self {
//         StreamProcessorError::DynamoDBStreams(err.to_string())
//     }
// }

// impl From<aws_sdk_dynamodbstreams::error::SdkError<GetRecordsError>> for StreamProcessorError {
//     fn from(err: aws_sdk_dynamodbstreams::error::SdkError<GetRecordsError>) -> Self {
//         StreamProcessorError::DynamoDBStreams(err.to_string())
//     }
// }

pub type Result<T> = std::result::Result<T, StreamProcessorError>;
