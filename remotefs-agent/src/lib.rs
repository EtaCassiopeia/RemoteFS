//! RemoteFS Agent Library
//! 
//! This crate provides the agent functionality for the RemoteFS system,
//! allowing secure access to local file systems through a relay server.

pub mod access;
pub mod filesystem;
pub mod connection;
pub mod server;
pub mod config_utils;

// Re-export commonly used types
pub use access::AccessControl;
pub use filesystem::FilesystemHandler;
pub use server::AgentServer;
pub use config_utils::{create_default_agent_config, load_config_from_file, save_config_to_file};

use remotefs_common::{config::AgentConfig, error::Result};

/// Initialize the agent with the given configuration
pub async fn init_agent(config: AgentConfig) -> Result<AgentServer> {
    AgentServer::new(config)
}
