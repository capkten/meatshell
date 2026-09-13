#[path = "struct/layout.rs"]
#[expect(
    clippy::module_inception,
    reason = "keep the layout implementation path stable"
)]
mod layout;
#[path = "impls/panes.rs"]
mod panes;

pub(crate) use layout::{Dir, Layout, LogicalRect, TerminalWheelHit};
