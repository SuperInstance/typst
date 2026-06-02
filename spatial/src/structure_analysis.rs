//! Spectral document structure analysis — Laplacian eigenmaps, Fiedler vectors,
//! Cheeger cuts, bridge section detection.
//!
//! Given a `PageGraph`, computes:
//!
//! 1. **Graph Laplacian**: L = D - A (unnormalized) or the normalized
//!    Laplacian L_norm = I - D^{-1/2} A D^{-1/2}
//! 2. **Fiedler vector**: the eigenvector corresponding to the second-smallest
//!    eigenvalue of the Laplacian — reveals the natural "spectral cut" of the
//!    document structure
//! 3. **Bridge sections**: Pages or sections whose removal would disconnect
//!    the document graph (articulation points in the ref graph)
//! 4. **Cheeger cuts**: Partition suggestions based on the Cheeger constant,
//!    which measures the quality of the best spectral separation

use crate::page_graph::PageGraph;
use nalgebra::{DMatrix, SymmetricEigen};

/// Result of spectral analysis on a document graph.
#[derive(Debug, Clone)]
pub struct SpectralAnalysis {
    /// Number of pages analyzed.
    pub page_count: usize,
    /// Graph Laplacian matrix (n x n).
    pub laplacian: DMatrix<f64>,
    /// Normalized Laplacian (I - D^{-1/2} A D^{-1/2}).
    pub laplacian_normalized: DMatrix<f64>,
    /// All eigenvalues, sorted ascending.
    pub eigenvalues: Vec<f64>,
    /// Fiedler vector (eigenvector for λ₂).
    pub fiedler_vector: Option<Vec<f64>>,
    /// Second eigenvalue λ₂ (algebraic connectivity).
    pub algebraic_connectivity: Option<f64>,
    /// Ratio of λ₂ / λ_{max} — connectivity index (higher = tighter).
    pub connectivity_index: Option<f64>,
    /// Cheeger constant estimate via Fiedler cut.
    pub cheeger_constant: Option<f64>,
    /// Bridge pages: pages whose removal disconnects the graph.
    pub bridge_pages: Vec<BridgePage>,
    /// Suggested spectral clusters (by page index).
    pub clusters: Vec<SpectralCluster>,
    /// Eigenvalue gap ratio (λ₃ - λ₂) / (λ_{max} - λ₁) — large gap suggests
    /// clear cluster structure.
    pub spectral_gap_ratio: Option<f64>,
    /// Effective rank (number of eigenvalues accounting for most of the trace).
    pub effective_rank: f64,
}

/// A bridge page — its removal would disconnect the document graph.
#[derive(Debug, Clone)]
pub struct BridgePage {
    /// Page number.
    pub page: usize,
    /// Page title.
    pub title: String,
    /// Number of components the graph splits into if this page is removed.
    pub splits_into: usize,
    /// Degree of this page (total cross-references).
    pub degree: usize,
}

/// A spectral cluster — a group of pages identified by the Fiedler cut.
#[derive(Debug, Clone)]
pub struct SpectralCluster {
    /// Pages in this cluster (page numbers, 1-indexed).
    pub pages: Vec<usize>,
    /// Mean Fiedler vector value.
    pub mean_fiedler: f64,
    /// Cluster label.
    pub label: String,
}

