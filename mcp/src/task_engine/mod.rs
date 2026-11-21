//! Cache-first local task services, shared by the harness and MCP adapters.
//! No model-produced text is permission, a registered executor, or acceptance.
pub mod adaptive;
pub mod contracts;
pub mod grounded;
pub mod intake;
pub mod models;
pub mod native;
pub mod optimization;
pub mod progress;
pub mod project_metadata;
pub mod read_tools;
mod repository;
pub mod resources;
pub mod routing;
pub mod schemas;
pub mod semantic;
pub mod validators;
pub mod workflows;
mod workspace_evidence;
pub use repository::{KnowledgeSource, LookupBinding, Repository, SearchHit};
