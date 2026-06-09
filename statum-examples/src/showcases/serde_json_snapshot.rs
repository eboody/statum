use serde::{Deserialize, Serialize};
use statum::{machine, state, transition, validators};
use std::{collections::HashMap, error::Error, fmt};

const STATUS_OPEN: &str = "open";
const STATUS_CHECKED_OUT: &str = "checked_out";

#[state]
pub enum CartState {
    Open(OpenCart),
    CheckedOut(CheckedOutCart),
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Serialize)]
pub struct CartId(pub i64);

impl fmt::Display for CartId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReceiptId(pub String);

impl ReceiptId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::ops::Deref for ReceiptId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenCart {
    pub items: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckedOutCart {
    pub items: Vec<String>,
    pub receipt_id: ReceiptId,
}

#[machine]
pub struct CartMachine<CartState> {
    pub cart_id: CartId,
    pub owner: String,
}

#[transition]
impl CartMachine<Open> {
    fn checkout(self, receipt_id: ReceiptId) -> CartMachine<CheckedOut> {
        self.transition_map(|open| CheckedOutCart {
            items: open.items,
            receipt_id,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CartSnapshot {
    pub cart_id: i64,
    pub owner: String,
    pub status: String,
    pub items: Vec<String>,
    pub receipt_id: Option<String>,
}

#[validators(CartMachine)]
impl CartSnapshot {
    fn is_open(&self) -> statum::Result<OpenCart> {
        if self.cart_id <= 0
            || self.owner.trim().is_empty()
            || self.status != STATUS_OPEN
            || self.receipt_id.is_some()
        {
            return Err(statum::Error::InvalidState);
        }

        Ok(OpenCart {
            items: self.items.clone(),
        })
    }

    fn is_checked_out(&self) -> statum::Result<CheckedOutCart> {
        if self.cart_id <= 0 || self.owner.trim().is_empty() || self.status != STATUS_CHECKED_OUT {
            return Err(statum::Error::InvalidState);
        }

        self.receipt_id
            .clone()
            .filter(|receipt_id| !receipt_id.trim().is_empty())
            .map(|receipt_id| CheckedOutCart {
                items: self.items.clone(),
                receipt_id: ReceiptId(receipt_id),
            })
            .ok_or(statum::Error::InvalidState)
    }
}

#[derive(Debug, Default)]
pub struct JsonSnapshotStore {
    rows: HashMap<CartId, String>,
    next_id: i64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CartResponse {
    pub cart_id: CartId,
    pub owner: String,
    pub status: String,
    pub items: Vec<String>,
    pub receipt_id: Option<ReceiptId>,
}

#[derive(Debug)]
pub enum StoreError {
    NotFound,
    InvalidInput(&'static str),
    InvalidTransition(&'static str),
    CorruptSnapshot,
    Json(serde_json::Error),
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("cart not found"),
            Self::InvalidInput(message) | Self::InvalidTransition(message) => {
                formatter.write_str(message)
            }
            Self::CorruptSnapshot => {
                formatter.write_str("stored cart snapshot did not match any validator")
            }
            Self::Json(error) => write!(formatter, "json snapshot error: {error}"),
        }
    }
}

impl Error for StoreError {}

impl From<serde_json::Error> for StoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl JsonSnapshotStore {
    pub fn create_cart(
        &mut self,
        owner: &str,
        items: Vec<String>,
    ) -> Result<CartResponse, StoreError> {
        if owner.trim().is_empty() {
            return Err(StoreError::InvalidInput("owner is required"));
        }
        if items.iter().any(|item| item.trim().is_empty()) {
            return Err(StoreError::InvalidInput("cart items must not be empty"));
        }

        let cart_id = self.allocate_id();
        let snapshot = CartSnapshot {
            cart_id: cart_id.0,
            owner: owner.to_owned(),
            status: STATUS_OPEN.to_owned(),
            items,
            receipt_id: None,
        };

        self.persist_snapshot(&snapshot)?;
        Ok(snapshot.into())
    }

    pub fn fetch_snapshot(&self, cart_id: CartId) -> Result<CartSnapshot, StoreError> {
        let json = self.rows.get(&cart_id).ok_or(StoreError::NotFound)?;
        Ok(serde_json::from_str(json)?)
    }

    pub fn load_cart_state(&self, cart_id: CartId) -> Result<cart_machine::SomeState, StoreError> {
        let snapshot = self.fetch_snapshot(cart_id)?;
        rebuild_cart_snapshot(&snapshot)
            .into_result()
            .map_err(|_| StoreError::CorruptSnapshot)
    }

    pub fn checkout(
        &mut self,
        cart_id: CartId,
        receipt_id: &str,
    ) -> Result<CartResponse, StoreError> {
        if receipt_id.trim().is_empty() {
            return Err(StoreError::InvalidInput("receipt_id is required"));
        }

        let machine = match self.load_cart_state(cart_id)? {
            cart_machine::SomeState::Open(machine) => {
                machine.checkout(ReceiptId(receipt_id.to_owned()))
            }
            _ => {
                return Err(StoreError::InvalidTransition(
                    "checkout requires an open cart",
                ));
            }
        };

        let snapshot = CartSnapshot::from_checked_out(&machine);
        self.persist_snapshot(&snapshot)?;
        Ok(snapshot.into())
    }

    fn allocate_id(&mut self) -> CartId {
        self.next_id += 1;
        CartId(self.next_id)
    }

    fn persist_snapshot(&mut self, snapshot: &CartSnapshot) -> Result<(), StoreError> {
        let json = serde_json::to_string(snapshot)?;
        self.rows.insert(CartId(snapshot.cart_id), json);
        Ok(())
    }
}

fn rebuild_cart_snapshot(
    snapshot: &CartSnapshot,
) -> statum::RebuildReport<cart_machine::SomeState> {
    CartMachine::rebuild(snapshot)
        .cart_id(CartId(snapshot.cart_id))
        .owner(snapshot.owner.clone())
        .build_report()
}

impl CartSnapshot {
    fn from_checked_out(machine: &CartMachine<CheckedOut>) -> Self {
        Self {
            cart_id: machine.cart_id.0,
            owner: machine.owner.clone(),
            status: STATUS_CHECKED_OUT.to_owned(),
            items: machine.state_data.items.clone(),
            receipt_id: Some(machine.state_data.receipt_id.0.clone()),
        }
    }
}

impl From<CartSnapshot> for CartResponse {
    fn from(snapshot: CartSnapshot) -> Self {
        Self {
            cart_id: CartId(snapshot.cart_id),
            owner: snapshot.owner,
            status: snapshot.status,
            items: snapshot.items,
            receipt_id: snapshot.receipt_id.map(ReceiptId),
        }
    }
}