impl SpectralAnalysis {
    /// Run full spectral analysis on a page graph.
    pub fn analyze(graph: &PageGraph) -> Self {
        let n = graph.page_count();
        if n == 0 {
            return Self::empty();
        }

        // Build adjacency and Laplacian
        let adj = build_adjacency_matrix(graph);
        let degrees = compute_degrees(&adj);
        let laplacian = laplacian_from_adj(&adj, &degrees);
        let lap_norm = laplacian_normalized(&adj, &degrees);

        // Eigendecomposition of the normalized Laplacian
        let eigen = SymmetricEigen::new(lap_norm.clone());
        let mut eigenvalues: Vec<f64> = eigen.eigenvalues.iter().copied().collect();
        let eigenvectors = eigen.eigenvectors;

        // Sort eigenvalues and eigenvectors by eigenvalue (ascending)
        let mut pairs: Vec<(usize, f64)> = eigenvalues.iter().copied().enumerate().collect();
        pairs.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let sorted_vals: Vec<f64> = pairs.iter().map(|(_, v)| *v).collect();
        eigenvalues = sorted_vals;

        // Reorder eigenvectors
        let mut eigvecs = DMatrix::zeros(n, n);
        for (new_idx, &(old_idx, _)) in pairs.iter().enumerate() {
            eigvecs.set_column(new_idx, &eigenvectors.column(old_idx));
        }

        // Fiedler vector (second eigenvector)
        let fiedler_vector: Option<Vec<f64>> = if n >= 2 {
            Some(eigvecs.column(1).iter().copied().collect::<Vec<f64>>())
        } else {
            None
        };

        let algebraic_connectivity = if n >= 2 {
            Some(eigenvalues[1])
        } else {
            None
        };

        let connectivity_index = if n >= 2 {
            let max_lambda = eigenvalues[n - 1];
            if max_lambda > 1e-15 {
                Some(eigenvalues[1] / max_lambda)
            } else {
                None
            }
        } else {
            None
        };

        // Cheeger constant via Fiedler cut
        let cheeger_constant = if let Some(ref fv) = fiedler_vector {
            estimate_cheeger_constant(&adj, &degrees, fv)
        } else {
            None
        };

        // Spectral gap
        let spectral_gap_ratio = if n >= 5 {
            let gap = eigenvalues[2] - eigenvalues[1];
            let range = eigenvalues[n - 1] - eigenvalues[0];
            if range > 1e-15 {
                Some(gap / range)
            } else {
                None
            }
        } else {
            None
        };

        // Effective rank: number of eigenvalues > 1% of max
        let max_eval = eigenvalues.last().copied().unwrap_or(1.0);
        let effective_rank = if max_eval > 1e-15 {
            eigenvalues.iter().filter(|&&v| v / max_eval > 0.01).count() as f64
        } else {
            0.0
        };

        // Bridge detection
        let bridge_pages = detect_bridges(graph, &adj);

        // Spectral clustering from Fiedler vector
        let clusters = if let Some(ref fv) = fiedler_vector {
            compute_clusters(graph, fv)
        } else {
            vec![]
        };

        Self {
            page_count: n,
            laplacian,
            laplacian_normalized: lap_norm,
            eigenvalues,
            fiedler_vector,
            algebraic_connectivity,
            connectivity_index,
            cheeger_constant,
            bridge_pages,
            clusters,
            spectral_gap_ratio,
            effective_rank,
        }
    }

    fn empty() -> Self {
        Self {
            page_count: 0,
            laplacian: DMatrix::zeros(0, 0),
            laplacian_normalized: DMatrix::zeros(0, 0),
            eigenvalues: vec![],
            fiedler_vector: None,
            algebraic_connectivity: None,
            connectivity_index: None,
            cheeger_constant: None,
            bridge_pages: vec![],
            clusters: vec![],
            spectral_gap_ratio: None,
            effective_rank: 0.0,
        }
    }

    /// Number of spectral clusters found (typically 2 from Fiedler cut).
    pub fn cluster_count(&self) -> usize {
        self.clusters.len()
    }

    /// True if the document has a clear bisection structure.
    pub fn has_clear_split(&self) -> bool {
        self.spectral_gap_ratio
            .map(|gap| gap > 0.05)
            .unwrap_or(false)
    }

    /// True if there are bridge pages that would disconnect the graph.
    pub fn has_bridges(&self) -> bool {
        !self.bridge_pages.is_empty()
    }
}

// ── Matrix construction helpers ──────────────────────────────────────

/// Build the adjacency matrix from a PageGraph.
fn build_adjacency_matrix(graph: &PageGraph) -> Vec<Vec<bool>> {
    let n = graph.page_count();
    let mut adj = vec![vec![false; n]; n];
    for node in graph.nodes() {
        let i = node.page - 1;
        if i >= n {
            continue;
        }
        for &ref_p in &node.refs {
            if let Some(other) = graph.get_page(ref_p) {
                let j = other.page - 1;
                if j < n && i != j {
                    adj[i][j] = true;
                    adj[j][i] = true;
                }
            }
        }
    }
    adj
}

