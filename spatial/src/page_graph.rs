//! Document page graph — pages as nodes on the E12 Eisenstein hex lattice,
//! cross-references as edges, reading flow as lattice paths.
//!
//! Each page is assigned a position on the A₂ hexagonal lattice (Eisenstein
//! integer coordinates). Cross-references between pages become edges. The
//! reading flow of a document forms a path through the lattice, where each
//! step follows one of the six unit directions.
//!
//! ## Layout strategy
//!
//! Positions are assigned in reading order along a spiral-like hex path that
//! fills the lattice outward. Cross-references bridge non-adjacent pages,
//! creating edges that may span multiple hex steps.

use eisenstein::E12;

/// A page node in the document graph.
#[derive(Debug, Clone)]
pub struct PageNode {
    /// Page number (1-indexed).
    pub page: usize,
    /// Page title / heading text.
    pub title: String,
    /// Hex lattice coordinate on the E12 grid.
    pub position: E12,
    /// Cross-references to other pages (by page number).
    pub refs: Vec<usize>,
}

impl PageNode {
    /// Create a new page node at a given hex position.
    pub fn new(page: usize, title: String, position: E12) -> Self {
        Self {
            page,
            title,
            position,
            refs: Vec::new(),
        }
    }
}

/// A document as a graph on the E12 hexagonal lattice.
#[derive(Debug, Clone)]
pub struct PageGraph {
    /// All pages, indexed by page number (1-indexed, page 0 unused).
    pages: Vec<Option<PageNode>>,
    /// Number of pages.
    page_count: usize,
    /// Sequence of hex steps forming the reading path.
    reading_path: Vec<E12>,
}

impl PageGraph {
    /// Create a new empty page graph.
    pub fn new() -> Self {
        Self {
            pages: vec![None], // index 0 unused
            page_count: 0,
            reading_path: Vec::new(),
        }
    }

    /// Add a page positioned along a hexagonal spiral starting from origin.
    ///
    /// The first page goes at (0, 0), then pages spiral outward on the hex
    /// lattice: (1, 0), (0, 1), (-1, 1), (-1, 0), (0, -1), (1, -1), and
    /// continue to successive rings. This maximises local adjacency for
    /// the document's natural page order.
    pub fn add_page(&mut self, page: usize, title: String) {
        assert!(page > 0, "Page numbers must be 1-indexed");
        if self.page_count >= self.spiral_size() as usize {
            // We need to grow — should not happen with reasonable documents,
            // but handle gracefully
        }

        let position = self.next_spiral_position();
        let node = PageNode::new(page, title, position);

        // Ensure pages vec is large enough
        while self.pages.len() <= page {
            self.pages.push(None);
        }
        self.pages[page] = Some(node);
        self.page_count += 1;
    }

    /// Add a page at a specific hex coordinate.
    pub fn add_page_at(&mut self, page: usize, title: String, pos: E12) {
        assert!(page > 0, "Page numbers must be 1-indexed");
        while self.pages.len() <= page {
            self.pages.push(None);
        }
        self.pages[page] = Some(PageNode::new(page, title, pos));
        self.page_count += 1;
    }

    /// Add a cross-reference from `from` to `to` (directed edge).
    pub fn add_ref(&mut self, from: usize, to: usize) {
        if let Some(Some(node)) = self.pages.get_mut(from) {
            if !node.refs.contains(&to) {
                node.refs.push(to);
            }
        }
    }

    /// Get a page node by page number.
    pub fn get_page(&self, page: usize) -> Option<&PageNode> {
        self.pages.get(page).and_then(|o| o.as_ref())
    }

    /// Get a mutable reference to a page node.
    pub fn get_page_mut(&mut self, page: usize) -> Option<&mut PageNode> {
        self.pages.get_mut(page).and_then(|o| o.as_mut())
    }

    /// Number of pages in the graph.
    pub fn page_count(&self) -> usize {
        self.page_count
    }

