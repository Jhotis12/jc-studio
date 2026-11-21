//! Domain model for JC Studio.
//!
//! This crate is deliberately pure: no I/O, no async, no database, no clock beyond
//! explicitly-passed timestamps. Everything here is deterministic and testable in
//! isolation. Persistence lives in `jcs-store`; orchestration in `jcs-daemon`.

pub mod boundaries;
pub mod error;
pub mod event;
pub mod ids;
pub mod model;
pub mod plan;
pub mod status;

pub use boundaries::BoundaryRule;
pub use error::CoreError;
pub use event::{Event, EventKind, NewEvent, EVENT_SCHEMA_VERSION};
pub use ids::{EventId, GoalId, ProjectId, RunId, TaskId};
pub use model::{Goal, Project, Run, Task};
pub use plan::{Plan, PlanError, TaskRole, TaskSpec, MAX_PLAN_TASKS};
pub use status::{GateStatus, GoalStatus, RunStatus, StatusMachine, TaskStatus};