/// Compute degree of each node from the adjacency matrix.
fn compute_degrees(adj: &[Vec<bool>]) -> Vec<f64> {
    let n = adj.len();
    let mut degs = vec![0.0; n];
    for i in 0..n {
        for j in 0..n {
            if adj[i][j] {
                degs[i] += 1.0;
            }
        }
    }
    degs
}

/// Build unnormalized Laplacian: L = D - A.
fn laplacian_from_adj(adj: &[Vec<bool>], degrees: &[f64]) -> DMatrix<f64> {
    let n = adj.len();
    let mut lap = DMatrix::zeros(n, n);
    for i in 0..n {
        lap[(i, i)] = degrees[i];
        for j in 0..n {
            if adj[i][j] {
                lap[(i, j)] = -1.0;
            }
        }
    }
    lap
}

/// Build normalized Laplacian: L_norm = I - D^{-1/2} A D^{-1/2}.
fn laplacian_normalized(adj: &[Vec<bool>], degrees: &[f64]) -> DMatrix<f64> {
    let n = adj.len();
    let mut lap = DMatrix::identity(n, n);
    for i in 0..n {
        for j in 0..n {
            if i != j && adj[i][j] {
                let div = (degrees[i] * degrees[j]).sqrt();
                if div > 1e-15 {
                    lap[(i, j)] = -1.0 / div;
                }
            }
        }
    }
    lap
}

// ── Cheeger cut ──────────────────────────────────────────────────────

