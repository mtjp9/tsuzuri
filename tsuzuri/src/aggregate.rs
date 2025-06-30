use crate::{
    aggregate_id::{AggregateId, HasIdPrefix},
    command::Command,
    domain_event::DomainEvent,
    integration_event::{IntegrationEvent, IntoIntegrationEvents},
};
use std::fmt;

/// Trait that aggregates must implement to provide their ID prefix
/// and handle commands, domain events, and integration events.
pub trait AggregateRoot: fmt::Debug + Send + Sync + 'static {
    const TYPE: &'static str;
    type ID: HasIdPrefix;
    type Command: Command;
    type DomainEvent: DomainEvent + IntoIntegrationEvents<IntegrationEvent = Self::IntegrationEvent>;
    type IntegrationEvent: IntegrationEvent;
    type Error: std::error::Error;

    /// Initializes a new aggregate with the given ID.
    fn init(id: AggregateId<Self::ID>) -> Self;

    /// Returns the ID of the aggregate.
    fn id(&self) -> &AggregateId<Self::ID>;

    /// Handles a command and returns a domain event or an error.
    fn handle(&mut self, cmd: Self::Command) -> Result<Self::DomainEvent, Self::Error>;

    /// Applies changes to the aggregate's state.
    fn apply(&mut self, event: Self::DomainEvent);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{event_id::EventIdType, integration_event, message, test::TestFramework};
    use std::sync::Arc;

    // Test ID types
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct UserId;

    impl HasIdPrefix for UserId {
        const PREFIX: &'static str = "usr";
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct OrderId;

    impl HasIdPrefix for OrderId {
        const PREFIX: &'static str = "ord";
    }

    // Commands
    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    enum OrderCommand {
        Create {
            id: AggregateId<OrderId>,
            user_id: AggregateId<UserId>,
            total_amount: u64,
        },
        Confirm {
            id: AggregateId<OrderId>,
        },
        Ship {
            id: AggregateId<OrderId>,
        },
        Deliver {
            id: AggregateId<OrderId>,
        },
    }

    impl message::Message for OrderCommand {
        fn name(&self) -> &'static str {
            "OrderCommand"
        }
    }

    impl Command for OrderCommand {
        type ID = OrderId;

        fn id(&self) -> AggregateId<Self::ID> {
            match self {
                Self::Create { id, .. } => *id,
                Self::Confirm { id } => *id,
                Self::Ship { id } => *id,
                Self::Deliver { id } => *id,
            }
        }
    }

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    enum UserCommand {
        Create {
            id: AggregateId<UserId>,
            name: String,
            email: String,
        },
        UpdateEmail {
            id: AggregateId<UserId>,
            email: String,
        },
    }

    impl message::Message for UserCommand {
        fn name(&self) -> &'static str {
            "UserCommand"
        }
    }

    impl Command for UserCommand {
        type ID = UserId;

        fn id(&self) -> AggregateId<Self::ID> {
            match self {
                Self::Create { id, .. } => *id,
                Self::UpdateEmail { id, .. } => *id,
            }
        }
    }

    // Domain Events
    #[derive(Debug, Clone, PartialEq)]
    #[allow(dead_code)]
    enum OrderEvent {
        Created {
            id: EventIdType,
            user_id: AggregateId<UserId>,
            total_amount: u64,
        },
        Confirmed {
            id: EventIdType,
        },
        Shipped {
            id: EventIdType,
        },
        Delivered {
            id: EventIdType,
        },
    }

    impl message::Message for OrderEvent {
        fn name(&self) -> &'static str {
            "OrderEvent"
        }
    }

    impl DomainEvent for OrderEvent {
        fn id(&self) -> EventIdType {
            match self {
                Self::Created { id, .. } => *id,
                Self::Confirmed { id } => *id,
                Self::Shipped { id } => *id,
                Self::Delivered { id } => *id,
            }
        }

        fn event_type(&self) -> &'static str {
            match self {
                Self::Created { .. } => "OrderCreated",
                Self::Confirmed { .. } => "OrderConfirmed",
                Self::Shipped { .. } => "OrderShipped",
                Self::Delivered { .. } => "OrderDelivered",
            }
        }
    }

    impl integration_event::IntoIntegrationEvents for OrderEvent {
        type IntegrationEvent = OrderIntegrationEvent;
        type IntoIter = Vec<OrderIntegrationEvent>;

        fn into_integration_events(self) -> Self::IntoIter {
            match self {
                OrderEvent::Created {
                    user_id, total_amount, ..
                } => {
                    vec![OrderIntegrationEvent::OrderCreatedForNotification {
                        order_id: AggregateId::<OrderId>::new(),
                        user_id,
                        total_amount,
                    }]
                }
                OrderEvent::Shipped { .. } => {
                    vec![OrderIntegrationEvent::OrderShippedForTracking {
                        order_id: AggregateId::<OrderId>::new(),
                        tracking_number: "TRACK123456".to_string(),
                    }]
                }
                _ => vec![],
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    #[allow(dead_code)]
    enum UserEvent {
        Created {
            id: EventIdType,
            name: String,
            email: String,
        },
        EmailUpdated {
            id: EventIdType,
            old_email: String,
            new_email: String,
        },
    }

    impl message::Message for UserEvent {
        fn name(&self) -> &'static str {
            "UserEvent"
        }
    }

    impl DomainEvent for UserEvent {
        fn id(&self) -> EventIdType {
            match self {
                Self::Created { id, .. } => *id,
                Self::EmailUpdated { id, .. } => *id,
            }
        }

        fn event_type(&self) -> &'static str {
            match self {
                Self::Created { .. } => "UserCreated",
                Self::EmailUpdated { .. } => "UserEmailUpdated",
            }
        }
    }

    impl integration_event::IntoIntegrationEvents for UserEvent {
        type IntegrationEvent = UserIntegrationEvent;
        type IntoIter = Vec<UserIntegrationEvent>;

        fn into_integration_events(self) -> Self::IntoIter {
            match self {
                UserEvent::Created { name: _, email, .. } => {
                    vec![UserIntegrationEvent::UserRegisteredForWelcome {
                        user_id: AggregateId::<UserId>::new(),
                        email,
                    }]
                }
                UserEvent::EmailUpdated { new_email, .. } => {
                    vec![UserIntegrationEvent::EmailChangedForVerification {
                        user_id: AggregateId::<UserId>::new(),
                        new_email,
                    }]
                }
            }
        }
    }

    // Integration Events
    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    enum OrderIntegrationEvent {
        OrderCreatedForNotification {
            order_id: AggregateId<OrderId>,
            user_id: AggregateId<UserId>,
            total_amount: u64,
        },
        OrderShippedForTracking {
            order_id: AggregateId<OrderId>,
            tracking_number: String,
        },
    }

    impl message::Message for OrderIntegrationEvent {
        fn name(&self) -> &'static str {
            "OrderIntegrationEvent"
        }
    }

    impl IntegrationEvent for OrderIntegrationEvent {
        fn id(&self) -> String {
            ulid::Ulid::new().to_string()
        }

        fn event_type(&self) -> &'static str {
            match self {
                Self::OrderCreatedForNotification { .. } => "order.created.for_notification",
                Self::OrderShippedForTracking { .. } => "order.shipped.for_tracking",
            }
        }
    }

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    enum UserIntegrationEvent {
        UserRegisteredForWelcome {
            user_id: AggregateId<UserId>,
            email: String,
        },
        EmailChangedForVerification {
            user_id: AggregateId<UserId>,
            new_email: String,
        },
    }

    impl message::Message for UserIntegrationEvent {
        fn name(&self) -> &'static str {
            "UserIntegrationEvent"
        }
    }

    impl IntegrationEvent for UserIntegrationEvent {
        fn id(&self) -> String {
            ulid::Ulid::new().to_string()
        }

        fn event_type(&self) -> &'static str {
            match self {
                Self::UserRegisteredForWelcome { .. } => "user.registered.for_welcome",
                Self::EmailChangedForVerification { .. } => "email.changed.for_verification",
            }
        }
    }

    // Errors
    #[derive(Debug, thiserror::Error)]
    #[allow(dead_code)]
    enum OrderError {
        #[error("Invalid state transition")]
        InvalidStateTransition,
        #[error("Order already exists")]
        AlreadyExists,
    }

    #[derive(Debug, thiserror::Error)]
    #[allow(dead_code)]
    enum UserError {
        #[error("Invalid email format")]
        InvalidEmail,
        #[error("User already exists")]
        AlreadyExists,
    }

    // Test Aggregates
    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct OrderAggregate {
        id: AggregateId<OrderId>,
        user_id: AggregateId<UserId>,
        total_amount: u64,
        status: OrderStatus,
    }

    #[derive(Debug, Clone, PartialEq)]
    enum OrderStatus {
        Pending,
        Confirmed,
        Shipped,
        Delivered,
    }

    impl AggregateRoot for OrderAggregate {
        const TYPE: &'static str = "Order";
        type ID = OrderId;
        type Command = OrderCommand;
        type DomainEvent = OrderEvent;
        type IntegrationEvent = OrderIntegrationEvent;
        type Error = OrderError;

        fn init(id: AggregateId<Self::ID>) -> Self {
            Self {
                id,
                user_id: AggregateId::<UserId>::new(), // Default user ID
                total_amount: 0,
                status: OrderStatus::Pending,
            }
        }

        fn id(&self) -> &AggregateId<Self::ID> {
            &self.id
        }

        fn handle(&mut self, cmd: Self::Command) -> Result<Self::DomainEvent, Self::Error> {
            match cmd {
                OrderCommand::Create {
                    id: _,
                    user_id,
                    total_amount,
                } => Ok(OrderEvent::Created {
                    id: EventIdType::new(),
                    user_id,
                    total_amount,
                }),
                OrderCommand::Confirm { id: _ } => {
                    if self.status != OrderStatus::Pending {
                        return Err(OrderError::InvalidStateTransition);
                    }
                    Ok(OrderEvent::Confirmed { id: EventIdType::new() })
                }
                OrderCommand::Ship { id: _ } => {
                    if self.status != OrderStatus::Confirmed {
                        return Err(OrderError::InvalidStateTransition);
                    }
                    Ok(OrderEvent::Shipped { id: EventIdType::new() })
                }
                OrderCommand::Deliver { id: _ } => {
                    if self.status != OrderStatus::Shipped {
                        return Err(OrderError::InvalidStateTransition);
                    }
                    Ok(OrderEvent::Delivered { id: EventIdType::new() })
                }
            }
        }

        fn apply(&mut self, event: Self::DomainEvent) {
            match event {
                OrderEvent::Created {
                    id: _,
                    user_id,
                    total_amount,
                } => {
                    self.user_id = user_id;
                    self.total_amount = total_amount;
                    self.status = OrderStatus::Pending;
                }
                OrderEvent::Confirmed { id: _ } => {
                    self.status = OrderStatus::Confirmed;
                }
                OrderEvent::Shipped { id: _ } => {
                    self.status = OrderStatus::Shipped;
                }
                OrderEvent::Delivered { id: _ } => {
                    self.status = OrderStatus::Delivered;
                }
            }
        }
    }

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct UserAggregate {
        id: AggregateId<UserId>,
        name: String,
        email: String,
    }

    impl AggregateRoot for UserAggregate {
        const TYPE: &'static str = "User";
        type ID = UserId;
        type Command = UserCommand;
        type DomainEvent = UserEvent;
        type IntegrationEvent = UserIntegrationEvent;
        type Error = UserError;

        fn init(id: AggregateId<Self::ID>) -> Self {
            Self {
                id,
                name: String::new(),
                email: String::new(),
            }
        }

        fn id(&self) -> &AggregateId<Self::ID> {
            &self.id
        }

        fn handle(&mut self, cmd: Self::Command) -> Result<Self::DomainEvent, Self::Error> {
            match cmd {
                UserCommand::Create { id: _, name, email } => {
                    if !email.contains('@') {
                        return Err(UserError::InvalidEmail);
                    }
                    Ok(UserEvent::Created {
                        id: EventIdType::new(),
                        name,
                        email,
                    })
                }
                UserCommand::UpdateEmail { id: _, email } => {
                    if !email.contains('@') {
                        return Err(UserError::InvalidEmail);
                    }
                    let old_email = self.email.clone();
                    Ok(UserEvent::EmailUpdated {
                        id: EventIdType::new(),
                        old_email,
                        new_email: email,
                    })
                }
            }
        }

        fn apply(&mut self, event: Self::DomainEvent) {
            match event {
                UserEvent::Created { id: _, name, email } => {
                    self.name = name;
                    self.email = email;
                }
                UserEvent::EmailUpdated {
                    id: _,
                    old_email: _,
                    new_email,
                } => {
                    self.email = new_email;
                }
            }
        }
    }

    #[test]
    fn test_aggregate_id_access() {
        let order = OrderAggregate {
            id: AggregateId::<OrderId>::new(),
            user_id: AggregateId::<UserId>::new(),
            total_amount: 10000,
            status: OrderStatus::Pending,
        };

        let order_id = order.id();
        assert!(order_id.to_string().starts_with("ord-"));
    }

    #[test]
    fn test_different_aggregate_types() {
        let order = OrderAggregate {
            id: AggregateId::<OrderId>::new(),
            user_id: AggregateId::<UserId>::new(),
            total_amount: 5000,
            status: OrderStatus::Confirmed,
        };

        let user = UserAggregate {
            id: AggregateId::<UserId>::new(),
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
        };

        // Ensure different ID types
        assert!(order.id().to_string().starts_with("ord-"));
        assert!(user.id().to_string().starts_with("usr-"));
    }

    #[test]
    fn test_aggregate_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<OrderAggregate>();
        assert_send_sync::<UserAggregate>();
    }

    #[test]
    fn test_aggregate_static_lifetime() {
        fn assert_static<T: 'static>() {}

        assert_static::<OrderAggregate>();
        assert_static::<UserAggregate>();
    }

    #[test]
    fn test_aggregate_in_arc() {
        let order = Arc::new(OrderAggregate {
            id: AggregateId::<OrderId>::new(),
            user_id: AggregateId::<UserId>::new(),
            total_amount: 15000,
            status: OrderStatus::Shipped,
        });

        let order_clone = Arc::clone(&order);
        assert_eq!(order.id().to_string(), order_clone.id().to_string());
    }

    #[test]
    fn test_aggregate_with_state_changes() {
        let mut order = OrderAggregate {
            id: AggregateId::<OrderId>::new(),
            user_id: AggregateId::<UserId>::new(),
            total_amount: 20000,
            status: OrderStatus::Pending,
        };

        // Simulate state changes (in real CQRS, this would be through events)
        order.status = OrderStatus::Confirmed;
        assert_eq!(order.status, OrderStatus::Confirmed);

        order.status = OrderStatus::Shipped;
        assert_eq!(order.status, OrderStatus::Shipped);

        // ID should remain constant
        let id_before = order.id().to_string();
        order.status = OrderStatus::Delivered;
        let id_after = order.id().to_string();
        assert_eq!(id_before, id_after);
    }

    #[test]
    fn test_aggregate_repository_simulation() {
        use std::collections::HashMap;

        // Simulate a simple repository
        struct Repository<A: AggregateRoot> {
            storage: HashMap<String, A>,
        }

        impl<A: AggregateRoot> Repository<A> {
            fn new() -> Self {
                Self {
                    storage: HashMap::new(),
                }
            }

            fn save(&mut self, aggregate: A) {
                let id = aggregate.id().to_string();
                self.storage.insert(id, aggregate);
            }

            fn get(&self, id: &str) -> Option<&A> {
                self.storage.get(id)
            }
        }

        let mut order_repo = Repository::<OrderAggregate>::new();
        let order = OrderAggregate {
            id: AggregateId::<OrderId>::new(),
            user_id: AggregateId::<UserId>::new(),
            total_amount: 30000,
            status: OrderStatus::Pending,
        };

        let order_id_string = order.id().to_string();
        order_repo.save(order);

        let retrieved = order_repo.get(&order_id_string);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().total_amount, 30000);
    }

    #[test]
    fn test_order_command_handling() {
        let order_id = AggregateId::<OrderId>::new();
        let user_id = AggregateId::<UserId>::new();
        let order = OrderAggregate::init(order_id);

        // Test create command
        TestFramework::with(order.clone())
            .given_no_previous_events()
            .when(OrderCommand::Create {
                id: order_id,
                user_id,
                total_amount: 25000,
            })
            .then_verify(|result| {
                assert!(result.is_ok());
                let events = result.unwrap();
                assert_eq!(events.len(), 1);
                match &events[0] {
                    OrderEvent::Created {
                        id: _,
                        user_id: uid,
                        total_amount,
                    } => {
                        assert_eq!(uid, &user_id);
                        assert_eq!(total_amount, &25000);
                    }
                    _ => panic!("Expected OrderEvent::Created"),
                }
            });

        // Test confirm command from pending state
        TestFramework::with(order.clone())
            .given(vec![OrderEvent::Created {
                id: EventIdType::new(),
                user_id,
                total_amount: 25000,
            }])
            .when(OrderCommand::Confirm { id: order_id })
            .then_verify(|result| {
                assert!(result.is_ok());
                let events = result.unwrap();
                assert_eq!(events.len(), 1);
                assert!(matches!(events[0], OrderEvent::Confirmed { .. }));
            });

        // Test ship command from confirmed state
        TestFramework::with(order.clone())
            .given(vec![
                OrderEvent::Created {
                    id: EventIdType::new(),
                    user_id,
                    total_amount: 25000,
                },
                OrderEvent::Confirmed { id: EventIdType::new() },
            ])
            .when(OrderCommand::Ship { id: order_id })
            .then_verify(|result| {
                assert!(result.is_ok());
                let events = result.unwrap();
                assert_eq!(events.len(), 1);
                assert!(matches!(events[0], OrderEvent::Shipped { .. }));
            });

        // Test invalid state transition - trying to confirm when already shipped
        TestFramework::with(order)
            .given(vec![
                OrderEvent::Created {
                    id: EventIdType::new(),
                    user_id,
                    total_amount: 25000,
                },
                OrderEvent::Confirmed { id: EventIdType::new() },
                OrderEvent::Shipped { id: EventIdType::new() },
            ])
            .when(OrderCommand::Confirm { id: order_id })
            .then_expect_error_matches(|e| matches!(e, OrderError::InvalidStateTransition));
    }

    #[test]
    fn test_user_command_handling() {
        let user_id = AggregateId::<UserId>::new();
        let user = UserAggregate::init(user_id);

        // Test create command with valid email
        TestFramework::with(user.clone())
            .given_no_previous_events()
            .when(UserCommand::Create {
                id: user_id,
                name: "John Doe".to_string(),
                email: "john@example.com".to_string(),
            })
            .then_verify(|result| {
                assert!(result.is_ok());
                let events = result.unwrap();
                assert_eq!(events.len(), 1);
                match &events[0] {
                    UserEvent::Created { id: _, name, email } => {
                        assert_eq!(name, "John Doe");
                        assert_eq!(email, "john@example.com");
                    }
                    _ => panic!("Expected UserEvent::Created"),
                }
            });

        // Test email update with valid email
        TestFramework::with(user.clone())
            .given(vec![UserEvent::Created {
                id: EventIdType::new(),
                name: "John Doe".to_string(),
                email: "john@example.com".to_string(),
            }])
            .when(UserCommand::UpdateEmail {
                id: user_id,
                email: "john.doe@example.com".to_string(),
            })
            .then_verify(|result| {
                assert!(result.is_ok());
                let events = result.unwrap();
                assert_eq!(events.len(), 1);
                match &events[0] {
                    UserEvent::EmailUpdated {
                        id: _,
                        old_email,
                        new_email,
                    } => {
                        assert_eq!(old_email, "john@example.com");
                        assert_eq!(new_email, "john.doe@example.com");
                    }
                    _ => panic!("Expected UserEvent::EmailUpdated"),
                }
            });

        // Test create command with invalid email
        TestFramework::with(user.clone())
            .given_no_previous_events()
            .when(UserCommand::Create {
                id: user_id,
                name: "John Doe".to_string(),
                email: "invalid-email".to_string(),
            })
            .then_expect_error_matches(|e| matches!(e, UserError::InvalidEmail));

        // Test email update with invalid email
        TestFramework::with(user)
            .given(vec![UserEvent::Created {
                id: EventIdType::new(),
                name: "John Doe".to_string(),
                email: "john@example.com".to_string(),
            }])
            .when(UserCommand::UpdateEmail {
                id: user_id,
                email: "invalid-email".to_string(),
            })
            .then_expect_error_matches(|e| matches!(e, UserError::InvalidEmail));
    }

    #[test]
    fn test_command_and_event_traits() {
        fn assert_command<T: Command>() {}
        fn assert_domain_event<T: DomainEvent>() {}
        fn assert_integration_event<T: IntegrationEvent>() {}

        assert_command::<OrderCommand>();
        assert_command::<UserCommand>();
        assert_domain_event::<OrderEvent>();
        assert_domain_event::<UserEvent>();
        assert_integration_event::<OrderIntegrationEvent>();
        assert_integration_event::<UserIntegrationEvent>();
    }

    #[test]
    fn test_integration_event_creation() {
        // Test Order integration events
        let order_id = AggregateId::<OrderId>::new();
        let user_id = AggregateId::<UserId>::new();

        let order_created_event = OrderIntegrationEvent::OrderCreatedForNotification {
            order_id,
            user_id,
            total_amount: 50000,
        };

        match order_created_event {
            OrderIntegrationEvent::OrderCreatedForNotification {
                order_id: oid,
                user_id: uid,
                total_amount,
            } => {
                assert_eq!(oid, order_id);
                assert_eq!(uid, user_id);
                assert_eq!(total_amount, 50000);
            }
            _ => panic!("Unexpected event"),
        }

        // Test User integration events
        let user_id = AggregateId::<UserId>::new();
        let email = "test@example.com".to_string();

        let user_registered_event = UserIntegrationEvent::UserRegisteredForWelcome {
            user_id,
            email: email.clone(),
        };

        match user_registered_event {
            UserIntegrationEvent::UserRegisteredForWelcome { user_id: uid, email: e } => {
                assert_eq!(uid, user_id);
                assert_eq!(e, email);
            }
            _ => panic!("Unexpected event"),
        }
    }

    #[test]
    fn test_integration_event_flow() {
        // Simulate a complete flow with domain and integration events
        let order_id = AggregateId::<OrderId>::new();
        let _user_id = AggregateId::<UserId>::new();

        // Domain event
        let domain_event = OrderEvent::Shipped { id: EventIdType::new() };

        // Corresponding integration event
        let integration_event = OrderIntegrationEvent::OrderShippedForTracking {
            order_id,
            tracking_number: "TRACK123456".to_string(),
        };

        // Verify they can coexist in the same context
        match (&domain_event, &integration_event) {
            (
                OrderEvent::Shipped { id: _ },
                OrderIntegrationEvent::OrderShippedForTracking {
                    order_id: oid,
                    tracking_number,
                },
            ) => {
                assert_eq!(oid, &order_id);
                assert_eq!(tracking_number, "TRACK123456");
            }
            _ => panic!("Unexpected event combination"),
        }
    }

    #[test]
    fn test_aggregate_init() {
        // Test OrderAggregate init
        let order_id = AggregateId::<OrderId>::new();
        let order = OrderAggregate::init(order_id);

        assert_eq!(order.id, order_id);
        assert_eq!(order.total_amount, 0);
        assert_eq!(order.status, OrderStatus::Pending);
        assert!(order.user_id.to_string().starts_with("usr-"));

        // Test UserAggregate init
        let user_id = AggregateId::<UserId>::new();
        let user = UserAggregate::init(user_id);

        assert_eq!(user.id, user_id);
        assert_eq!(user.name, "");
        assert_eq!(user.email, "");
    }

    #[test]
    fn test_apply_method() {
        // Test OrderAggregate apply
        let mut order = OrderAggregate {
            id: AggregateId::<OrderId>::new(),
            user_id: AggregateId::<UserId>::new(),
            total_amount: 0,
            status: OrderStatus::Pending,
        };

        // Apply Created event
        let user_id = AggregateId::<UserId>::new();
        order.apply(OrderEvent::Created {
            id: EventIdType::new(),
            user_id,
            total_amount: 10000,
        });
        assert_eq!(order.user_id, user_id);
        assert_eq!(order.total_amount, 10000);
        assert_eq!(order.status, OrderStatus::Pending);

        // Apply Confirmed event
        order.apply(OrderEvent::Confirmed { id: EventIdType::new() });
        assert_eq!(order.status, OrderStatus::Confirmed);

        // Apply Shipped event
        order.apply(OrderEvent::Shipped { id: EventIdType::new() });
        assert_eq!(order.status, OrderStatus::Shipped);

        // Apply Delivered event
        order.apply(OrderEvent::Delivered { id: EventIdType::new() });
        assert_eq!(order.status, OrderStatus::Delivered);

        // Test UserAggregate apply
        let mut user = UserAggregate {
            id: AggregateId::<UserId>::new(),
            name: String::new(),
            email: String::new(),
        };

        // Apply Created event
        user.apply(UserEvent::Created {
            id: EventIdType::new(),
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
        });
        assert_eq!(user.name, "John Doe");
        assert_eq!(user.email, "john@example.com");

        // Apply EmailUpdated event
        user.apply(UserEvent::EmailUpdated {
            id: EventIdType::new(),
            old_email: "john@example.com".to_string(),
            new_email: "john.doe@example.com".to_string(),
        });
        assert_eq!(user.email, "john.doe@example.com");
        assert_eq!(user.name, "John Doe"); // Name should remain unchanged
    }
}
