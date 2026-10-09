# clab-tui

A Terminal User Interface (TUI) and interactive 2D canvas for [Containerlab](https://containerlab.dev).

`clab-tui` provides a terminal environment to design, visualize, and manage Containerlab network topologies. You can build topologies on a 2D canvas, wire nodes together, inspect container states, stream logs, edit topology YAML, and execute Containerlab commands directly.

---

## Features

- **2D Topology Canvas**:
  - Pan and zoom network topologies in your terminal.
  - Move nodes with keyboard arrow/Vim keys or mouse drag.
  - Select multiple nodes with marquee box selection (`Shift+drag`, `Alt+drag`, or visual mode `v`/`b`) and delete in bulk.
  - Add, clone, and configure node properties with the Node Property Drawer (`e`).
  - Supports common network kinds: Nokia SR Linux (`nokia_srlinux`), Arista cEOS (`arista_ceos`), Cisco C8000v (`cisco_c8000v`), Cisco XRv9k (`cisco_xrv9k`), Juniper cRPD (`juniper_crpd`), and Linux hosts (with legacy alias support).

- **Link Wiring & Routing**:
  - Connect node ports interactively in wiring mode (`w`).
  - Route links using orthogonal, diagonal, or octilinear styles (`r`).
  - Auto-assign interface patterns and edit link endpoints (`e`).

- **Containerlab Management**:
  - Deploy topologies (`d`) with options for `--cleanup` and `--reconfigure`.
  - Destroy running labs (`D`) with confirmation prompt.
  - Live Inspect view (`2` or `r`/`i`) displaying container status, IP addresses, kinds, and states.
  - Real-time streaming log viewer (`3`) with filtering and auto-scroll.
  - Interactive Containerlab CLI command runner (`5`) to configure flags and execute commands.

- **Bidirectional YAML Synchronization**:
  - Canvas layout and Containerlab YAML definitions stay synchronized in real time.
  - Canvas coordinates are saved in YAML labels so node positions persist.
  - View and edit YAML directly in the TUI (`4`).

- **Terminal Display & Compatibility**:
  - Universal Unicode box-drawing and badge rendering compatible with any standard terminal emulator.
  - Offline mock mode (`--mock`) for testing without Docker or root access.

---

## Installation

### Pre-built Binary (Linux x86_64)

Download the latest release archive from [GitHub Releases](https://github.com/andywhitaker/clab-tui/releases/latest):

```bash
# Download and extract the latest release
tar -xzf clab-tui-v0.0.1-linux-x86_64.tar.gz
chmod +x clab-tui
sudo mv clab-tui /usr/local/bin/
```

### Build from Source

Prerequisites: [Rust toolchain](https://rustup.rs/) (1.80+) and [Containerlab](https://containerlab.dev/install/) (optional if running in mock mode).

```bash
git clone https://github.com/andywhitaker/clab-tui.git
cd clab-tui
cargo build --release
```

The compiled binary will be located at `./target/release/clab-tui`.

---

## Quickstart

> **Note on Permissions**: Containerlab deploy and destroy operations require root or `sudo` privileges to interact with Docker and network namespaces. Run with `sudo clab-tui ...` or configure passwordless sudo. You can also run with `--mock` (`-m`) to explore the interface without root or Docker.

### Launch with Default Canvas
```bash
clab-tui
```

### Open an Existing Topology File
```bash
clab-tui -f examples/demo.clab.yml
```

### Run in Offline Mock Mode
Explore the interface without Docker or root privileges:
```bash
clab-tui -m
```

---

## Keybindings Reference

### Navigation & Views
| Key | Action |
|-----|--------|
| `1` | Switch to **Canvas** view |
| `2` | Switch to **Lab Inspector** view |
| `3` | Switch to **Live Logs** view |
| `4` | Switch to **YAML Source** view |
| `5` | Switch to **CLI Command Explorer** view |
| `Tab` / `Shift+Tab` | Cycle focus between panels |
| `?` / `F1` | Open keybinding help reference |
| `q` / `Ctrl+C` | Quit `clab-tui` (or dismiss open modal/drawer) |

### Canvas Designer
| Key | Action |
|-----|--------|
| `h` / `j` / `k` / `l` or `Arrow Keys` | Move selected node |
| `Shift+Arrows` / `Ctrl+Arrows` / `H`, `J`, `K`, `L` | Pan canvas viewport |
| `+` / `-` | Zoom in / Zoom out |
| `0` | Reset viewport and zoom (100%) |
| `v` / `b` / `Space` | Toggle visual / marquee box selection mode |
| `w` | Enter/exit port-to-port wiring mode |
| `a` | Open **Add Node** modal |
| `e` | Open **Node Property Drawer** (or edit selected link) |
| `[` / `]` | Select previous / next link |
| `c` | Clone selected node |
| `x` / `Delete` | Delete selected node, selected link, or marquee selection |
| `r` / `R` | Cycle link routing style (Orthogonal, Direct, Octilinear) |
| `Ctrl+S` | Save topology to file (or `s` in YAML view) |

### Containerlab Operations
| Key | Action |
|-----|--------|
| `d` | Deploy topology (shows cleanup / reconfigure options) |
| `D` | Destroy lab (shows confirmation modal) |
| `r` / `i` | Refresh inspect table |
| `Ctrl+S` | Save topology directly to `*.clab.yml` |

### Mouse Controls
| Input | Action |
|-------|--------|
| `Left Click` | Select node or link; click port anchor to start/complete link |
| `Left Drag` | Drag selected node across canvas |
| `Ctrl + Drag` or Drag empty canvas | Pan canvas viewport |
| `Shift + Drag` or `Alt + Drag` | Marquee box select multiple nodes |
| `Scroll Wheel` | Pan canvas viewport vertically |

---

## Examples

An example multi-vendor leaf-spine topology is provided in [`examples/demo.clab.yml`](examples/demo.clab.yml):

```bash
clab-tui -f examples/demo.clab.yml
```

---

## License

Dual-licensed under MIT OR Apache-2.0.
