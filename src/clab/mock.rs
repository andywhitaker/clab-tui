use crate::clab::model::{ContainerInspectInfo, LabTopology};
use anyhow::Result;
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, Default)]
pub struct MockClabClient;

impl MockClabClient {
    pub fn new() -> Self {
        Self
    }

    /// Simulate lab deployment with realistic streaming logs
    pub async fn simulate_deploy(
        &self,
        topo: &LabTopology,
        reconfigure: bool,
        cleanup: bool,
        log_tx: UnboundedSender<String>,
    ) -> Result<()> {
        let lab_name = &topo.name;

        if cleanup {
            let _ = log_tx.send(format!(
                "INFO[0000] Removing previous lab artifacts for '{}' (--cleanup)...",
                lab_name
            ));
            tokio::time::sleep(Duration::from_millis(30)).await;
        }

        if reconfigure {
            let _ = log_tx.send(
                "INFO[0000] Reconfiguring existing containers (--reconfigure)...".to_string(),
            );
            tokio::time::sleep(Duration::from_millis(30)).await;
        }

        let logs = vec![
            format!(
                "INFO[0000] Parsing & validating topology: {}.clab.yml",
                lab_name
            ),
            "INFO[0000] Containerlab version 0.79.0".to_string(),
            "INFO[0000] Creating docker network: clab (172.20.20.0/24)...".to_string(),
            format!("INFO[0001] Creating lab directory: /etc/clab/{}", lab_name),
        ];

        for line in logs {
            let _ = log_tx.send(line);
            tokio::time::sleep(Duration::from_millis(40)).await;
        }

        // Emit node creations
        for (i, (node_name, node_def)) in topo.topology.nodes.iter().enumerate() {
            let kind = node_def.kind.as_deref().unwrap_or("linux");
            let image = node_def.image.as_deref().unwrap_or("default");
            let line = format!(
                "INFO[{:04}] Creating container: clab-{}-{} [kind={}, image={}]",
                i + 1,
                lab_name,
                node_name,
                kind,
                image
            );
            let _ = log_tx.send(line);
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Emit link creations
        for (i, link) in topo.topology.links.iter().enumerate() {
            let line = format!(
                "INFO[{:04}] Creating veth pair: {} <--> {}",
                topo.topology.nodes.len() + i + 1,
                link.endpoints[0],
                link.endpoints[1]
            );
            let _ = log_tx.send(line);
            tokio::time::sleep(Duration::from_millis(40)).await;
        }

        let summary = vec![
            format!(
                "INFO[{:04}] Generating hosts file entries...",
                topo.topology.nodes.len() + topo.topology.links.len() + 1
            ),
            format!(
                "INFO[{:04}] Executing post-deploy tasks...",
                topo.topology.nodes.len() + topo.topology.links.len() + 2
            ),
            format!(
                "INFO[{:04}] Lab '{}' successfully deployed!",
                topo.topology.nodes.len() + topo.topology.links.len() + 3,
                lab_name
            ),
        ];

        for line in summary {
            let _ = log_tx.send(line);
            tokio::time::sleep(Duration::from_millis(30)).await;
        }

        Ok(())
    }

    /// Simulate lab destruction with realistic streaming logs
    pub async fn simulate_destroy(
        &self,
        topo_file: &Path,
        cleanup: bool,
        log_tx: UnboundedSender<String>,
    ) -> Result<()> {
        let lab_name = topo_file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("lab")
            .replace(".clab", "");

        let steps = vec![
            format!(
                "INFO[0000] Parsing & validating topology file: {}",
                topo_file.display()
            ),
            format!("INFO[0000] Destroying lab: {}", lab_name),
            "INFO[0001] Removing veth links...".to_string(),
            "INFO[0001] Stopping and removing node containers...".to_string(),
            format!("INFO[0002] Container clab-{}-srl1 removed", lab_name),
            format!("INFO[0002] Container clab-{}-srl2 removed", lab_name),
            format!("INFO[0002] Container clab-{}-host1 removed", lab_name),
            format!("INFO[0002] Container clab-{}-host2 removed", lab_name),
        ];

        for step in steps {
            let _ = log_tx.send(step);
            tokio::time::sleep(Duration::from_millis(40)).await;
        }

        if cleanup {
            let _ = log_tx.send(format!(
                "INFO[0003] Removing lab directory /etc/clab/{}...",
                lab_name
            ));
            tokio::time::sleep(Duration::from_millis(30)).await;
        }

        let _ = log_tx.send(format!(
            "INFO[0003] Lab '{}' successfully destroyed!",
            lab_name
        ));
        Ok(())
    }

    /// Simulate inspect response based on topology
    pub fn mock_inspect(topo: &LabTopology) -> Vec<ContainerInspectInfo> {
        let mut list = Vec::new();

        for (octet, (node_name, def)) in (11usize..).zip(topo.topology.nodes.iter()) {
            let kind = def.kind.as_deref().unwrap_or("linux").to_string();
            let image = def
                .image
                .as_deref()
                .unwrap_or(match crate::clab::model::canonical_kind(&kind) {
                    "nokia_srlinux" => "ghcr.io/nokia/srlinux:latest",
                    "arista_ceos" => "ceos:latest",
                    "cisco_c8000v" => "c8000v:latest",
                    "cisco_xrv9k" => "xrv9k:latest",
                    "juniper_crpd" => "crpd:latest",
                    "sonic-vs" => "docker-sonic-vs:latest",
                    "checkpoint_cloudguard" => "checkpoint:latest",
                    "nokia_sros" => "sros:latest",
                    "arista_veos" => "veos:latest",
                    "juniper_vmx" => "vmx:latest",
                    "nvidia_cumulusvx" => "cumulus-vx:latest",
                    _ => "ghcr.io/srl-labs/network-multitool:latest",
                })
                .to_string();

            let ipv4 = def
                .mgmt_ipv4
                .clone()
                .unwrap_or_else(|| format!("172.20.20.{}", octet));

            list.push(ContainerInspectInfo {
                lab_name: topo.name.clone(),
                lab_path: Some(format!("/etc/clab/{}/topology.clab.yml", topo.name)),
                name: format!("clab-{}-{}", topo.name, node_name),
                container_id: Some(format!("c{:x}{:x}", octet * 1234, octet)),
                image,
                kind,
                state: "running".to_string(),
                ipv4_address: Some(ipv4),
                ipv6_address: Some(format!("2001:172:20:20::{:x}/64", octet)),
            });
        }

        list
    }

    /// Simulate arbitrary containerlab CLI command execution with realistic streaming output
    pub async fn simulate_command(
        &self,
        command_name: &str,
        args: &[String],
        log_tx: UnboundedSender<String>,
    ) -> Result<()> {
        let cmd = command_name.trim();
        let _ = log_tx.send(format!(
            "INFO[0000] Executing: containerlab {}",
            args.join(" ")
        ));
        tokio::time::sleep(Duration::from_millis(30)).await;

        match cmd {
            "deploy" | "redeploy" => {
                let has_cleanup = args.iter().any(|a| a == "--cleanup" || a == "-c");
                let has_reconfig = args.iter().any(|a| a == "--reconfigure");
                if has_cleanup {
                    let _ = log_tx.send(
                        "INFO[0000] Removing previous lab artifacts (--cleanup)...".to_string(),
                    );
                    tokio::time::sleep(Duration::from_millis(30)).await;
                }
                if has_reconfig {
                    let _ = log_tx.send(
                        "INFO[0000] Reconfiguring existing containers (--reconfigure)..."
                            .to_string(),
                    );
                    tokio::time::sleep(Duration::from_millis(30)).await;
                }
                let _ = log_tx.send("INFO[0000] Creating docker network: clab...".to_string());
                tokio::time::sleep(Duration::from_millis(40)).await;
                let _ = log_tx.send(
                    "INFO[0001] Provisioning containers and virtual interfaces...".to_string(),
                );
                tokio::time::sleep(Duration::from_millis(40)).await;
                let _ = log_tx.send("INFO[0002] Lab successfully deployed!".to_string());
            }
            "destroy" => {
                let has_cleanup = args.iter().any(|a| a == "--cleanup" || a == "-c");
                let _ = log_tx.send(
                    "INFO[0000] Destroying lab containers and veth interfaces...".to_string(),
                );
                tokio::time::sleep(Duration::from_millis(40)).await;
                if has_cleanup {
                    let _ = log_tx.send(
                        "INFO[0001] Removing lab directory and bridge networks (--cleanup)..."
                            .to_string(),
                    );
                    tokio::time::sleep(Duration::from_millis(30)).await;
                }
                let _ = log_tx.send("INFO[0001] Lab successfully destroyed!".to_string());
            }
            "inspect" => {
                let _ = log_tx.send("INFO[0000] Inspecting containerlab labs:".to_string());
                tokio::time::sleep(Duration::from_millis(30)).await;
                let _ = log_tx.send("+---+------------------+--------------+-----------------------+---------------+----------------+".to_string());
                let _ = log_tx.send("| # | Name             | Container ID | Image                 | Kind          | State          |".to_string());
                let _ = log_tx.send("+---+------------------+--------------+-----------------------+---------------+----------------+".to_string());
                let _ = log_tx.send("| 1 | clab-demo-srl1   | a1b2c3d4e5f6 | ghcr.io/nokia/srlinux | nokia_srlinux | running        |".to_string());
                let _ = log_tx.send("| 2 | clab-demo-host1  | f6e5d4c3b2a1 | network-multitool     | linux         | running        |".to_string());
                let _ = log_tx.send("+---+------------------+--------------+-----------------------+---------------+----------------+".to_string());
            }
            "graph" => {
                let _ = log_tx.send("INFO[0000] Generating topology graph...".to_string());
                tokio::time::sleep(Duration::from_millis(30)).await;
                let _ = log_tx
                    .send("INFO[0001] Graph server listening on http://0.0.0.0:50080".to_string());
            }
            "save" => {
                let _ = log_tx.send(
                    "INFO[0000] Extracting running configuration from lab containers..."
                        .to_string(),
                );
                tokio::time::sleep(Duration::from_millis(40)).await;
                let _ = log_tx
                    .send("INFO[0001] Saved configurations saved to clab directory.".to_string());
            }
            "version" => {
                let _ = log_tx.send("INFO[0000] version: 0.79.0".to_string());
                let _ = log_tx.send("INFO[0000] commit: 8e5f2b1c".to_string());
                let _ = log_tx.send("INFO[0000] date: 2026-03-15T12:00:00Z".to_string());
                let _ = log_tx.send(
                    "INFO[0000] source: https://github.com/srl-labs/containerlab".to_string(),
                );
                let _ = log_tx.send("INFO[0000] rel: 0.79.0".to_string());
            }
            "exec" => {
                let _ =
                    log_tx.send("INFO[0000] Executing command on target containers...".to_string());
                tokio::time::sleep(Duration::from_millis(30)).await;
                let _ = log_tx.send(
                    "INFO[0000] [srl1] Linux srl1 6.8.0-clab #1 SMP PREEMPT x86_64 GNU/Linux"
                        .to_string(),
                );
                let _ = log_tx.send(
                    "INFO[0000] [host1] Linux host1 6.8.0-clab #1 SMP PREEMPT x86_64 GNU/Linux"
                        .to_string(),
                );
            }
            "config" => {
                let _ = log_tx.send(
                    "INFO[0000] Applying interface and network configuration templates..."
                        .to_string(),
                );
                tokio::time::sleep(Duration::from_millis(30)).await;
                let _ = log_tx.send("INFO[0000] Configuration applied successfully.".to_string());
            }
            "tools capture" => {
                let _ = log_tx
                    .send("INFO[0000] Initiating packet capture on node interface...".to_string());
                tokio::time::sleep(Duration::from_millis(40)).await;
                let _ = log_tx
                    .send("INFO[0001] Capture active: writing packets to PCAP...".to_string());
                let _ = log_tx.send("INFO[0001] Captured 42 packets (0 dropped)".to_string());
            }
            "tools veth" => {
                let _ = log_tx.send(
                    "INFO[0000] Creating veth link endpoints across namespaces...".to_string(),
                );
                tokio::time::sleep(Duration::from_millis(30)).await;
                let _ = log_tx
                    .send("INFO[0000] veth link created and mapped successfully.".to_string());
            }
            "tools disable-tx-offload" => {
                let _ = log_tx
                    .send("INFO[0000] Disabling TX checksum offload on interface...".to_string());
                tokio::time::sleep(Duration::from_millis(30)).await;
                let _ = log_tx.send("INFO[0000] TX offload disabled successfully.".to_string());
            }
            "tools vxlan" => {
                let _ = log_tx
                    .send("INFO[0000] Setting up VXLAN endpoint and bridge tunnel...".to_string());
                tokio::time::sleep(Duration::from_millis(30)).await;
                let _ = log_tx.send("INFO[0000] VXLAN interface created and linked.".to_string());
            }
            "tools netem" => {
                let _ = log_tx.send(
                    "INFO[0000] Configuring tc qdisc netem rules on interface...".to_string(),
                );
                tokio::time::sleep(Duration::from_millis(30)).await;
                let _ =
                    log_tx.send("INFO[0000] Netem parameters applied successfully.".to_string());
            }
            "tools cert" => {
                let _ = log_tx.send(
                    "INFO[0000] Generating Certificate Authority and node keypairs...".to_string(),
                );
                tokio::time::sleep(Duration::from_millis(30)).await;
                let _ =
                    log_tx.send("INFO[0000] Generated certificates and private keys.".to_string());
            }
            "tools api-server" => {
                let _ =
                    log_tx.send("INFO[0000] Initializing containerlab API server...".to_string());
                tokio::time::sleep(Duration::from_millis(30)).await;
                let _ =
                    log_tx.send("INFO[0001] REST API server listening at 0.0.0.0:8000".to_string());
            }
            _ => {
                let _ = log_tx.send(format!("INFO[0000] Running containerlab {}...", cmd));
                tokio::time::sleep(Duration::from_millis(30)).await;
                let _ = log_tx.send("INFO[0000] Command completed successfully.".to_string());
            }
        }

        Ok(())
    }
}
