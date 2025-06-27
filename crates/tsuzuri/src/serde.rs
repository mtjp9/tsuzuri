use prost::bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::marker::PhantomData;

#[derive(Debug, thiserror::Error)]
pub enum SerdeError {
    #[error("failed to convert type values: {0}")]
    ConversionError(String),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("failed to deserialize protobuf message into value: {0}")]
    ProtobufDeserializationError(#[from] prost::DecodeError),
}

pub trait Serializer<T>: Send + Sync {
    fn serialize(&self, value: T) -> Result<Vec<u8>, SerdeError>;
}

pub trait Deserializer<T>: Send + Sync {
    fn deserialize(&self, data: &[u8]) -> Result<T, SerdeError>;
}

pub trait Serde<T>: Serializer<T> + Deserializer<T> + Send + Sync {}

impl<S, T> Serde<T> for S where S: Serializer<T> + Deserializer<T> {}

#[derive(Clone, Copy)]
pub struct Convert<In, Out, S>
where
    In: Send + Sync,
    Out: Send + Sync,
    S: Serde<Out> + Send + Sync,
{
    serde: S,
    inn: PhantomData<In>,
    out: PhantomData<Out>,
}

impl<In, Out, S> Convert<In, Out, S>
where
    In: Send + Sync,
    Out: Send + Sync,
    S: Serde<Out> + Send + Sync,
{
    pub fn new(serde: S) -> Self {
        Self {
            serde,
            inn: PhantomData,
            out: PhantomData,
        }
    }
}

impl<In, Out, S> Serializer<In> for Convert<In, Out, S>
where
    In: TryFrom<Out> + Send + Sync,
    Out: TryFrom<In> + Send + Sync,
    <Out as TryFrom<In>>::Error: Display,
    S: Serde<Out> + Send + Sync,
{
    fn serialize(&self, value: In) -> Result<Vec<u8>, SerdeError> {
        let out = value
            .try_into()
            .map_err(|err: <Out as TryFrom<In>>::Error| SerdeError::ConversionError(err.to_string()))?;

        self.serde.serialize(out)
    }
}

impl<In, Out, S> Deserializer<In> for Convert<In, Out, S>
where
    In: TryFrom<Out> + Send + Sync,
    Out: TryFrom<In> + Send + Sync,
    <In as TryFrom<Out>>::Error: Display,
    S: Serde<Out> + Send + Sync,
{
    fn deserialize(&self, data: &[u8]) -> Result<In, SerdeError> {
        let out = self.serde.deserialize(data)?;

        out.try_into()
            .map_err(|err: <In as TryFrom<Out>>::Error| SerdeError::ConversionError(err.to_string()))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Json<T>(PhantomData<T>)
where
    T: Serialize + Send + Sync,
    for<'d> T: Deserialize<'d>;

impl<T> Default for Json<T>
where
    T: Serialize + Send + Sync,
    for<'d> T: Deserialize<'d>,
{
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T> Serializer<T> for Json<T>
where
    T: Serialize + Send + Sync,
    for<'d> T: Deserialize<'d>,
{
    fn serialize(&self, value: T) -> Result<Vec<u8>, SerdeError> {
        Ok(serde_json::to_vec(&value)?)
    }
}

impl<T> Deserializer<T> for Json<T>
where
    T: Serialize + Send + Sync,
    for<'d> T: Deserialize<'d>,
{
    fn deserialize(&self, data: &[u8]) -> Result<T, SerdeError> {
        Ok(serde_json::from_slice(data)?)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Protobuf<T>(PhantomData<T>)
where
    T: prost::Message + Default;

impl<T> Serializer<T> for Protobuf<T>
where
    T: prost::Message + Default,
{
    fn serialize(&self, value: T) -> Result<Vec<u8>, SerdeError> {
        Ok(value.encode_to_vec())
    }
}

impl<T> Deserializer<T> for Protobuf<T>
where
    T: prost::Message + Default,
{
    fn deserialize(&self, data: &[u8]) -> Result<T, SerdeError> {
        let buf = Bytes::copy_from_slice(data);
        Ok(T::decode(buf)?)
    }
}

#[derive(Clone, Copy, Default)]
pub struct ProtoJson<T>(PhantomData<T>)
where
    T: prost::Message + Serialize + Default,
    for<'de> T: Deserialize<'de>;

impl<T> Serializer<T> for ProtoJson<T>
where
    T: prost::Message + Serialize + Default,
    for<'de> T: Deserialize<'de>,
{
    fn serialize(&self, value: T) -> Result<Vec<u8>, SerdeError> {
        Json::<T>::default().serialize(value)
    }
}

impl<T> Deserializer<T> for ProtoJson<T>
where
    T: prost::Message + Serialize + Default,
    for<'de> T: Deserialize<'de>,
{
    fn deserialize(&self, data: &[u8]) -> Result<T, SerdeError> {
        Json::<T>::default().deserialize(data)
    }
}
