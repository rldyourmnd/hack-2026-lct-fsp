//! Public GitHub activity analysis without a token.
//!
//! A separate loopback service: it collects public facts about one account through GitHub search,
//! the public REST API and blobless clones of public repositories ([`collect`], [`git`]), turns
//! them into documented heuristics, a developer portrait and scores ([`analysis`], [`profile`],
//! [`tech`]) and serves background analyses to the career API ([`http`]).

pub mod analysis;
pub mod client;
pub mod collect;
pub mod git;
pub mod http;
pub mod profile;
pub mod snapshot;
pub mod tech;

pub use http::{InsightsState, router};