    /// Iterate over all pages (by page number).
    pub fn page_numbers(&self) -> Vec<usize> {
        self.pages
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.as_ref().map(|_| i))
            .collect()
    }

    /// Iterate over all page nodes.
    pub fn nodes(&self) -> Vec<&PageNode> {
        self.pages.iter().filter_map(|p| p.as_ref()).collect()
    }

    /// Total number of cross-references (edges) in the graph.
    pub fn total_refs(&self) -> usize {
        self.nodes().iter().map(|n| n.refs.len()).sum()
    }

    // ── Hex spiral position generator ─────────────────────────────────

    /// Generate the n-th hex spiral position (n = page_count).
    /// Order: centers, then ring 1 clockwise from East, then ring 2, etc.
    fn next_spiral_position(&self) -> E12 {
        if self.page_count == 0 {
            return E12::new(0, 0);
        }

        let n = self.page_count;
        // Compute which ring and which segment within that ring
        // Ring 0: 1 point
        // Ring 1: 6 points, ring 2: 12 points, ..., ring k: 6*k points
        // Total up to ring k (inclusive): 1 + 3k(k+1)
        let mut k = 1u32;
        let mut prev_total = 1u64;
        loop {
            let ring_pts = 6 * k as u64;
            let total = prev_total + ring_pts;
            if total > n as u64 {
                break;
            }
            prev_total = total;
            k += 1;
        }

        let ring_idx = n as u64 - prev_total; // 0-indexed within this ring
        let side_length = k; // each side of the hex ring has k points
        let side_length_u64 = side_length as u64;
        let side = (ring_idx / side_length_u64) as u32; // which of the 6 sides
        let step = (ring_idx % side_length_u64) as i32; // point along the side

        // Hex directions for the 6 sides (clockwise from East)
        // We assign the standard E12 unit directions
        // More direct: walk from the corner of ring k, side by side
        // Ring k starts at (k, 0) [East-most point of ring k]
        // Side 0: from (k, 0) stepping (-1, 0) k times → goes to (0, k)?
        // Hmm, let me use a direct formula.

        // For side s (0..6), step t (0..k-1):
        // Corner positions on ring k:
        // C0 = (k, 0)
        // C1 = (0, k)
        // C2 = (-k, k)
        // C3 = (-k, 0)
        // C4 = (0, -k)
        // C5 = (k, -k)
        // Between C_s and C_{s+1}, we step in direction d_s:

        // Direction along side s:
        // S0: (k,0) → (0,k):  (-k, k) = k * (-1, 1) → step (-1, 1)
        // S1: (0,k) → (-k,k):  (-k, 0) = step (-1, 0)
        // S2: (-k,k) → (-k,0): (0, -k) = step (0, -1)
        // S3: (-k,0) → (0,-k): (k, -k) = step (1, -1)
        // S4: (0,-k) → (k,-k): (k, 0) = step (1, 0)
        // S5: (k,-k) → (k,0):  (0, k) = step (0, 1)
        // Wait, these steps don't match the E12 directions in order.
        // Let me use a cleaner formulation based on axial coords directly.

        self.ring_position(k, side, step)
    }

    /// Compute position on ring `k` at side `side` (0..6), step `step` (0..k-1).
    fn ring_position(&self, k: u32, side: u32, step: i32) -> E12 {
        // Corner positions of ring k in axial coordinates:
        // C0 =  ( k,  0)
        // C1 =  ( 0,  k)
        // C2 =  (-k,  k)
        // C3 =  (-k,  0)
        // C4 =  ( 0, -k)
        // C5 =  ( k, -k)
        // back to C0 = (k, 0)

        let k = k as i32;
        let t = step; // 0..k-1 along side

        match side {
            0 => E12::new(k - t, 0 + t),       // (k,0) → (0,k): step (-1, +1)
            1 => E12::new(0 - t, k + 0),       // (0,k) → (-k,k): step (-1, 0)
            2 => E12::new(-k + 0, k - t),      // (-k,k) → (-k,0): step (0, -1)
            3 => E12::new(-k + t, 0 - t),      // (-k,0) → (0,-k): step (+1, -1)
            4 => E12::new(0 + t, -k + 0),      // (0,-k) → (k,-k): step (+1, 0)
            5 => E12::new(k + 0, -k + t),      // (k,-k) → (k,0): step (0, +1)
            _ => unreachable!(),
        }
    }

    /// Total pages that can fit in the currently allocated spiral.
    fn spiral_size(&self) -> u64 {
        1 // grows as needed
    }

    /// Compute the hex distance between two pages in the lattice.
    pub fn page_distance(&self, a: usize, b: usize) -> Option<u32> {
        let pa = self.get_page(a)?;
        let pb = self.get_page(b)?;
        Some((pa.position - pb.position).hex_distance())
    }

    /// Compute the reading path as a sequence of hex directions.
    /// The reading path connects pages in page-number order.
    pub fn reading_path_directions(&self) -> Vec<E12> {
        let nums = self.page_numbers();
        nums.windows(2)
            .filter_map(|w| {
                let p1 = self.get_page(w[0])?;
                let p2 = self.get_page(w[1])?;
                let delta = p2.position - p1.position;
                // Find the shortest hex path step from origin in delta's direction
                // We return delta itself (it may span multiple unit steps)
                Some(delta)
            })
            .collect()
    }

    /// Build the adjacency matrix as a symmetric boolean matrix.
    /// Returns a flat vec where entry [i*n + j] = true if pages i+1 and j+1
    /// are connected by a cross-reference in either direction.
    pub fn adjacency_matrix(&self) -> Vec<bool> {
        let n = self.page_count;
        let mut mat = vec![false; n * n];
        for node in self.nodes() {
            for &ref_p in &node.refs {
                if let Some(other) = self.get_page(ref_p) {
                    let i = node.page - 1;
                    let j = other.page - 1;
                    if i < n && j < n {
                        mat[i * n + j] = true;
                        mat[j * n + i] = true; // symmetric
                    }
                }
            }
        }
        mat
    }

    /// Degree of each page (number of cross-references).
    pub fn degrees(&self) -> Vec<usize> {
        let mut degs = vec![0usize; self.page_count];
        for node in self.nodes() {
            let i = node.page - 1;
            if i < self.page_count {
                degs[i] += node.refs.len();
            }
            for &ref_p in &node.refs {
                if let Some(other) = self.get_page(ref_p) {
                    let j = other.page - 1;
                    if j < self.page_count {
                        degs[j] += 1;
                    }
                }
            }
        }
        degs
    }
}

