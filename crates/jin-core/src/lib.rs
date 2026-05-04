pub mod chat;
pub mod command;
pub mod orchestrator;
pub mod policy;
pub mod repository;
pub mod runner;
pub mod store;
pub mod task;
pub mod telegram;
pub mod version;

pub fn component_name() -> &'static str {
    "jin-core"
}
