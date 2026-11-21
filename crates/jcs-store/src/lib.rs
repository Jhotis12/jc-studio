//! Durable state for JC Studio.
//!
//! One SQLite database (WAL) per daemon, owned exclusively through a lock file.
//! The append-only `events` table is the source of truth; `projects`, `goals`,
//! `tasks` and `runs` are projections written in the same transaction as their event.

pub mod error;
pub mod events;
pub mod goals;
pub mod integration;
pub mod leases;
pub mod plans;
pub mod projects;
pub mod runs;
pub mod store;
pub mod tasks;

pub use error::{Result, StoreError};
pub use events::{count_events, events_for_task, events_since, latest_seq, recent_events};
pub use goals::GoalSettings;
pub use leases::Lease;
pub use plans::TaskCounts;
pub use runs::GateResult;
pub use store::{append_event, Store};