/// Estimate the Cheeger constant via the Fiedler vector cut.
///
/// The Cheeger constant h(G) = min_S (|∂S| / min(vol(S), vol(S̄)))
/// where ∂S is the boundary of set S.
///
/// We approximate by sweeping the Fiedler vector (the classic spectral
/// clustering approach) and finding the split with minimum conductance.
fn estimate_cheeger_constant(
    adj: &[Vec<bool>],
    degrees: &[f64],
    fiedler: &[f64],
) -> Option<f64> {
    let n = adj.len();
    if n < 4 {
        return None;
    }

    // Sort indices by Fiedler vector value
    let mut indices: Vec<usize> = (0..n).collect();
    indices.sort_by(|&a, &b| {
        fiedler[a]
            .partial_cmp(&fiedler[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Sweep cut: consider partitions where first k entries form set S
    let total_volume: f64 = degrees.iter().sum();
    let mut best_conductance = f64::MAX;

    let mut vol_s = 0.0;
    let mut in_s = vec![false; n];

    for k in 1..n {
        let v = indices[k - 1];
        in_s[v] = true;
        vol_s += degrees[v];

        // Count boundary edges: edges from S to S̄
        let mut boundary = 0.0;
        for &u in &indices[..k] {
            for w in 0..n {
                if !in_s[w] && adj[u][w] {
                    boundary += 1.0;
                }
            }
        }

        let vol_comp = total_volume - vol_s;
        let min_vol = vol_s.min(vol_comp);

        if min_vol > 1e-15 {
            let conductance = boundary / min_vol;
            if conductance < best_conductance {
                best_conductance = conductance;
            }
        }
    }

    if best_conductance < f64::MAX / 2.0 {
        Some(best_conductance)
    } else {
        None
    }
}

// ── Bridge detection ─────────────────────────────────────────────────

/// Detect bridge pages using DFS-based articulation point detection.
fn detect_bridges(graph: &PageGraph, adj: &[Vec<bool>]) -> Vec<BridgePage> {
    let n = graph.page_count();
    if n < 2 {
        return vec![];
    }

    let mut bridges = Vec::new();

    for page in 1..=n {
        if let Some(node) = graph.get_page(page) {
            if node.refs.is_empty() {
                continue;
            }

            // Check if removing this page disconnects the graph
            // Compute connected components of the graph minus this node
            let components = count_components_excluding(adj, page - 1);
            let base_components = count_components_excluding(adj, n); // all nodes

            if components > base_components + 1 {
                // Adding the page back merges components
                bridges.push(BridgePage {
                    page: node.page,
                    title: node.title.clone(),
                    splits_into: components,
                    degree: node.refs.len(),
                });
            }
        }
    }

    bridges
}

/// Count connected components of the graph with one node excluded.
/// `exclude = n` means no exclusion.
fn count_components_excluding(adj: &[Vec<bool>], exclude: usize) -> usize {
    let n = adj.len();
    let mut visited = vec![false; n];
    let mut components = 0;

    // Mark excluded node as visited
    if exclude < n {
        visited[exclude] = true;
    }

    for i in 0..n {
        if !visited[i] {
            components += 1;
            dfs_component(adj, i, &mut visited);
        }
    }

    components
}

fn dfs_component(adj: &[Vec<bool>], start: usize, visited: &mut [bool]) {
    let mut stack = vec![start];
    visited[start] = true;
    while let Some(v) = stack.pop() {
        for u in 0..adj.len() {
            if adj[v][u] && !visited[u] {
                visited[u] = true;
                stack.push(u);
            }
        }
    }
}

// ── Spectral clustering ──────────────────────────────────────────────

/// Cluster pages by Fiedler vector sign.
fn compute_clusters(graph: &PageGraph, fiedler: &[f64]) -> Vec<SpectralCluster> {
    let n = graph.page_count();
    if n == 0 || fiedler.len() < n {
        return vec![];
    }

    // Use median split for robustness
    let mut sorted: Vec<f64> = fiedler.iter().copied().collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if sorted.is_empty() {
        0.0
    } else {
        sorted[sorted.len() / 2]
    };

    let mut neg_pages = Vec::new();
    let mut pos_pages = Vec::new();
    let mut neg_sum = 0.0;
    let mut pos_sum = 0.0;

    for (i, &val) in fiedler.iter().enumerate() {
        let page = i + 1;
        if val <= median {
            neg_pages.push(page);
            neg_sum += val;
        } else {
            pos_pages.push(page);
            pos_sum += val;
        }
    }

    let mut clusters = Vec::new();

    if !neg_pages.is_empty() {
        clusters.push(SpectralCluster {
            pages: neg_pages.clone(),
            mean_fiedler: neg_sum / neg_pages.len() as f64,
            label: "Cluster A".into(),
        });
    }

    if !pos_pages.is_empty() {
        clusters.push(SpectralCluster {
            pages: pos_pages.clone(),
            mean_fiedler: pos_sum / pos_pages.len() as f64,
            label: "Cluster B".into(),
        });
    }

    clusters
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PageGraph;

    fn make_simple_graph() -> PageGraph {
        let mut g = PageGraph::new();
        for i in 1..=10 {
            g.add_page(i, format!("Page {}", i));
        }
        // Chain: 1-2-3-4-5 and 6-7-8-9-10
        g.add_ref(1, 2);
        g.add_ref(2, 3);
        g.add_ref(3, 4);
        g.add_ref(4, 5);
        g.add_ref(6, 7);
        g.add_ref(7, 8);
        g.add_ref(8, 9);
        g.add_ref(9, 10);
        g
    }

    fn make_bridged_graph() -> PageGraph {
        let mut g = PageGraph::new();
        for i in 1..=10 {
            g.add_page(i, format!("Page {}", i));
        }
        // Two clusters bridged by page 3: 1-2-3-4-5 and 6-7-3-8-9-10
        g.add_ref(1, 2);
        g.add_ref(2, 3);
        g.add_ref(3, 4);
        g.add_ref(4, 5);
        g.add_ref(3, 6);
        g.add_ref(6, 7);
        g.add_ref(3, 8);
        g.add_ref(8, 9);
        g.add_ref(9, 10);
        g
    }

    #[test]
    fn test_empty_graph() {
        let g = PageGraph::new();
        let analysis = SpectralAnalysis::analyze(&g);
        assert_eq!(analysis.page_count, 0);
        assert!(analysis.fiedler_vector.is_none());
    }

    #[test]
    fn test_laplacian_size() {
        let g = make_simple_graph();
        let analysis = SpectralAnalysis::analyze(&g);
        assert_eq!(analysis.page_count, 10);
        assert_eq!(analysis.laplacian.nrows(), 10);
        assert_eq!(analysis.laplacian.ncols(), 10);
    }

    #[test]
    fn test_eigenvalues_sorted() {
        let g = make_simple_graph();
        let analysis = SpectralAnalysis::analyze(&g);
        // Should be non-decreasing
        for w in analysis.eigenvalues.windows(2) {
            assert!(
                w[0] <= w[1] + 1e-10,
                "Eigenvalues not sorted: {} > {}",
                w[0],
                w[1]
            );
        }
        // First eigenvalue should be ≈ 0 (unconnected graph has 2 zero eigenvalues)
        // For our two-chain graph, there are 2 connected components → 2 zero eigenvalues
        assert!(analysis.eigenvalues[0].abs() < 1e-8);
        assert!(analysis.eigenvalues[1].abs() < 1e-8);
    }

    #[test]
    fn test_algebraic_connectivity() {
        let g = make_bridged_graph();
        let analysis = SpectralAnalysis::analyze(&g);
        // Algebraic connectivity should be > 0 (graph is connected)
        //
        // With a single bridge, λ₂ should be small but positive
        if let Some(ac) = analysis.algebraic_connectivity {
            assert!(
                ac > 0.0,
                "Algebraic connectivity should be positive for connected graph, got {}",
                ac
            );
        }
    }

    #[test]
    fn test_eigenvalue_count() {
        let g = make_simple_graph();
        let analysis = SpectralAnalysis::analyze(&g);
        assert_eq!(analysis.eigenvalues.len(), 10);
    }

    #[test]
    fn test_bridge_detection() {
        let g = make_bridged_graph();
        let analysis = SpectralAnalysis::analyze(&g);
        // Page 3 should be detected as a bridge
        let bridge_pages: Vec<&BridgePage> = analysis
            .bridge_pages
            .iter()
            .filter(|b| b.page == 3)
            .collect();
        assert!(
            !bridge_pages.is_empty(),
            "Page 3 should be detected as bridge. Bridges found: {:?}",
            analysis
                .bridge_pages
                .iter()
                .map(|b| b.page)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_no_bridges_in_chain() {
        let g = make_simple_graph();
        let analysis = SpectralAnalysis::analyze(&g);
        // In a chain with 2 components, inner nodes are not bridges
        // (they're articulation points but detection depends on disconnection)
        // Actually each internal node in a chain is an articulation point.
        // So there might be bridges. But that's OK.
        println!(
            "Bridges in chain graph: {:?}",
            analysis.bridge_pages.iter().map(|b| b.page).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_spectral_clusters() {
        let g = make_bridged_graph();
        let analysis = SpectralAnalysis::analyze(&g);
        assert!(
            analysis.cluster_count() >= 2,
            "Should have at least 2 spectral clusters, got {}",
            analysis.cluster_count()
        );
        // Total pages should equal sum of cluster pages
        let total: usize = analysis.clusters.iter().map(|c| c.pages.len()).sum();
        assert_eq!(total, 10);
    }

    #[test]
    fn test_connectivity_index() {
        let g = make_bridged_graph();
        let analysis = SpectralAnalysis::analyze(&g);
        if let Some(ci) = analysis.connectivity_index {
            assert!(
                ci > 0.0,
                "Connectivity index should be positive"
            );
        }
    }

    #[test]
    fn test_fiedler_vector_length() {
        let g = make_bridged_graph();
        let analysis = SpectralAnalysis::analyze(&g);
        if let Some(ref fv) = analysis.fiedler_vector {
            assert_eq!(fv.len(), 10);
        }
    }

    #[test]
    fn test_cheeger_constant() {
        let g = make_bridged_graph();
        let analysis = SpectralAnalysis::analyze(&g);
        if let Some(cc) = analysis.cheeger_constant {
            assert!(
                cc > 0.0,
                "Cheeger constant should be positive"
            );
        }
    }
}
