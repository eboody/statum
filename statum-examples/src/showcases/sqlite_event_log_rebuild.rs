use sqlx::{FromRow, SqlitePool, sqlite::SqlitePoolOptions};
use statum::{
    machine,
    projection::{ProjectionError, ProjectionReducer, reduce_grouped, reduce_one},
    state, transition, validators,
};

const EVENT_CREATED: &str = "created";
const EVENT_PAID: &str = "paid";
const EVENT_PACKED: &str = "packed";
const EVENT_SHIPPED: &str = "shipped";
const EVENT_DELIVERED: &str = "delivered";

#[state]
pub enum OrderState {
    Created(CreatedOrder),
    Paid(PaidOrder),
    Packed(PackedOrder),
    Shipped(ShippedOrder),
    Delivered(DeliveredOrder),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderContext {
    pub order_id: i64,
    pub customer: String,
    pub sku: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatedOrder {
    pub order: OrderContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaidOrder {
    pub order: OrderContext,
    pub payment_receipt: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackedOrder {
    pub order: OrderContext,
    pub payment_receipt: String,
    pub pick_ticket: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShippedOrder {
    pub order: OrderContext,
    pub payment_receipt: String,
    pub pick_ticket: String,
    pub tracking_number: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveredOrder {
    pub order: OrderContext,
    pub payment_receipt: String,
    pub pick_ticket: String,
    pub tracking_number: String,
}

#[machine]
pub struct OrderMachine<OrderState> {}

#[transition]
impl OrderMachine<Created> {
    fn pay(self, payment_receipt: String) -> OrderMachine<Paid> {
        self.transition_map(|created| PaidOrder {
            order: created.order,
            payment_receipt,
        })
    }
}

#[transition]
impl OrderMachine<Paid> {
    fn pack(self, pick_ticket: String) -> OrderMachine<Packed> {
        self.transition_map(|paid| PackedOrder {
            order: paid.order,
            payment_receipt: paid.payment_receipt,
            pick_ticket,
        })
    }
}

#[transition]
impl OrderMachine<Packed> {
    fn ship(self, tracking_number: String) -> OrderMachine<Shipped> {
        self.transition_map(|packed| ShippedOrder {
            order: packed.order,
            payment_receipt: packed.payment_receipt,
            pick_ticket: packed.pick_ticket,
            tracking_number,
        })
    }
}

#[transition]
impl OrderMachine<Shipped> {
    fn deliver(self) -> OrderMachine<Delivered> {
        self.transition_map(|shipped| DeliveredOrder {
            order: shipped.order,
            payment_receipt: shipped.payment_receipt,
            pick_ticket: shipped.pick_ticket,
            tracking_number: shipped.tracking_number,
        })
    }
}

#[derive(Clone, Debug, FromRow)]
struct EventRow {
    event_id: i64,
    order_id: i64,
    event_type: String,
    customer: Option<String>,
    sku: Option<String>,
    value: Option<String>,
}

#[derive(Clone, Debug)]
struct OrderProjectionRow {
    order_id: i64,
    customer: String,
    sku: String,
    status: String,
    payment_receipt: Option<String>,
    pick_ticket: Option<String>,
    tracking_number: Option<String>,
}

struct OrderProjector;

#[validators(OrderMachine)]
impl OrderProjectionRow {
    fn is_created(&self) -> statum::Result<CreatedOrder> {
        if self.order_id <= 0
            || self.customer.is_empty()
            || self.sku.is_empty()
            || self.status != EVENT_CREATED
            || self.payment_receipt.is_some()
            || self.pick_ticket.is_some()
            || self.tracking_number.is_some()
        {
            return Err(statum::Error::InvalidState);
        }

        Ok(CreatedOrder {
            order: OrderContext {
                order_id: self.order_id,
                customer: self.customer.clone(),
                sku: self.sku.clone(),
            },
        })
    }

    fn is_paid(&self) -> statum::Result<PaidOrder> {
        if self.order_id <= 0
            || self.customer.is_empty()
            || self.sku.is_empty()
            || self.status != EVENT_PAID
            || self.pick_ticket.is_some()
            || self.tracking_number.is_some()
        {
            return Err(statum::Error::InvalidState);
        }

        self.payment_receipt
            .clone()
            .filter(|receipt| !receipt.trim().is_empty())
            .map(|payment_receipt| PaidOrder {
                order: OrderContext {
                    order_id: self.order_id,
                    customer: self.customer.clone(),
                    sku: self.sku.clone(),
                },
                payment_receipt,
            })
            .ok_or(statum::Error::InvalidState)
    }

    fn is_packed(&self) -> statum::Result<PackedOrder> {
        if self.order_id <= 0
            || self.customer.is_empty()
            || self.sku.is_empty()
            || self.status != EVENT_PACKED
            || self.tracking_number.is_some()
        {
            return Err(statum::Error::InvalidState);
        }

        match (&self.payment_receipt, &self.pick_ticket) {
            (Some(payment_receipt), Some(pick_ticket))
                if !payment_receipt.trim().is_empty() && !pick_ticket.trim().is_empty() =>
            {
                Ok(PackedOrder {
                    order: OrderContext {
                        order_id: self.order_id,
                        customer: self.customer.clone(),
                        sku: self.sku.clone(),
                    },
                    payment_receipt: payment_receipt.clone(),
                    pick_ticket: pick_ticket.clone(),
                })
            }
            _ => Err(statum::Error::InvalidState),
        }
    }

    fn is_shipped(&self) -> statum::Result<ShippedOrder> {
        if self.order_id <= 0
            || self.customer.is_empty()
            || self.sku.is_empty()
            || self.status != EVENT_SHIPPED
        {
            return Err(statum::Error::InvalidState);
        }

        match (
            &self.payment_receipt,
            &self.pick_ticket,
            &self.tracking_number,
        ) {
            (Some(payment_receipt), Some(pick_ticket), Some(tracking_number))
                if !payment_receipt.trim().is_empty()
                    && !pick_ticket.trim().is_empty()
                    && !tracking_number.trim().is_empty() =>
            {
                Ok(ShippedOrder {
                    order: OrderContext {
                        order_id: self.order_id,
                        customer: self.customer.clone(),
                        sku: self.sku.clone(),
                    },
                    payment_receipt: payment_receipt.clone(),
                    pick_ticket: pick_ticket.clone(),
                    tracking_number: tracking_number.clone(),
                })
            }
            _ => Err(statum::Error::InvalidState),
        }
    }

    fn is_delivered(&self) -> statum::Result<DeliveredOrder> {
        if self.order_id <= 0
            || self.customer.is_empty()
            || self.sku.is_empty()
            || self.status != EVENT_DELIVERED
        {
            return Err(statum::Error::InvalidState);
        }

        match (
            &self.payment_receipt,
            &self.pick_ticket,
            &self.tracking_number,
        ) {
            (Some(payment_receipt), Some(pick_ticket), Some(tracking_number))
                if !payment_receipt.trim().is_empty()
                    && !pick_ticket.trim().is_empty()
                    && !tracking_number.trim().is_empty() =>
            {
                Ok(DeliveredOrder {
                    order: OrderContext {
                        order_id: self.order_id,
                        customer: self.customer.clone(),
                        sku: self.sku.clone(),
                    },
                    payment_receipt: payment_receipt.clone(),
                    pick_ticket: pick_ticket.clone(),
                    tracking_number: tracking_number.clone(),
                })
            }
            _ => Err(statum::Error::InvalidState),
        }
    }
}

pub struct OrderEventStore {
    pool: SqlitePool,
}

#[derive(Debug)]
pub enum RebuildError {
    Database(sqlx::Error),
    InvalidInput(&'static str),
    InvalidTransition(&'static str),
    CorruptEventLog(&'static str),
    NotFound(i64),
}

impl core::fmt::Display for RebuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Database(error) => write!(f, "{error}"),
            Self::InvalidInput(message) => write!(f, "{message}"),
            Self::InvalidTransition(message) => write!(f, "{message}"),
            Self::CorruptEventLog(message) => write!(f, "{message}"),
            Self::NotFound(order_id) => write!(f, "order {order_id} not found"),
        }
    }
}

impl std::error::Error for RebuildError {}

impl From<sqlx::Error> for RebuildError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

pub async fn build_store() -> Result<OrderEventStore, RebuildError> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;

    let store = OrderEventStore { pool };
    store.init_schema().await?;
    Ok(store)
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let store = build_store().await?;

    let delivered = store.create_order("acme", "widget").await?;
    store.pay(delivered, "pay-001").await?;
    store.pack(delivered, "pick-001").await?;
    store.ship(delivered, "trk-001").await?;
    store.deliver(delivered).await?;

    let packed = store.create_order("globex", "gizmo").await?;
    store.pay(packed, "pay-002").await?;
    store.pack(packed, "pick-002").await?;

    for state in store.load_all_states().await? {
        println!("{}", format_state(&state));
    }

    Ok(())
}

impl OrderEventStore {
    async fn init_schema(&self) -> Result<(), RebuildError> {
        sqlx::query(
            r#"
            CREATE TABLE order_events (
                event_id INTEGER PRIMARY KEY AUTOINCREMENT,
                order_id INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                customer TEXT,
                sku TEXT,
                value TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn create_order(&self, customer: &str, sku: &str) -> Result<i64, RebuildError> {
        ensure_non_empty(customer, "customer is required")?;
        ensure_non_empty(sku, "sku is required")?;

        let order_id = self.next_order_id().await?;
        self.append_event(order_id, EVENT_CREATED, Some(customer), Some(sku), None)
            .await?;
        Ok(order_id)
    }

    pub async fn pay(&self, order_id: i64, payment_receipt: &str) -> Result<(), RebuildError> {
        ensure_non_empty(payment_receipt, "payment_receipt is required")?;

        match self.load_state(order_id).await? {
            order_machine::State::Created(machine) => {
                let machine = machine.pay(payment_receipt.to_string());
                self.append_event(
                    machine.state_data.order.order_id,
                    EVENT_PAID,
                    None,
                    None,
                    Some(&machine.state_data.payment_receipt),
                )
                .await
            }
            _ => Err(RebuildError::InvalidTransition(
                "pay requires a created order",
            )),
        }
    }

    pub async fn pack(&self, order_id: i64, pick_ticket: &str) -> Result<(), RebuildError> {
        ensure_non_empty(pick_ticket, "pick_ticket is required")?;

        match self.load_state(order_id).await? {
            order_machine::State::Paid(machine) => {
                let machine = machine.pack(pick_ticket.to_string());
                self.append_event(
                    machine.state_data.order.order_id,
                    EVENT_PACKED,
                    None,
                    None,
                    Some(&machine.state_data.pick_ticket),
                )
                .await
            }
            _ => Err(RebuildError::InvalidTransition(
                "pack requires a paid order",
            )),
        }
    }

    pub async fn ship(&self, order_id: i64, tracking_number: &str) -> Result<(), RebuildError> {
        ensure_non_empty(tracking_number, "tracking_number is required")?;

        match self.load_state(order_id).await? {
            order_machine::State::Packed(machine) => {
                let machine = machine.ship(tracking_number.to_string());
                self.append_event(
                    machine.state_data.order.order_id,
                    EVENT_SHIPPED,
                    None,
                    None,
                    Some(&machine.state_data.tracking_number),
                )
                .await
            }
            _ => Err(RebuildError::InvalidTransition(
                "ship requires a packed order",
            )),
        }
    }

    pub async fn deliver(&self, order_id: i64) -> Result<(), RebuildError> {
        match self.load_state(order_id).await? {
            order_machine::State::Shipped(machine) => {
                let machine = machine.deliver();
                self.append_event(
                    machine.state_data.order.order_id,
                    EVENT_DELIVERED,
                    None,
                    None,
                    None,
                )
                .await
            }
            _ => Err(RebuildError::InvalidTransition(
                "deliver requires a shipped order",
            )),
        }
    }

    pub async fn load_state(&self, order_id: i64) -> Result<order_machine::State, RebuildError> {
        let events = self.events_for(order_id).await?;
        let row = reduce_one(events, &OrderProjector)
            .map_err(|error| map_projection_error(error, Some(order_id)))?;
        row.into_machine().build().map_err(|_| {
            RebuildError::CorruptEventLog("projected order row did not match any validator")
        })
    }

    pub async fn load_all_states(&self) -> Result<Vec<order_machine::State>, RebuildError> {
        let rows = self.project_all_orders().await?;
        rows.into_machines()
            .build()
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| {
                RebuildError::CorruptEventLog("projected order row did not match any validator")
            })
    }

    async fn next_order_id(&self) -> Result<i64, RebuildError> {
        let next =
            sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(order_id), 0) + 1 FROM order_events")
                .fetch_one(&self.pool)
                .await?;
        Ok(next)
    }

    async fn append_event(
        &self,
        order_id: i64,
        event_type: &str,
        customer: Option<&str>,
        sku: Option<&str>,
        value: Option<&str>,
    ) -> Result<(), RebuildError> {
        sqlx::query(
            r#"
            INSERT INTO order_events (order_id, event_type, customer, sku, value)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(order_id)
        .bind(event_type)
        .bind(customer)
        .bind(sku)
        .bind(value)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn events_for(&self, order_id: i64) -> Result<Vec<EventRow>, RebuildError> {
        sqlx::query_as::<_, EventRow>(
            r#"
            SELECT event_id, order_id, event_type, customer, sku, value
            FROM order_events
            WHERE order_id = ?
            ORDER BY event_id ASC
            "#,
        )
        .bind(order_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    async fn project_all_orders(&self) -> Result<Vec<OrderProjectionRow>, RebuildError> {
        let events = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT event_id, order_id, event_type, customer, sku, value
            FROM order_events
            ORDER BY event_id ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        reduce_grouped(events, |event| event.order_id, &OrderProjector)
            .map_err(|error| map_projection_error(error, None))
    }
}

fn ensure_non_empty(value: &str, message: &'static str) -> Result<(), RebuildError> {
    if value.trim().is_empty() {
        Err(RebuildError::InvalidInput(message))
    } else {
        Ok(())
    }
}

fn required_created_value(value: &Option<String>) -> Result<String, RebuildError> {
    value
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or(RebuildError::CorruptEventLog(
            "created event was missing required data",
        ))
}

fn required_event_value(event: &EventRow) -> Result<String, RebuildError> {
    event
        .value
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or(RebuildError::CorruptEventLog(
            "order event was missing required payload data",
        ))
}

fn map_projection_error(
    error: ProjectionError<RebuildError>,
    order_id: Option<i64>,
) -> RebuildError {
    match error {
        ProjectionError::EmptyInput => order_id.map_or(
            RebuildError::CorruptEventLog("order event stream was empty"),
            RebuildError::NotFound,
        ),
        ProjectionError::Reducer(error) => error,
    }
}

impl ProjectionReducer<EventRow> for OrderProjector {
    type Projection = OrderProjectionRow;
    type Error = RebuildError;

    fn seed(&self, event: &EventRow) -> Result<Self::Projection, Self::Error> {
        if event.event_type != EVENT_CREATED {
            return Err(RebuildError::CorruptEventLog(
                "order event stream must start with a created event",
            ));
        }

        Ok(OrderProjectionRow {
            order_id: event.order_id,
            customer: required_created_value(&event.customer)?,
            sku: required_created_value(&event.sku)?,
            status: EVENT_CREATED.to_string(),
            payment_receipt: None,
            pick_ticket: None,
            tracking_number: None,
        })
    }

    fn apply(
        &self,
        projection: &mut Self::Projection,
        event: &EventRow,
    ) -> Result<(), Self::Error> {
        match event.event_type.as_str() {
            EVENT_CREATED => Err(RebuildError::CorruptEventLog(
                "order event stream contained multiple created events",
            )),
            EVENT_PAID => {
                projection.status = EVENT_PAID.to_string();
                projection.payment_receipt = Some(required_event_value(event)?);
                Ok(())
            }
            EVENT_PACKED => {
                projection.status = EVENT_PACKED.to_string();
                projection.pick_ticket = Some(required_event_value(event)?);
                Ok(())
            }
            EVENT_SHIPPED => {
                projection.status = EVENT_SHIPPED.to_string();
                projection.tracking_number = Some(required_event_value(event)?);
                Ok(())
            }
            EVENT_DELIVERED => {
                projection.status = EVENT_DELIVERED.to_string();
                Ok(())
            }
            _ => Err(RebuildError::CorruptEventLog(
                "order event stream contained an unknown event type",
            )),
        }
    }
}

fn format_state(state: &order_machine::State) -> String {
    match state {
        order_machine::State::Created(machine) => format!(
            "order={} customer={} sku={} state=created",
            machine.state_data.order.order_id,
            machine.state_data.order.customer,
            machine.state_data.order.sku
        ),
        order_machine::State::Paid(machine) => format!(
            "order={} customer={} sku={} state=paid payment_receipt={}",
            machine.state_data.order.order_id,
            machine.state_data.order.customer,
            machine.state_data.order.sku,
            machine.state_data.payment_receipt
        ),
        order_machine::State::Packed(machine) => format!(
            "order={} customer={} sku={} state=packed payment_receipt={} pick_ticket={}",
            machine.state_data.order.order_id,
            machine.state_data.order.customer,
            machine.state_data.order.sku,
            machine.state_data.payment_receipt,
            machine.state_data.pick_ticket
        ),
        order_machine::State::Shipped(machine) => format!(
            "order={} customer={} sku={} state=shipped payment_receipt={} pick_ticket={} tracking_number={}",
            machine.state_data.order.order_id,
            machine.state_data.order.customer,
            machine.state_data.order.sku,
            machine.state_data.payment_receipt,
            machine.state_data.pick_ticket,
            machine.state_data.tracking_number
        ),
        order_machine::State::Delivered(machine) => format!(
            "order={} customer={} sku={} state=delivered payment_receipt={} pick_ticket={} tracking_number={}",
            machine.state_data.order.order_id,
            machine.state_data.order.customer,
            machine.state_data.order.sku,
            machine.state_data.payment_receipt,
            machine.state_data.pick_ticket,
            machine.state_data.tracking_number
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn projection_rehydrates_into_packed_state() {
        let row = OrderProjectionRow {
            order_id: 7,
            customer: "acme".to_string(),
            sku: "widget".to_string(),
            status: EVENT_PACKED.to_string(),
            payment_receipt: Some("pay-7".to_string()),
            pick_ticket: Some("pick-7".to_string()),
            tracking_number: None,
        };

        let state = row.into_machine().build().unwrap();
        match state {
            order_machine::State::Packed(machine) => {
                assert_eq!(machine.state_data.order.order_id, 7);
                assert_eq!(machine.state_data.pick_ticket.as_str(), "pick-7");
            }
            _ => panic!("expected a packed order"),
        }
    }
}
