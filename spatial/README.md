# spatial — Eisenstein Layout Intelligence

A hexagonal-lattice layout engine for the [Typst](https://typst.app) fork,
built on Eisenstein integers. Provides zero-drift geometric snapping,
spectral document structure analysis, and hexagonal packing — all with
exact integer arithmetic.

## Architecture

```
spatial/
├── Cargo.toml
├── src/
│   ├── lib.rs                # Public API, re-exports
│   ├── page_graph.rs         # Document as graph on E12 hex lattice
│   ├── grid_snap.rs          # Zero-drift hex snapping & packing
│   ├── structure_analysis.rs # Laplacian eigenmaps, Fiedler, Cheeger
│   └── report.rs             # Human-readable intelligence reports
└── README.md
```

## Modules

### `page_graph` — Document Page Graph

Pages as nodes on the A₂ Eisenstein hexagonal lattice, cross-references as
edges, reading flow as lattice paths.

- Pages placed on a hexagonal spiral from origin
- Cross-references model document connectivity
- Reading flow encoded as hex lattice paths
- Adjacency matrix and degree computation

### `grid_snap` — Hex Grid Element Snapping

Zero-drift hexagonal element snapping using the Eisenstein integer lattice.

- **Voronoi-based snap**: Guaranteed nearest-neighbor via 9-candidate search
  (covering radius ≤ 1/√3 of hex spacing)
- **Zero drift**: Integer arithmetic means roundtrip-safe snapping —
  once snapped, elements stay on lattice vertices forever
- **Image alignment**: Snap image centroids with bounding box computation
- **Hexagonal column packing**: ~15% denser than square grids
  (π/√12 ≈ 90.7% vs π/4 ≈ 78.5% packing density)

### `structure_analysis` — Spectral Document Analysis

Graph Laplacian → eigendecomposition → structural insights:

- **Graph Laplacian**: Normalized and unnormalized forms
- **Fiedler vector**: Second eigenvector reveals natural spectral cuts
- **Algebraic connectivity**: λ₂ measures how well-connected the document is
- **Bridge detection**: Articulation points whose removal disconnects the graph
- **Cheeger constant**: Quality measure for the best spectral split
- **Spectral clustering**: Pages grouped by Fiedler vector sign

### `report` — Layout Intelligence Reports

Human-readable reports that combine spectral analysis, bridge detection,
and packing density into actionable guidance.

> "47-page doc has 3 spectral clusters. Section 4 bridges topics A and B — split here."

### Crates used

- [`eisenstein`](https://crates.io/crates/eisenstein) (v0.3) — Eisenstein integer
  lattice, E12 type, hex disks, D6 symmetry
- [`snapkit`](https://crates.io/crates/snapkit) (v0.1) — Voronoï snap, spectral
  analysis, temporal grids
- [`nalgebra`](https://crates.io/crates/nalgebra) (v0.33) — Matrix operations,
  symmetric eigendecomposition

## Quick Start

```rust
use spatial::page_graph::PageGraph;
use spatial::structure_analysis::SpectralAnalysis;
use spatial::report::LayoutReport;

// Build a document graph
let mut graph = PageGraph::new();
graph.add_page(1, "Introduction".into());
graph.add_page(2, "Background".into());
graph.add_page(3, "Methods".into());
graph.add_page(4, "Results".into());
graph.add_page(5, "Discussion".into());

// Add cross-references
graph.add_ref(1, 3);
graph.add_ref(2, 4);
graph.add_ref(3, 5);

// Analyze
let analysis = SpectralAnalysis::analyze(&graph);
let report = LayoutReport::from_graph(&graph, 800.0, 600.0, 50.0);
println!("{}", report);
```

## Building

```bash
cd /tmp/spatial-typst
cargo build -p spatial
cargo test -p spatial
```

## License

Apache 2.0 (inherited from the Typst project).
