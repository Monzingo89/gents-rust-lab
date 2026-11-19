//! Cache-first local task services, shared by the harness and MCP adapters.
//! No model-produced text is permission, a registered executor, or acceptance.
pub mod contracts;
pub mod grounded;
mod repository;
pub use repository::{KnowledgeSource, LookupBinding, Repository, SearchHit};
