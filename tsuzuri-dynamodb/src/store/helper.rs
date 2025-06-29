use crate::store::error::DynamoAggregateError;
use aws_sdk_dynamodb::{
    types::{AttributeValue, TransactWriteItem},
    Client,
};
use serde_json::Value;
use std::collections::HashMap;
use tsuzuri::domain_event::SerializedDomainEvent;

pub fn att_as_vec(
    values: &HashMap<String, AttributeValue>,
    attribute_name: &str,
) -> Result<Vec<u8>, DynamoAggregateError> {
    let attribute = require_attribute(values, attribute_name)?;
    match attribute.as_b() {
        Ok(payload_blob) => Ok(payload_blob.as_ref().to_vec()),
        Err(_) => Err(DynamoAggregateError::MissingAttribute(attribute_name.to_string())),
    }
}

pub fn att_as_value(
    values: &HashMap<String, AttributeValue>,
    attribute_name: &str,
) -> Result<Value, DynamoAggregateError> {
    let attribute = require_attribute(values, attribute_name)?;
    match attribute.as_b() {
        Ok(payload_blob) => Ok(serde_json::from_slice(payload_blob.as_ref())?),
        Err(_) => Err(DynamoAggregateError::MissingAttribute(attribute_name.to_string())),
    }
}

pub fn att_as_number(
    values: &HashMap<String, AttributeValue>,
    attribute_name: &str,
) -> Result<usize, DynamoAggregateError> {
    let attribute = require_attribute(values, attribute_name)?;
    match attribute.as_n() {
        Ok(attribute_as_n) => attribute_as_n
            .parse::<usize>()
            .map_err(|_| DynamoAggregateError::MissingAttribute(attribute_name.to_string())),
        Err(_) => Err(DynamoAggregateError::MissingAttribute(attribute_name.to_string())),
    }
}

pub fn att_as_string(
    values: &HashMap<String, AttributeValue>,
    attribute_name: &str,
) -> Result<String, DynamoAggregateError> {
    let attribute = require_attribute(values, attribute_name)?;
    match attribute.as_s() {
        Ok(attribute_as_s) => Ok(attribute_as_s.to_string()),
        Err(_) => Err(DynamoAggregateError::MissingAttribute(attribute_name.to_string())),
    }
}

pub fn require_attribute<'a>(
    values: &'a HashMap<String, AttributeValue>,
    attribute_name: &str,
) -> Result<&'a AttributeValue, DynamoAggregateError> {
    values
        .get(attribute_name)
        .ok_or(DynamoAggregateError::MissingAttribute(attribute_name.to_string()))
}

pub fn serialized_event(entry: HashMap<String, AttributeValue>) -> Result<SerializedDomainEvent, DynamoAggregateError> {
    let id = att_as_string(&entry, "event_id")?;
    let aggregate_id = att_as_string(&entry, "aid")?;
    let seq_nr = att_as_number(&entry, "seq_nr")?;
    let aggregate_type = att_as_string(&entry, "aggregate_type")?;
    let event_type = att_as_string(&entry, "event_type")?;
    let payload = att_as_vec(&entry, "payload")?;
    let metadata = att_as_value(&entry, "metadata")?;

    Ok(SerializedDomainEvent {
        id,
        aggregate_id,
        seq_nr,
        aggregate_type,
        event_type,
        payload,
        metadata,
    })
}

pub async fn commit_transactions(
    client: &Client,
    transactions: Vec<TransactWriteItem>,
) -> Result<(), DynamoAggregateError> {
    let transaction_len = transactions.len();
    if transaction_len > 25 {
        return Err(DynamoAggregateError::TransactionListTooLong(transaction_len));
    }
    client
        .transact_write_items()
        .set_transact_items(Some(transactions))
        .send()
        .await?;
    Ok(())
}
