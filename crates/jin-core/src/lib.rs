pub mod chat;
pub mod command;
pub mod factory;
pub mod orchestrator;
pub mod policy;
pub mod repository;
pub mod runner;
pub mod store;
pub mod sync;
pub mod task;
pub mod telegram;
pub mod version;

pub fn component_name() -> &'static str {
    "jin-core"
}