impl Default for PageGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let g = PageGraph::new();
        assert_eq!(g.page_count(), 0);
    }

    #[test]
    fn test_single_page() {
        let mut g = PageGraph::new();
        g.add_page(1, "Introduction".into());
        assert_eq!(g.page_count(), 1);
        assert_eq!(g.get_page(1).unwrap().title, "Introduction");
        assert_eq!(g.get_page(1).unwrap().position, E12::new(0, 0));
    }

    #[test]
    fn test_spiral_positions() {
        let mut g = PageGraph::new();
        // Pages 1..7 should fill center + ring 1
        for i in 1..=7 {
            g.add_page(i, format!("Page {}", i));
        }
        assert_eq!(g.page_count(), 7);

        // Center at (0, 0)
        assert_eq!(g.get_page(1).unwrap().position, E12::new(0, 0));

        // Ring 1 positions (one per unit neighbor, clockwise):
        let ring1: Vec<E12> = (2..=7)
            .map(|i| g.get_page(i).unwrap().position)
            .collect();

        // All ring 1 positions should have hex distance 1 from origin
        for pos in &ring1 {
            assert_eq!(pos.hex_distance(), 1);
        }
    }

    #[test]
    fn test_reading_path() {
        let mut g = PageGraph::new();
        for i in 1..=5 {
            g.add_page(i, format!("P{}", i));
        }

        let path = g.reading_path_directions();
        // Each consecutive pair should connect adjacent hex positions
        assert_eq!(path.len(), 4);
        // All steps should be non-zero
        for step in &path {
            assert!(step.a() != 0 || step.b() != 0);
        }
    }

    #[test]
    fn test_cross_references() {
        let mut g = PageGraph::new();
        for i in 1..=7 {
            g.add_page(i, format!("S{}", i));
        }
        g.add_ref(1, 3);
        g.add_ref(3, 5);
        g.add_ref(2, 6);
        g.add_ref(1, 7);

        assert_eq!(g.total_refs(), 4);
        assert_eq!(g.get_page(1).unwrap().refs.len(), 2);
        assert_eq!(g.get_page(3).unwrap().refs.len(), 1);
    }

    #[test]
    fn test_adjacency_matrix() {
        let mut g = PageGraph::new();
        for i in 1..=5 {
            g.add_page(i, format!("P{}", i));
        }
        g.add_ref(1, 3);
        g.add_ref(2, 4);

        let adj = g.adjacency_matrix();
        let n = g.page_count();
        // (1,3) and (3,1) should be true
        assert!(adj[0 * n + 2]);
        assert!(adj[2 * n + 0]);
        // (1,2) should be false
        assert!(!adj[0 * n + 1]);
    }

    #[test]
    fn test_degrees() {
        let mut g = PageGraph::new();
        for i in 1..=5 {
            g.add_page(i, format!("P{}", i));
        }
        g.add_ref(1, 3);
        g.add_ref(3, 5);
        g.add_ref(1, 5);

        let degs = g.degrees();
        assert_eq!(degs[0], 2); // page 1
        assert_eq!(degs[2], 2); // page 3
        assert_eq!(degs[4], 2); // page 5
        assert_eq!(degs[1], 0); // page 2
    }

    #[test]
    fn test_page_distance() {
        let mut g = PageGraph::new();
        g.add_page_at(1, "A".into(), E12::new(0, 0));
        g.add_page_at(2, "B".into(), E12::new(3, 0));
        g.add_page_at(3, "C".into(), E12::new(-2, 1));

        assert_eq!(g.page_distance(1, 2), Some(3));
        assert_eq!(g.page_distance(1, 3), Some(2));
        assert_eq!(g.page_distance(2, 3), Some(5));
    }
}
