use crate::clab::model::ContainerInspectInfo;
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub struct ClabClient {
    clab_bin: PathBuf,
    is_root: bool,
    sudo_available: bool,
}

impl Default for ClabClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ClabClient {
    pub fn new() -> Self {
        let is_root = Self::check_is_root();
        let sudo_available = if !is_root {
            Self::check_sudo_available()
        } else {
            false
        };

        // Try to find containerlab binary
        let clab_bin = Self::find_binary("containerlab")
            .or_else(|| Self::find_binary("clab"))
            .unwrap_or_else(|| PathBuf::from("containerlab"));

        Self {
            clab_bin,
            is_root,
            sudo_available,
        }
    }

    fn find_binary(name: &str) -> Option<PathBuf> {
        if let Ok(path) = std::env::var("PATH") {
            for dir in path.split(':') {
                if dir.is_empty() || dir == "." {
                    continue;
                }
                let dir_path = Path::new(dir);
                if !dir_path.is_absolute() {
                    continue;
                }
                let bin = dir_path.join(name);
                if bin.is_file() {
                    return Some(bin);
                }
            }
        }
        None
    }

    pub fn check_is_root() -> bool {
        if let Ok(output) = std::process::Command::new("id").arg("-u").output() {
            if let Ok(uid_str) = String::from_utf8(output.stdout) {
                return uid_str.trim() == "0";
            }
        }
        false
    }

    pub fn check_sudo_available() -> bool {
        std::process::Command::new("sudo")
            .arg("-n")
            .arg("true")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn is_privileged(&self) -> bool {
        self.is_root || self.sudo_available
    }

    pub fn privilege_status_desc(&self) -> &'static str {
        if self.is_root {
            "root"
        } else if self.sudo_available {
            "sudo (passwordless)"
        } else {
            "unprivileged"
        }
    }

    /// Build a command with sudo if needed
    fn build_cmd(&self) -> Command {
        if !self.is_root && self.sudo_available {
            let mut cmd = Command::new("sudo");
            cmd.arg("-n");
            cmd.arg(&self.clab_bin);
            cmd
        } else {
            Command::new(&self.clab_bin)
        }
    }

    /// Deploy a lab topology
    pub async fn deploy(
        &self,
        topo_file: &Path,
        reconfigure: bool,
        cleanup: bool,
        log_tx: UnboundedSender<String>,
    ) -> Result<ExitStatus> {
        let abs_path = if topo_file.is_absolute() {
            topo_file.to_path_buf()
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.join(topo_file)
        } else {
            topo_file.to_path_buf()
        };

        if !self.is_privileged() {
            let _ = log_tx.send(
                "WARN: Operating unprivileged (neither root nor passwordless sudo). Deployment may fail."
                    .to_string(),
            );
        }

        let mut cmd = self.build_cmd();
        cmd.arg("deploy");
        cmd.arg("-t");
        cmd.arg(&abs_path);
        if cleanup {
            cmd.arg("-c");
        } else if reconfigure {
            cmd.arg("--reconfigure");
        }

        self.run_streaming(cmd, log_tx).await
    }

    /// Destroy a lab topology
    pub async fn destroy(
        &self,
        topo_file: &Path,
        cleanup: bool,
        log_tx: UnboundedSender<String>,
    ) -> Result<ExitStatus> {
        let abs_path = if topo_file.is_absolute() {
            topo_file.to_path_buf()
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.join(topo_file)
        } else {
            topo_file.to_path_buf()
        };

        if !abs_path.exists() {
            let msg = format!("Topology file '{}' does not exist", abs_path.display());
            let _ = log_tx.send(format!("ERROR: {}", msg));
            anyhow::bail!(msg);
        }

        if !self.is_privileged() {
            let _ = log_tx.send(
                "WARN: Operating unprivileged (neither root nor passwordless sudo). Teardown or cleanup may fail."
                    .to_string(),
            );
        }

        let mut cmd = self.build_cmd();
        cmd.arg("destroy");
        cmd.arg("-t");
        cmd.arg(&abs_path);
        if cleanup {
            cmd.arg("--cleanup");
        }

        self.run_streaming(cmd, log_tx).await
    }

    /// Run an arbitrary containerlab command with streaming output
    pub async fn run_custom_command(
        &self,
        cmd_args: &[String],
        log_tx: UnboundedSender<String>,
    ) -> Result<ExitStatus> {
        let is_deploy = cmd_args.first().map(|s| s.as_str()) == Some("deploy");
        let is_destroy = cmd_args.first().map(|s| s.as_str()) == Some("destroy");

        if (is_deploy || is_destroy) && !self.is_privileged() {
            let _ = log_tx.send(
                "WARN: Operating unprivileged (neither root nor passwordless sudo). Teardown, deployment, or cleanup may fail."
                    .to_string(),
            );
        }

        let mut cmd = self.build_cmd();
        let mut i = 0;
        while i < cmd_args.len() {
            let arg = &cmd_args[i];
            if is_deploy && arg == "--cleanup" {
                cmd.arg("-c");
            } else if (arg == "--topo" || arg == "-t") && i + 1 < cmd_args.len() {
                cmd.arg(arg);
                i += 1;
                let path_arg = &cmd_args[i];
                let p = Path::new(path_arg);
                let abs = if p.is_absolute() {
                    path_arg.clone()
                } else if let Ok(cwd) = std::env::current_dir() {
                    cwd.join(p).to_string_lossy().to_string()
                } else {
                    path_arg.clone()
                };

                if is_destroy && !Path::new(&abs).exists() {
                    let msg = format!("Topology file '{}' does not exist", abs);
                    let _ = log_tx.send(format!("ERROR: {}", msg));
                    anyhow::bail!(msg);
                }

                cmd.arg(abs);
            } else {
                cmd.arg(arg);
            }
            i += 1;
        }
        self.run_streaming(cmd, log_tx).await
    }

