// Node Manager for multi-node testing
// Manages starting, stopping, and monitoring LuxTensor nodes

use std::process::{Child, Command, Stdio};
use std::path::PathBuf;
use std::time::Duration;
use std::fs;

/// Node configuration
pub struct NodeConfig {
    pub name: String,
    pub data_dir: PathBuf,
    pub rpc_port: u16,
    pub p2p_port: u16,
    pub config_file: PathBuf,
}

/// Managed node instance
pub struct ManagedNode {
    pub config: NodeConfig,
    process: Option<Child>,
}

impl NodeConfig {
    /// Create config for Node 1
    pub fn node1() -> Self {
        Self {
            name: "node-1".to_string(),
            data_dir: PathBuf::from("node1/data"),
            rpc_port: 8545,
            p2p_port: 30303,
            config_file: PathBuf::from("node1/config.toml"),
        }
    }

    /// Create config for Node 2
    pub fn node2() -> Self {
        Self {
            name: "node-2".to_string(),
            data_dir: PathBuf::from("node2/data"),
            rpc_port: 8555,
            p2p_port: 30304,
            config_file: PathBuf::from("node2/config.toml"),
        }
    }

    /// Create config for Node 3
    pub fn node3() -> Self {
        Self {
            name: "node-3".to_string(),
            data_dir: PathBuf::from("node3/data"),
            rpc_port: 8565,
            p2p_port: 30305,
            config_file: PathBuf::from("node3/config.toml"),
        }
    }
}

impl ManagedNode {
    /// Create a new managed node (not started)
    pub fn new(config: NodeConfig) -> Self {
        Self {
            config,
            process: None,
        }
    }

    /// Clean data directory
    pub fn clean_data(&self) -> std::io::Result<()> {
        if self.config.data_dir.exists() {
            fs::remove_dir_all(&self.config.data_dir)?;
        }
        Ok(())
    }

    /// Start the node
    pub fn start(&mut self, binary_path: &str) -> std::io::Result<()> {
        if self.process.is_some() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Node already running",
            ));
        }

        let default_dir = PathBuf::from(".");
        let working_dir = self.config.config_file.parent()
            .unwrap_or(&default_dir);

        let child = Command::new(binary_path)
            .arg("--config")
            .arg(self.config.config_file.file_name().unwrap_or_default())
            .current_dir(working_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        self.process = Some(child);
        Ok(())
    }

    /// Stop the node
    pub fn stop(&mut self) -> std::io::Result<()> {
        if let Some(mut child) = self.process.take() {
            child.kill()?;
            child.wait()?;
        }
        Ok(())
    }

    /// Check if node process is running
    pub fn is_running(&mut self) -> bool {
        if let Some(ref mut child) = self.process {
            matches!(child.try_wait(), Ok(None))
        } else {
            false
        }
    }

    /// Get RPC URL
    pub fn rpc_url(&self) -> String {
        format!("http://localhost:{}", self.config.rpc_port)
    }
}

impl Drop for ManagedNode {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Test network manager for multi-node tests
pub struct TestNetwork {
    pub nodes: Vec<ManagedNode>,
    binary_path: String,
}

impl TestNetwork {
    /// Create a new test network
    pub fn new(binary_path: &str) -> Self {
        Self {
            nodes: Vec::new(),
            binary_path: binary_path.to_string(),
        }
    }

    /// Create default 2-node network
    pub fn two_nodes(binary_path: &str) -> Self {
        let mut network = Self::new(binary_path);
        network.nodes.push(ManagedNode::new(NodeConfig::node1()));
        network.nodes.push(ManagedNode::new(NodeConfig::node2()));
        network
    }

    /// Create default 3-node network
    pub fn three_nodes(binary_path: &str) -> Self {
        let mut network = Self::new(binary_path);
        network.nodes.push(ManagedNode::new(NodeConfig::node1()));
        network.nodes.push(ManagedNode::new(NodeConfig::node2()));
        network.nodes.push(ManagedNode::new(NodeConfig::node3()));
        network
    }

    /// Clean all data directories
    pub fn clean_all(&self) -> std::io::Result<()> {
        for node in &self.nodes {
            node.clean_data()?;
        }
        Ok(())
    }

    /// Start all nodes with delay between each
    pub fn start_all(&mut self, delay_ms: u64) -> std::io::Result<()> {
        for node in self.nodes.iter_mut() {
            node.start(&self.binary_path)?;
            std::thread::sleep(Duration::from_millis(delay_ms));
        }
        Ok(())
    }

    /// Stop all nodes
    pub fn stop_all(&mut self) -> std::io::Result<()> {
        for node in self.nodes.iter_mut() {
            node.stop()?;
        }
        Ok(())
    }

    /// Wait for all nodes to be ready
    pub fn wait_for_ready(&self, timeout_secs: u64) -> bool {
        use crate::rpc_client::RpcClient;

        for node in &self.nodes {
            let client = RpcClient::new(&node.rpc_url());
            if !client.wait_for_ready(timeout_secs) {
                return false;
            }
        }
        true
    }
}

impl Drop for TestNetwork {
    fn drop(&mut self) {
        let _ = self.stop_all();
    }
}

/// Kill all running luxtensor-node processes
pub fn kill_all_nodes() {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "luxtensor-node.exe"])
            .output();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("pkill")
            .arg("luxtensor-node")
            .output();
    }
}

/// Get default binary path
pub fn default_binary_path() -> String {
    #[cfg(windows)]
    return String::from("./target/release/luxtensor-node.exe");
    #[cfg(not(windows))]
    return String::from("./target/release/luxtensor-node");
}
