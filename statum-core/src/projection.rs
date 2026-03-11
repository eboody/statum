//! Event-stream projection helpers for Statum rebuild flows.
//!
//! `#[validators]` works on one persisted shape at a time. If your storage model
//! is append-only, reduce events into a row-like projection first and then feed
//! that projection into `into_machine()` or `.into_machines()`.
//!
//! ```
//! use statum_core::projection::{reduce_grouped, ProjectionReducer};
//!
//! #[derive(Clone)]
//! struct OrderEvent {
//!     order_id: u64,
//!     amount_cents: u64,
//! }
//!
//! struct OrderTotals;
//!
//! impl ProjectionReducer<OrderEvent> for OrderTotals {
//!     type Projection = (u64, u64);
//!     type Error = core::convert::Infallible;
//!
//!     fn seed(&self, event: &OrderEvent) -> Result<Self::Projection, Self::Error> {
//!         Ok((event.order_id, event.amount_cents))
//!     }
//!
//!     fn apply(
//!         &self,
//!         projection: &mut Self::Projection,
//!         event: &OrderEvent,
//!     ) -> Result<(), Self::Error> {
//!         projection.1 += event.amount_cents;
//!         Ok(())
//!     }
//! }
//!
//! let projections = reduce_grouped(
//!     vec![
//!         OrderEvent {
//!             order_id: 2,
//!             amount_cents: 100,
//!         },
//!         OrderEvent {
//!             order_id: 1,
//!             amount_cents: 50,
//!         },
//!         OrderEvent {
//!             order_id: 2,
//!             amount_cents: 25,
//!         },
//!     ],
//!     |event| event.order_id,
//!     &OrderTotals,
//! )?;
//!
//! assert_eq!(projections, vec![(2, 125), (1, 50)]);
//! # Ok::<(), statum_core::projection::ProjectionError<core::convert::Infallible>>(())
//! ```

use core::fmt;
use std::collections::{hash_map::Entry, HashMap};
use std::hash::Hash;

/// Fold events into a projection that can later be rehydrated with `#[validators]`.
pub trait ProjectionReducer<Event> {
    /// The output projection type.
    type Projection;
    /// The reducer-specific error type.
    type Error;

    /// Create the first projection value from the first event in a stream.
    fn seed(&self, event: &Event) -> Result<Self::Projection, Self::Error>;

    /// Apply a later event to an existing projection value.
    fn apply(&self, projection: &mut Self::Projection, event: &Event) -> Result<(), Self::Error>;
}

/// Errors returned by projection helpers.
#[derive(Debug, PartialEq, Eq)]
pub enum ProjectionError<E> {
    /// The reducer was asked to fold an empty stream.
    EmptyInput,
    /// The reducer rejected one of the events in the stream.
    Reducer(E),
}

impl<E> fmt::Display for ProjectionError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "projection input was empty"),
            Self::Reducer(error) => write!(f, "{error}"),
        }
    }
}

impl<E> std::error::Error for ProjectionError<E> where E: std::error::Error + 'static {}

/// Reduce one ordered event stream into one projection value.
pub fn reduce_one<Event, I, R>(
    events: I,
    reducer: &R,
) -> Result<R::Projection, ProjectionError<R::Error>>
where
    I: IntoIterator<Item = Event>,
    R: ProjectionReducer<Event>,
{
    let mut iter = events.into_iter();
    let first = iter.next().ok_or(ProjectionError::EmptyInput)?;
    let mut projection = reducer.seed(&first).map_err(ProjectionError::Reducer)?;

    for event in iter {
        reducer
            .apply(&mut projection, &event)
            .map_err(ProjectionError::Reducer)?;
    }

    Ok(projection)
}

/// Reduce many ordered event streams into projection values grouped by key.
///
/// Events are applied in encounter order for each key, and the output preserves
/// the first-seen order of keys in the input stream.
pub fn reduce_grouped<Event, I, K, KF, R>(
    events: I,
    key_fn: KF,
    reducer: &R,
) -> Result<Vec<R::Projection>, ProjectionError<R::Error>>
where
    I: IntoIterator<Item = Event>,
    KF: Fn(&Event) -> K,
    K: Eq + Hash + Clone,
    R: ProjectionReducer<Event>,
{
    let mut order = Vec::new();
    let mut projections = HashMap::new();

    for event in events {
        let key = key_fn(&event);
        match projections.entry(key.clone()) {
            Entry::Occupied(mut entry) => {
                reducer
                    .apply(entry.get_mut(), &event)
                    .map_err(ProjectionError::Reducer)?;
            }
            Entry::Vacant(entry) => {
                order.push(key);
                let projection = reducer.seed(&event).map_err(ProjectionError::Reducer)?;
                entry.insert(projection);
            }
        }
    }

    let mut results = Vec::with_capacity(order.len());
    for key in order {
        if let Some(projection) = projections.remove(&key) {
            results.push(projection);
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Event {
        stream: &'static str,
        value: i32,
    }

    struct SumReducer;

    impl ProjectionReducer<Event> for SumReducer {
        type Projection = i32;
        type Error = &'static str;

        fn seed(&self, event: &Event) -> Result<Self::Projection, Self::Error> {
            if event.value < 0 {
                Err("negative seed")
            } else {
                Ok(event.value)
            }
        }

        fn apply(
            &self,
            projection: &mut Self::Projection,
            event: &Event,
        ) -> Result<(), Self::Error> {
            if event.value < 0 {
                return Err("negative apply");
            }

            *projection += event.value;
            Ok(())
        }
    }

    #[test]
    fn reduce_one_requires_input() {
        let result = reduce_one(Vec::<Event>::new(), &SumReducer);
        assert_eq!(result, Err(ProjectionError::EmptyInput));
    }

    #[test]
    fn reduce_one_folds_one_stream() {
        let result = reduce_one(
            vec![
                Event {
                    stream: "a",
                    value: 1,
                },
                Event {
                    stream: "a",
                    value: 2,
                },
            ],
            &SumReducer,
        )
        .unwrap();

        assert_eq!(result, 3);
    }

    #[test]
    fn reduce_grouped_preserves_first_seen_order() {
        let result = reduce_grouped(
            vec![
                Event {
                    stream: "b",
                    value: 1,
                },
                Event {
                    stream: "a",
                    value: 2,
                },
                Event {
                    stream: "b",
                    value: 3,
                },
            ],
            |event| event.stream,
            &SumReducer,
        )
        .unwrap();

        assert_eq!(result, vec![4, 2]);
    }

    #[test]
    fn reduce_grouped_propagates_reducer_errors() {
        let result = reduce_grouped(
            vec![
                Event {
                    stream: "a",
                    value: 1,
                },
                Event {
                    stream: "a",
                    value: -1,
                },
            ],
            |event| event.stream,
            &SumReducer,
        );

        assert_eq!(result, Err(ProjectionError::Reducer("negative apply")));
    }
}