    /// Inspect running labs and return structured container info
    pub async fn inspect(
        &self,
        topo_file: Option<&Path>,
        all: bool,
    ) -> Result<Vec<ContainerInspectInfo>> {
        let mut cmd = self.build_cmd();
        cmd.arg("inspect");
        cmd.arg("--format");
        cmd.arg("json");

        if all || topo_file.is_none() {
            cmd.arg("--all");
        } else if let Some(path) = topo_file {
            cmd.arg("-t");
            cmd.arg(path);
        }

        let output = cmd
            .output()
            .await
            .context("Failed to execute containerlab inspect")?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "Inspect failed (code {:?}): {}",
                output.status.code(),
                err_msg
            );
        }

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        Self::parse_inspect_json(&stdout_str)
    }

    /// Parse JSON output from `containerlab inspect --format json`
    pub fn parse_inspect_json(json_str: &str) -> Result<Vec<ContainerInspectInfo>> {
        let trimmed = json_str.trim();
        if trimmed.is_empty() || trimmed == "{}" {
            return Ok(Vec::new());
        }

        // Output can be a map: {"lab_name": [containers...]}
        // Or a direct array: [containers...]
        if trimmed.starts_with('{') {
            let map: BTreeMap<String, Vec<ContainerInspectInfo>> =
                serde_json::from_str(trimmed).context("Parsing inspect JSON map")?;
            let mut list = Vec::new();
            for (lab_name, mut containers) in map {
                for c in &mut containers {
                    if c.lab_name.is_empty() {
                        c.lab_name = lab_name.clone();
                    }
                }
                list.extend(containers);
            }
            Ok(list)
        } else if trimmed.starts_with('[') {
            let list: Vec<ContainerInspectInfo> =
                serde_json::from_str(trimmed).context("Parsing inspect JSON array")?;
            Ok(list)
        } else {
            Ok(Vec::new())
        }
    }

    pub fn validate_identifier(name: &str, field_name: &str) -> Result<()> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            anyhow::bail!("{} cannot be empty", field_name);
        }
        if trimmed.starts_with('-') {
            anyhow::bail!("{} cannot start with a hyphen '-'", field_name);
        }
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '/' || c == '.')
        {
            anyhow::bail!("{} contains invalid characters", field_name);
        }
        Ok(())
    }

    /// Run packet capture on node interface
    pub async fn tools_capture(
        &self,
        topo_file: &Path,
        node: &str,
        interface: &str,
        log_tx: UnboundedSender<String>,
    ) -> Result<ExitStatus> {
        Self::validate_identifier(node, "Node name")?;
        Self::validate_identifier(interface, "Interface name")?;

        let mut cmd = self.build_cmd();
        cmd.arg("tools");
        cmd.arg("capture");
        cmd.arg("-t");
        cmd.arg(topo_file);
        cmd.arg("-n");
        cmd.arg(node);
        cmd.arg("-i");
        cmd.arg(interface);

        self.run_streaming(cmd, log_tx).await
    }

    /// Execute a command in a node container
    pub async fn exec_command(&self, node: &str, command: &str) -> Result<String> {
        Self::validate_identifier(node, "Node name")?;

        let mut cmd = if !self.is_root && self.sudo_available {
            let mut c = Command::new("sudo");
            c.arg("docker");
            c
        } else {
            Command::new("docker")
        };

        cmd.arg("exec");
        cmd.arg(node);
        cmd.arg("sh");
        cmd.arg("-c");
        cmd.arg(command);

        let output = cmd.output().await.context("Failed to run docker exec")?;
        let result = String::from_utf8_lossy(&output.stdout).to_string();
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Exec error: {}", err);
        }

        Ok(result)
    }

    /// Internal helper to stream command output line-by-line
    async fn run_streaming(
        &self,
        mut cmd: Command,
        log_tx: UnboundedSender<String>,
    ) -> Result<ExitStatus> {
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("Failed to spawn containerlab process: {}", e);
                let _ = log_tx.send(format!("ERROR: {}", err_msg));
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    let _ = log_tx.send(
                        "ERROR: Permission denied executing containerlab. Check executable permissions and elevated privileges."
                            .to_string(),
                    );
                } else if e.kind() == std::io::ErrorKind::NotFound {
                    let _ = log_tx.send(
                        "ERROR: containerlab executable not found in PATH. Install containerlab (https://containerlab.dev) or run with --mock."
                            .to_string(),
                    );
                }
                return Err(anyhow::anyhow!(err_msg));
            }
        };

        let stdout = child.stdout.take().expect("Child stdout piped");
        let stderr = child.stderr.take().expect("Child stderr piped");

        let tx_out = log_tx.clone();
        let stdout_handle = tokio::spawn(async move {
            let reader = tokio::io::BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_out.send(line);
            }
        });

        let tx_err = log_tx.clone();
        let stderr_handle = tokio::spawn(async move {
            let reader = tokio::io::BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_err.send(line);
            }
        });

        let status = child.wait().await.context("Process execution failed")?;
        let _ = stdout_handle.await;
        let _ = stderr_handle.await;

        if !status.success() {
            let code_str = status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "terminated by signal".to_string());
            let _ = log_tx.send(format!(
                "ERROR: Process exited with failure status code: {}",
                code_str
            ));
            if !self.is_privileged() {
                let _ = log_tx.send(
                    "ERROR: Teardown/Command failed. Containerlab commands typically require root or passwordless sudo privileges to interact with Docker daemon and network namespaces."
                        .to_string(),
                );
            }
        }

        Ok(status)
    }
}
