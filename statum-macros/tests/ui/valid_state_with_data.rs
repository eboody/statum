#![allow(unused_imports)]
extern crate self as statum;
pub use bon;
use statum_macros::{machine, state};
use bon::builder as _;

#[state]
pub enum ReviewState {
    Draft,
    InReview(ReviewData),
    Published,
}

#[derive(Clone, Debug)]
pub struct ReviewData {
    reviewer: String,
}

#[machine]
pub struct Document<ReviewState> {
    id: u64,
}

fn main() {
    let review = ReviewData {
        reviewer: "sam".to_string(),
    };
    let _: Document<InReview> = Document::<InReview>::builder()
        .id(1)
        .state_data(review)
        .build();
    let _: Document<Draft> = Document::<Draft>::builder().id(2).build();
}