pub mod app;
pub mod canvas;
pub mod clab;
pub mod event;
pub mod graphics;
pub mod ui;

pub use app::App;
pub use canvas::state::{CanvasMode, CanvasState};
pub use clab::client::ClabClient;
pub use clab::commands::{
    default_command_tree, CliArg, CliArgKind, CliCategory, CliCommand, CliFocusedPane, CliViewState,
};
pub use clab::mock::MockClabClient;
pub use clab::model::{
    ContainerInspectInfo, LabTopology, LinkDefinition, NodeCategory, NodeDefinition, NodeProfile,
    KIND_TEMPLATES,
};
pub use clab::parser::{TopologyError, TopologyParser};
pub use clab::pattern::{InterfacePattern, PatternSet};
pub use event::{AppEvent, EventHandler};
pub use graphics::kitty::{GraphicsDetector, GraphicsProtocol, KittyGraphRenderer};
pub use ui::layout::{ActiveTab, AppLayout, ConfirmModal};
pub use ui::theme::Theme;
pub use ui::views::{
    AddNodeModal, CanvasView, CanvasViewParams, CliView, HelpPopup, InspectView, InspectViewState,
    LinkEditField, LinkEditModal, LogsView, ProfileEditModal, ProfileField, YamlView,
    YamlViewState,
};
pub use ui::widgets::{
    DrawerField, FileBrowserEntry, FileBrowserModal, FileEntryType, InspectorDrawer, LogViewer,
    Toast, ToastKind, ToastManager,
};
