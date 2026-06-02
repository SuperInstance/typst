//! # spatial — Eisenstein layout intelligence for the Typst fork
//!
//! A hexagonal-lattice layout engine built on Eisenstein integers, providing:
//!
//! - **`page_graph`** — Document pages as nodes on the E12 hex lattice,
//!   cross-references as edges, reading flow as lattice paths
//! - **`grid_snap`** — Zero-drift hexagonal element snapping, image alignment,
//!   hexagonal column packing (~15% denser than square grids)
//! - **`structure_analysis`** — Spectral document graph analysis via Laplacian
//!   eigenmaps, Fiedler vectors, Cheeger cuts, bridge section detection
//! - **`report`** — Human-readable layout intelligence reports
//!
//! ## Quick Start
//!
//! ```rust
//! use spatial::page_graph::PageGraph;
//! use spatial::grid_snap::HexGrid;
//! use spatial::structure_analysis::SpectralAnalysis;
//! use spatial::report::LayoutReport;
//!
//! // Build a document graph
//! let mut graph = PageGraph::new();
//! for i in 1..=5 {
//!     graph.add_page(i, format!("Section {}", i));
//! }
//! graph.add_ref(1, 3);
//! graph.add_ref(2, 4);
//! graph.add_ref(3, 5);
//! graph.add_ref(4, 5);
//!
//! // Run spectral analysis
//! let analysis = SpectralAnalysis::analyze(&graph);
//!
//! // Generate a human-readable report
//! let report = LayoutReport::from_analysis(&analysis);
//! println!("{}", report);
//! ```

#![allow(clippy::excessive_precision)]

pub mod page_graph;
pub mod grid_snap;
pub mod structure_analysis;
pub mod report;

// Re-export key types at crate root
pub use page_graph::PageGraph;
pub use grid_snap::HexGrid;
pub use structure_analysis::SpectralAnalysis;
pub use report::LayoutReport;
