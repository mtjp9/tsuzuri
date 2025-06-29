use serde::{Deserialize, Serialize};
use tsuzuri::{
    aggregate_id::{AggregateId, HasIdPrefix},
    command::Command,
    domain_event::DomainEvent,
    integration_event::{IntegrationEvent, IntoIntegrationEvents},
    message::Message,
    AggregateRoot, EventId,
};
use uuid::Uuid;

type EventIdType = AggregateId<EventId>;

// Test ID type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TestId;

impl HasIdPrefix for TestId {
    const PREFIX: &'static str = "test";
}

// Error type for test aggregate
#[derive(Debug, Clone, thiserror::Error)]
#[allow(dead_code)]
pub enum TestError {
    #[error("Test error: {0}")]
    TestError(String),
}

// Test aggregate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestAggregate {
    pub id: AggregateId<TestId>,
    pub name: String,
    pub value: i32,
}

// Command enum
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TestCommand {
    Create(CreateTestAggregate),
    Update(UpdateTestAggregate),
}

impl Message for TestCommand {
    fn name(&self) -> &'static str {
        match self {
            TestCommand::Create(_) => "CreateTestAggregate",
            TestCommand::Update(_) => "UpdateTestAggregate",
        }
    }
}

impl Command for TestCommand {
    type ID = TestId;

    fn id(&self) -> AggregateId<Self::ID> {
        match self {
            TestCommand::Create(cmd) => cmd.id.clone(),
            TestCommand::Update(_) => panic!("Update command doesn't have an ID"),
        }
    }
}

// Domain event enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestDomainEvent {
    Created(TestAggregateCreated),
    Updated(TestAggregateUpdated),
}

impl Message for TestDomainEvent {
    fn name(&self) -> &'static str {
        match self {
            TestDomainEvent::Created(_) => "TestAggregateCreated",
            TestDomainEvent::Updated(_) => "TestAggregateUpdated",
        }
    }
}

impl DomainEvent for TestDomainEvent {
    fn id(&self) -> EventIdType {
        EventIdType::new()
    }

    fn event_type(&self) -> &'static str {
        self.name()
    }
}

impl IntoIntegrationEvents for TestDomainEvent {
    type IntegrationEvent = TestIntegrationEvent;
    type IntoIter = std::vec::IntoIter<Self::IntegrationEvent>;

    fn into_integration_events(self) -> Self::IntoIter {
        match self {
            TestDomainEvent::Created(e) => vec![TestIntegrationEvent {
                aggregate_id: e.id,
                message: format!("Aggregate created: {}", e.name),
            }],
            TestDomainEvent::Updated(_) => vec![],
        }
        .into_iter()
    }
}

impl AggregateRoot for TestAggregate {
    type ID = TestId;
    type Command = TestCommand;
    type DomainEvent = TestDomainEvent;
    type IntegrationEvent = TestIntegrationEvent;
    type Error = TestError;
    const TYPE: &'static str = "TestAggregate";

    fn init(id: AggregateId<Self::ID>) -> Self {
        Self {
            id,
            name: String::new(),
            value: 0,
        }
    }

    fn id(&self) -> &AggregateId<Self::ID> {
        &self.id
    }

    fn handle(&mut self, cmd: Self::Command) -> Result<Self::DomainEvent, Self::Error> {
        match cmd {
            TestCommand::Create(c) => Ok(TestDomainEvent::Created(TestAggregateCreated {
                id: c.id,
                name: c.name,
            })),
            TestCommand::Update(u) => Ok(TestDomainEvent::Updated(TestAggregateUpdated { value: u.value })),
        }
    }

    fn apply(&mut self, event: Self::DomainEvent) {
        match event {
            TestDomainEvent::Created(e) => {
                self.name = e.name;
            }
            TestDomainEvent::Updated(e) => {
                self.value = e.value;
            }
        }
    }
}

// Test commands (individual structs for convenience)
#[derive(Debug, Clone)]
pub struct CreateTestAggregate {
    pub id: AggregateId<TestId>,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct UpdateTestAggregate {
    pub value: i32,
}

// Test events (individual structs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestAggregateCreated {
    pub id: AggregateId<TestId>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestAggregateUpdated {
    pub value: i32,
}

// Integration event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestIntegrationEvent {
    pub aggregate_id: AggregateId<TestId>,
    pub message: String,
}

impl Message for TestIntegrationEvent {
    fn name(&self) -> &'static str {
        "TestIntegrationEvent"
    }
}

impl IntegrationEvent for TestIntegrationEvent {
    fn id(&self) -> String {
        Uuid::new_v4().to_string()
    }

    fn event_type(&self) -> &'static str {
        self.name()
    }
}

// Helper functions for tests
#[allow(dead_code)]
pub fn create_test_domain_event(
    aggregate_id: &str,
    seq_nr: usize,
    event_type: &str,
) -> tsuzuri::domain_event::SerializedDomainEvent {
    tsuzuri::domain_event::SerializedDomainEvent {
        id: Uuid::new_v4().to_string(),
        aggregate_id: aggregate_id.to_string(),
        aggregate_type: TestAggregate::TYPE.to_string(),
        seq_nr,
        event_type: event_type.to_string(),
        payload: vec![],
        metadata: Default::default(),
    }
}
