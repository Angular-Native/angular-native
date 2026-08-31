//! Núcleo del renderer: shadow tree, mutaciones, commit y diff hacia el host.
//!
//! Nadie de fuera toca vistas nativas directamente. El flujo siempre es:
//!
//! ```text
//! JS  --mutaciones-->  ShadowTree  --commit-->  Frame { ops }  -->  HostRenderer
//! ```
//!
//! El commit es el único punto donde corre el layout, y el `Frame` que sale
//! contiene solo lo que cambió de verdad.

pub mod props;
pub mod tree;

pub use props::{NodeKind, PropValue};
pub use tree::{Frame, MountOp, NodeId, ShadowTree};

pub use an_layout::{FontSpec, NaiveMeasurer, Rect, TextMeasurer};
