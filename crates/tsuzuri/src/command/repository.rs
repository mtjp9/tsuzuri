use crate::{
    aggregate_id::AggregateId,
    domain_event::{DomainEvent, SerializedDomainEvent},
    event::{Envelope, SequenceSelect},
    event_store::EventStore,
    integration_event::{IntegrationEvent, IntoIntegrationEvents, SerializedIntegrationEvent},
    persist::PersistenceError,
    serde::Serde,
    snapshot::PersistedSnapshot,
    AggregateRoot, VersionedAggregate,
};
use async_trait::async_trait;
use futures::TryStreamExt;
use std::marker::PhantomData;

pub trait Repository<T>: AggregateLoader<T> + AggregateCommiter<T> + Send + Sync + 'static
where
    T: AggregateRoot,
{
}

impl<T, R> Repository<T> for R
where
    T: AggregateRoot,
    R: AggregateLoader<T> + AggregateCommiter<T> + Send + Sync + 'static,
{
}

#[async_trait]
pub trait AggregateLoader<T>: Send + Sync
where
    T: AggregateRoot,
{
    async fn load_aggregate(&self, id: &AggregateId<T::ID>) -> Result<VersionedAggregate<T>, PersistenceError>;
}

#[async_trait]
pub trait AggregateCommiter<T>: Send + Sync
where
    T: AggregateRoot,
{
    async fn commit(
        &self,
        versioned_aggregate: &VersionedAggregate<T>,
        event: Envelope<T::DomainEvent>,
    ) -> Result<(), PersistenceError>;
}

#[derive(Debug)]
pub struct EventSourced<T, S, AggSerde, DEvtSerde, IEvtSerde>
where
    T: AggregateRoot,
    S: EventStore,
    AggSerde: Serde<T>,
    DEvtSerde: Serde<T::DomainEvent>,
    IEvtSerde: Serde<T::IntegrationEvent>,
{
    pub store: S,
    pub aggregate_serde: AggSerde,
    pub domain_event_serde: DEvtSerde,
    pub integration_event_serde: IEvtSerde,
    pub aggregate: PhantomData<T>,
}

impl<T, S, AggSerde, DEvtSerde, IEvtSerde> EventSourced<T, S, AggSerde, DEvtSerde, IEvtSerde>
where
    T: AggregateRoot,
    S: EventStore,
    AggSerde: Serde<T>,
    DEvtSerde: Serde<T::DomainEvent>,
    IEvtSerde: Serde<T::IntegrationEvent>,
{
    pub fn new(
        store: S,
        aggregate_serde: AggSerde,
        domain_event_serde: DEvtSerde,
        integration_event_serde: IEvtSerde,
    ) -> Self {
        Self {
            store,
            aggregate_serde,
            domain_event_serde,
            integration_event_serde,
            aggregate: PhantomData,
        }
    }

    async fn prepare_events(
        &self,
        versioned_aggregate: &VersionedAggregate<T>,
        event: Envelope<T::DomainEvent>,
    ) -> Result<(SerializedDomainEvent, Vec<SerializedIntegrationEvent>), PersistenceError> {
        let domain_event = event.message;
        let event_id = domain_event.id();
        let aggregate_id = versioned_aggregate.id();
        let aggregate_type = T::TYPE;
        let event_type = domain_event.event_type();
        let seq_nr = versioned_aggregate.seq_nr();
        let serialized_event = SerializedDomainEvent::new(
            event_id.to_string(),
            aggregate_id.to_string(),
            seq_nr.saturating_add(1),
            aggregate_type.to_string(),
            event_type.to_string(),
            self.domain_event_serde.serialize(domain_event.clone())?,
            serde_json::to_value(event.metadata)?,
        );
        let serialized_integration_events = domain_event
            .into_integration_events()
            .into_iter()
            .map(|integration_event| {
                Ok(SerializedIntegrationEvent::new(
                    integration_event.id().to_string(),
                    aggregate_id.to_string(),
                    T::TYPE.to_string(),
                    integration_event.event_type().to_string(),
                    self.integration_event_serde.serialize(integration_event)?,
                ))
            })
            .collect::<Result<Vec<_>, PersistenceError>>()?;
        Ok((serialized_event, serialized_integration_events))
    }

    async fn prepare_snapshot_if_needed(
        &self,
        versioned_aggregate: &VersionedAggregate<T>,
    ) -> Result<Option<PersistedSnapshot>, PersistenceError>
    where
        T: Clone,
    {
        let aggregate = versioned_aggregate.aggregate();
        let version = versioned_aggregate.version();
        let seq_nr = versioned_aggregate.seq_nr();
        let aggregate_id = aggregate.id();
        // ライブラリの仕様上、1つのイベントを保存するので、
        // 固定で1を指定する
        let num_events = 1;
        let commit_snapshot_to_event = self.store.commit_snapshot_with_addl_events(seq_nr, num_events);

        if commit_snapshot_to_event == 0 {
            return Ok(None);
        }

        let payload = self.aggregate_serde.serialize(aggregate.clone())?;
        let next_snapshot = version.saturating_add(1);

        Ok(Some(PersistedSnapshot::new(
            T::TYPE.to_string(),
            aggregate_id.to_string(),
            payload,
            seq_nr,
            next_snapshot,
        )))
    }
}

