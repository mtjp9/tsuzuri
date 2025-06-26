use crate::{
    aggregate_id::AggregateId, command::Command, domain_event::DomainEvent,
    integration_event::IntegrationEvent,
};
use std::fmt;

pub trait Aggregate: fmt::Debug + Send + Sync + 'static {
    type ID: AggregateId;
    type Command: Command;
    type DomainEvent: DomainEvent;
    type IntegrationEvent: IntegrationEvent;
    type Error: std::error::Error;

    /// Returns the ID of the aggregate.
    fn id(&self) -> &Self::ID;

    /// Handles a command and returns a domain event or an error.
    fn handle(&mut self, cmd: Self::Command) -> Result<Self::DomainEvent, Self::Error>;

    /// Applies changes to the aggregate's state.
    fn apply(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate_id::{IdType, TypedId};
    use std::sync::Arc;

    // Test ID types
    #[derive(Debug, Clone, PartialEq)]
    struct OrderIdType;

    impl IdType for OrderIdType {
        const PREFIX: &'static str = "ord";
    }

    type OrderId = TypedId<OrderIdType>;

    #[derive(Debug, Clone, PartialEq)]
    struct UserIdType;

    impl IdType for UserIdType {
        const PREFIX: &'static str = "usr";
    }

    type UserId = TypedId<UserIdType>;

    // Commands
    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    enum OrderCommand {
        Create { user_id: UserId, total_amount: u64 },
        Confirm,
        Ship,
        Deliver,
    }

    impl Command for OrderCommand {}

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    enum UserCommand {
        Create { name: String, email: String },
        UpdateEmail { email: String },
    }

    impl Command for UserCommand {}

    // Domain Events
    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    enum OrderEvent {
        Created { user_id: UserId, total_amount: u64 },
        Confirmed,
        Shipped,
        Delivered,
    }

    impl DomainEvent for OrderEvent {}

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    enum UserEvent {
        Created {
            name: String,
            email: String,
        },
        EmailUpdated {
            old_email: String,
            new_email: String,
        },
    }

    impl DomainEvent for UserEvent {}

    // Integration Events
    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    enum OrderIntegrationEvent {
        OrderCreatedForNotification {
            order_id: OrderId,
            user_id: UserId,
            total_amount: u64,
        },
        OrderShippedForTracking {
            order_id: OrderId,
            tracking_number: String,
        },
    }

    impl IntegrationEvent for OrderIntegrationEvent {}

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    enum UserIntegrationEvent {
        UserRegisteredForWelcome { user_id: UserId, email: String },
        EmailChangedForVerification { user_id: UserId, new_email: String },
    }

    impl IntegrationEvent for UserIntegrationEvent {}

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
    #[derive(Debug)]
    #[allow(dead_code)]
    struct Order {
        id: OrderId,
        user_id: UserId,
        total_amount: u64,
        status: OrderStatus,
    }

    #[derive(Debug, PartialEq)]
    enum OrderStatus {
        Pending,
        Confirmed,
        Shipped,
        Delivered,
    }

    impl Aggregate for Order {
        type ID = OrderId;
        type Command = OrderCommand;
        type DomainEvent = OrderEvent;
        type IntegrationEvent = OrderIntegrationEvent;
        type Error = OrderError;

        fn id(&self) -> &Self::ID {
            &self.id
        }

        fn handle(&mut self, cmd: Self::Command) -> Result<Self::DomainEvent, Self::Error> {
            match cmd {
                OrderCommand::Create {
                    user_id,
                    total_amount,
                } => Ok(OrderEvent::Created {
                    user_id,
                    total_amount,
                }),
                OrderCommand::Confirm => {
                    if self.status != OrderStatus::Pending {
                        return Err(OrderError::InvalidStateTransition);
                    }
                    Ok(OrderEvent::Confirmed)
                }
                OrderCommand::Ship => {
                    if self.status != OrderStatus::Confirmed {
                        return Err(OrderError::InvalidStateTransition);
                    }
                    Ok(OrderEvent::Shipped)
                }
                OrderCommand::Deliver => {
                    if self.status != OrderStatus::Shipped {
                        return Err(OrderError::InvalidStateTransition);
                    }
                    Ok(OrderEvent::Delivered)
                }
            }
        }

        fn apply(&mut self) {
            // In a real implementation, this would apply events to update state
        }
    }

    #[derive(Debug)]
    #[allow(dead_code)]
    struct User {
        id: UserId,
        name: String,
        email: String,
    }

    impl Aggregate for User {
        type ID = UserId;
        type Command = UserCommand;
        type DomainEvent = UserEvent;
        type IntegrationEvent = UserIntegrationEvent;
        type Error = UserError;

        fn id(&self) -> &Self::ID {
            &self.id
        }

        fn handle(&mut self, cmd: Self::Command) -> Result<Self::DomainEvent, Self::Error> {
            match cmd {
                UserCommand::Create { name, email } => {
                    if !email.contains('@') {
                        return Err(UserError::InvalidEmail);
                    }
                    Ok(UserEvent::Created { name, email })
                }
                UserCommand::UpdateEmail { email } => {
                    if !email.contains('@') {
                        return Err(UserError::InvalidEmail);
                    }
                    let old_email = self.email.clone();
                    Ok(UserEvent::EmailUpdated {
                        old_email,
                        new_email: email,
                    })
                }
            }
        }

        fn apply(&mut self) {
            // In a real implementation, this would apply events to update state
        }
    }

    #[test]
    fn test_aggregate_id_access() {
        let order = Order {
            id: OrderId::new(),
            user_id: UserId::new(),
            total_amount: 10000,
            status: OrderStatus::Pending,
        };

        let order_id = order.id();
        assert!(order_id.to_string().starts_with("ord-"));
    }

    #[test]
    fn test_different_aggregate_types() {
        let order = Order {
            id: OrderId::new(),
            user_id: UserId::new(),
            total_amount: 5000,
            status: OrderStatus::Confirmed,
        };

        let user = User {
            id: UserId::new(),
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

        assert_send_sync::<Order>();
        assert_send_sync::<User>();
    }

    #[test]
    fn test_aggregate_static_lifetime() {
        fn assert_static<T: 'static>() {}

        assert_static::<Order>();
        assert_static::<User>();
    }

    #[test]
    fn test_aggregate_in_arc() {
        let order = Arc::new(Order {
            id: OrderId::new(),
            user_id: UserId::new(),
            total_amount: 15000,
            status: OrderStatus::Shipped,
        });

        let order_clone = Arc::clone(&order);
        assert_eq!(order.id().to_string(), order_clone.id().to_string());
    }

    #[test]
    fn test_aggregate_with_state_changes() {
        let mut order = Order {
            id: OrderId::new(),
            user_id: UserId::new(),
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
        struct Repository<A: Aggregate> {
            storage: HashMap<String, A>,
        }

        impl<A: Aggregate> Repository<A> {
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

        let mut order_repo = Repository::<Order>::new();
        let order = Order {
            id: OrderId::new(),
            user_id: UserId::new(),
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
        let mut order = Order {
            id: OrderId::new(),
            user_id: UserId::new(),
            total_amount: 25000,
            status: OrderStatus::Pending,
        };

        // Test confirm command
        let result = order.handle(OrderCommand::Confirm);
        assert!(result.is_ok());
        order.status = OrderStatus::Confirmed; // Simulate applying event

        // Test ship command
        let result = order.handle(OrderCommand::Ship);
        assert!(result.is_ok());
        order.status = OrderStatus::Shipped; // Simulate applying event

        // Test invalid state transition
        let result = order.handle(OrderCommand::Confirm);
        assert!(result.is_err());
    }

    #[test]
    fn test_user_command_handling() {
        let mut user = User {
            id: UserId::new(),
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
        };

        // Test valid email update
        let result = user.handle(UserCommand::UpdateEmail {
            email: "john.doe@example.com".to_string(),
        });
        assert!(result.is_ok());
        match result.unwrap() {
            UserEvent::EmailUpdated {
                old_email,
                new_email,
            } => {
                assert_eq!(old_email, "john@example.com");
                assert_eq!(new_email, "john.doe@example.com");
            }
            _ => panic!("Unexpected event"),
        }

        // Test invalid email
        let result = user.handle(UserCommand::UpdateEmail {
            email: "invalid-email".to_string(),
        });
        assert!(result.is_err());
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
        let order_id = OrderId::new();
        let user_id = UserId::new();

        let order_created_event = OrderIntegrationEvent::OrderCreatedForNotification {
            order_id: order_id.clone(),
            user_id: user_id.clone(),
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
        let user_id = UserId::new();
        let email = "test@example.com".to_string();

        let user_registered_event = UserIntegrationEvent::UserRegisteredForWelcome {
            user_id: user_id.clone(),
            email: email.clone(),
        };

        match user_registered_event {
            UserIntegrationEvent::UserRegisteredForWelcome {
                user_id: uid,
                email: e,
            } => {
                assert_eq!(uid, user_id);
                assert_eq!(e, email);
            }
            _ => panic!("Unexpected event"),
        }
    }

    #[test]
    fn test_integration_event_flow() {
        // Simulate a complete flow with domain and integration events
        let order_id = OrderId::new();
        let _user_id = UserId::new();

        // Domain event
        let domain_event = OrderEvent::Shipped;

        // Corresponding integration event
        let integration_event = OrderIntegrationEvent::OrderShippedForTracking {
            order_id: order_id.clone(),
            tracking_number: "TRACK123456".to_string(),
        };

        // Verify they can coexist in the same context
        match (&domain_event, &integration_event) {
            (
                OrderEvent::Shipped,
                OrderIntegrationEvent::OrderShippedForTracking {
                    order_id: oid,
                    tracking_number,
                },
            ) => {
                assert_eq!(*oid, order_id);
                assert_eq!(tracking_number, "TRACK123456");
            }
            _ => panic!("Unexpected event combination"),
        }
    }
}
