//! The renderer's core: shadow tree, mutations, commit, and the diff towards
//! the host.
//!
//! Nobody outside touches native views directly. The flow is always:
//!
//! ```text
//! JS  --mutations-->  ShadowTree  --commit-->  Frame { ops }  -->  HostRenderer
//! ```
//!
//! Commit is the only point where layout runs, and the `Frame` that comes out
//! carries only what actually changed.

pub mod accessibility;
pub mod color;
pub mod icons;
pub mod props;
pub mod tree;

pub use props::{NodeKind, PropValue};
pub use tree::{Frame, MountOp, NodeId, ShadowTree};

pub use an_layout::{FontSpec, NaiveMeasurer, Rect, TextMeasurer};