#[async_trait]
impl<T, S, AggSerde, DEvtSerde, IEvtSerde> AggregateLoader<T> for EventSourced<T, S, AggSerde, DEvtSerde, IEvtSerde>
where
    T: AggregateRoot,
    S: EventStore,
    AggSerde: Serde<T>,
    DEvtSerde: Serde<T::DomainEvent>,
    IEvtSerde: Serde<T::IntegrationEvent>,
{
    async fn load_aggregate(&self, id: &AggregateId<T::ID>) -> Result<VersionedAggregate<T>, PersistenceError> {
        // スナップショットまたは初期Aggregateを取得
        let (aggregate, version, seq_nr) = match self.store.get_snapshot::<T>(&id.to_string()).await {
            Ok(Some(snapshot)) => {
                // 既存のスナップショットから復元
                (
                    self.aggregate_serde.deserialize(&snapshot.aggregate)?,
                    snapshot.version,
                    snapshot.seq_nr,
                )
            }
            Ok(None) => {
                // 新規Aggregateを作成（スナップショットなし）
                (T::init(id.clone()), 0, 0)
            }
            Err(err) => {
                return Err(PersistenceError::UnknownError(
                    format!("Failed to get snapshot for aggregate {}: {}", id, err).into(),
                ))
            }
        };

        // VersionedAggregateを作成
        let versioned_aggregate = VersionedAggregate::from_snapshot(aggregate, version, seq_nr);

        // イベントストリームから最新状態まで再生
        let ctx = self
            .store
            .stream_events::<T>(&id.to_string(), SequenceSelect::From(seq_nr))
            .try_fold(versioned_aggregate, |mut versioned_aggregate, persisted| async move {
                let event = self.domain_event_serde.deserialize(&persisted.payload)?;
                versioned_aggregate.set_seq_nr(persisted.seq_nr);
                versioned_aggregate.apply(event);
                Ok(versioned_aggregate)
            })
            .await
            .map_err(|err| {
                PersistenceError::UnknownError(format!("Failed to replay events for aggregate {}: {}", id, err).into())
            })?;

        Ok(ctx)
    }
}

// #[async_trait]
// impl<T, S, AggSerde, DEvtSerde, IEvtSerde> AggregateCommiter<T> for EventSourced<T, S, AggSerde, DEvtSerde, IEvtSerde>
// where
//     T: AggregateRoot,
//     S: EventStore,
//     AggSerde: Serde<T>,
//     DEvtSerde: Serde<T::DomainEvent>,
//     IEvtSerde: Serde<T::IntegrationEvent>,
// {
//     async fn commit(
//         &self,
//         versioned_aggregate: &VersionedAggregate<T>,
//         event: Envelope<T::DomainEvent>,
//     ) -> Result<(), PersistenceError> {
//         let (serialized_domain_event, serialized_integration_events) =
//             self.prepare_events(versioned_aggregate, event).await?;
//         let serialized_snapshot = self.prepare_snapshot_if_needed(versioned_aggregate).await?;
//         self.store
//             .persist(
//                 &[serialized_domain_event],
//                 serialized_integration_events.as_ref(),
//                 serialized_snapshot.as_ref(),
//             )
//             .await?;
//         Ok(())
//     }
// }
