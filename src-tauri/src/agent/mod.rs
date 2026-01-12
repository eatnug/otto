// Legacy agent modules (keeping for now)
pub mod decomposer;
pub mod observer;
pub mod orchestrator;
pub mod prompts;
pub mod thinker;
pub mod verifier;

// New tool-based agent
pub mod runner;
pub mod tools;

pub use orchestrator::AgentOrchestrator;
pub use runner::Agent;
