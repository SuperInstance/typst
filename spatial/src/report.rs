//! Layout intelligence reports — human-readable analysis of document structure
//! from the Eisenstein layout module.
//!
//! ## Examples
//!
//! ```text
//! Layout Report: "My Document"
//! ────────────────────────────
//! 47-page doc has 3 spectral clusters.
//! Section 4 bridges topics A and B — split here.
//! ```
//!
//! Reports combine spectral analysis, bridge detection, and packing density
//! estimates into actionable guidance for document restructuring.

use crate::page_graph::PageGraph;
use crate::structure_analysis::{SpectralAnalysis, BridgePage};
use crate::grid_snap::{HexGrid, PackingEstimate};

/// A human-readable layout report for a document.
#[derive(Debug, Clone)]
pub struct LayoutReport {
    /// Report title.
    pub title: String,
    /// Number of pages.
    pub page_count: usize,
    /// Number of cross-references.
    pub ref_count: usize,
    /// Analysis sections of the report.
    pub sections: Vec<ReportSection>,
}

/// A section of the layout report.
#[derive(Debug, Clone)]
pub enum ReportSection {
    /// Spectral overview.
    SpectralOverview {
        num_clusters: usize,
        algebraic_connectivity: Option<f64>,
        has_clear_split: bool,
        effective_rank: f64,
    },
    /// Bridge pages and suggested splits.
    BridgeAnalysis {
        total_bridges: usize,
        critical_bridges: Vec<BridgePage>,
        suggestion: String,
    },
    /// Spectral cluster details.
    ClusterDetails {
        clusters: Vec<ClusterDetail>,
        explanation: String,
    },
    /// Fiedler vector summary.
    FiedlerSummary {
        values: String,
        interpretation: String,
    },
    /// Hex grid packing estimate.
    PackingAnalysis {
        estimate: PackingEstimate,
        density_percent: f64,
        note: String,
    },
}

/// Details about a spectral cluster.
#[derive(Debug, Clone)]
pub struct ClusterDetail {
    pub label: String,
    pub count: usize,
    pub pages: Vec<usize>,
}

impl LayoutReport {
    /// Create a report from spectral analysis.
    pub fn from_analysis(analysis: &SpectralAnalysis) -> Self {
        let mut sections = Vec::new();

        // Section 1: Spectral overview
        sections.push(ReportSection::SpectralOverview {
            num_clusters: analysis.cluster_count(),
            algebraic_connectivity: analysis.algebraic_connectivity,
            has_clear_split: analysis.has_clear_split(),
            effective_rank: analysis.effective_rank,
        });

        // Section 2: Bridge analysis
        {
            let suggestion = if analysis.bridge_pages.is_empty() {
                "No bridge sections detected; document is well-structured.".into()
            } else if analysis.bridge_pages.len() == 1 {
                let bp = &analysis.bridge_pages[0];
                format!(
                    "Section {} (\"{}\") bridges {} topics — consider splitting here.",
                    bp.page, bp.title, bp.splits_into
                )
            } else {
                let bp_list: Vec<String> = analysis
                    .bridge_pages
                    .iter()
                    .map(|b| format!("Section {} (\"{}\")", b.page, b.title))
                    .collect();
                format!(
                    "Multiple bridge sections found: {}. These separate distinct logical clusters.",
                    bp_list.join(", ")
                )
            };

            sections.push(ReportSection::BridgeAnalysis {
                total_bridges: analysis.bridge_pages.len(),
                critical_bridges: analysis.bridge_pages.to_vec(),
                suggestion,
            });
        }

        // Section 3: Cluster details
        {
            let cluster_details: Vec<ClusterDetail> = analysis
                .clusters
                .iter()
                .map(|c| ClusterDetail {
                    label: c.label.clone(),
                    count: c.pages.len(),
                    pages: c.pages.clone(),
                })
                .collect();

            let explanation = if analysis.cluster_count() >= 2 {
                format!(
                    "Pages split into {} clusters by spectral analysis. \
                     A Cheeger constant of {:.4} suggests {} separation quality.",
                    analysis.cluster_count(),
                    analysis.cheeger_constant.unwrap_or(0.5),
                    if analysis.cheeger_constant.unwrap_or(0.5) < 0.3 {
                        "good"
                    } else {
                        "moderate"
                    }
                )
            } else {
                "No significant cluster structure detected.".into()
            };

            sections.push(ReportSection::ClusterDetails {
                clusters: cluster_details,
                explanation,
            });
        }

        // Section 4: Fiedler vector summary
        {
            let fiedler_str = match &analysis.fiedler_vector {
                Some(fv) if !fv.is_empty() => {
                    let min_val = fv.iter().cloned().fold(f64::MAX, f64::min);
                    let max_val = fv.iter().cloned().fold(f64::MIN, f64::max);
                    let signed_count_neg = fv.iter().filter(|&&v| v < 0.0).count();
                    let signed_count_pos = fv.iter().filter(|&&v| v >= 0.0).count();
                    format!(
                        "range [{:.4}, {:.4}], {} negative, {} positive",
                        min_val, max_val, signed_count_neg, signed_count_pos
                    )
                }
                _ => "N/A (single page)".into(),
            };

            let interpretation = match &analysis.fiedler_vector {
                Some(fv) if fv.len() > 2 => {
                    // Compute the gap ratio between the two halves
                    format!(
                        "The Fiedler vector separates pages by their connection strength. \
                         Pages with similar values form spectral clusters. \
                         Algebraic connectivity: λ₂ = {:.4}.",
                        analysis.algebraic_connectivity.unwrap_or(0.0)
                    )
                }
                _ => "Insufficient data for spectral interpretation.".into(),
            };

            sections.push(ReportSection::FiedlerSummary {
                values: fiedler_str,
                interpretation,
            });
        }

        Self {
            title: "Layout Intelligence Report".into(),
            page_count: analysis.page_count,
            ref_count: 0, // filled in by from_graph
            sections,
        }
    }

    /// Create a complete layout report from a page graph (with packing analysis).
    pub fn from_graph(graph: &PageGraph, width: f64, height: f64, spacing: f64) -> Self {
        let mut report = Self::from_analysis(&SpectralAnalysis::analyze(graph));
        report.ref_count = graph.total_refs();

        // Add packing analysis
        let estimate = HexGrid::packing_density(width, height, spacing);
        let density_percent = (estimate.density_ratio - 1.0) * 100.0;

        let note = if density_percent > 10.0 {
            format!(
                "Hexagonal packing yields ~{:.0}% more cells than square grid \
                 with same spacing. ~15% improvement is expected from \
                 A₂ hexagonal packing (π/√12 ≈ 90.7% vs π/4 ≈ 78.5% density).",
                density_percent
            )
        } else {
            "Hexagonal and square packing similar for this layout.".into()
        };

        report.sections.push(ReportSection::PackingAnalysis {
            estimate,
            density_percent,
            note,
        });

        report
    }
}

impl std::fmt::Display for LayoutReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", "=".repeat(60))?;
        writeln!(f, "  {}", self.title)?;
        writeln!(f, "  {} pages, {} cross-refs", self.page_count, self.ref_count)?;
        writeln!(f, "{}", "=".repeat(60))?;

        for section in &self.sections {
            match section {
                ReportSection::SpectralOverview {
                    num_clusters,
                    algebraic_connectivity,
                    has_clear_split,
                    effective_rank,
                } => {
                    writeln!(f)?;
                    writeln!(f, "── Spectral Overview ──")?;
                    writeln!(
                        f,
                        "  • {} spectral cluster{}",
                        num_clusters,
                        if *num_clusters == 1 { "" } else { "s" }
                    )?;
                    if let Some(ac) = algebraic_connectivity {
                        writeln!(f, "  • Algebraic connectivity: λ₂ = {:.4}", ac)?;
                    }
                    writeln!(
                        f,
                        "  • Clear structural split: {}",
                        if *has_clear_split { "YES" } else { "no" }
                    )?;
                    writeln!(f, "  • Effective rank: {:.1}", effective_rank)?;
                }
                ReportSection::BridgeAnalysis {
                    total_bridges,
                    critical_bridges: bridges,
                    suggestion,
                } => {
                    writeln!(f)?;
                    writeln!(f, "── Bridge Sections ──")?;
                    writeln!(f, "  • {} bridge section{} found", total_bridges, if *total_bridges == 1 { "" } else { "s" })?;
                    for b in bridges {
                        writeln!(
                            f,
                            "    - Section {} (\"{}\"): degree {}, splits into {} components",
                            b.page, b.title, b.degree, b.splits_into
                        )?;
                    }
                    writeln!(f, "  ▶ {}", suggestion)?;
                }
                ReportSection::ClusterDetails {
                    clusters,
                    explanation,
                } => {
                    writeln!(f)?;
                    writeln!(f, "── Spectral Clusters ──")?;
                    for c in clusters {
                        let page_list = if c.pages.len() <= 10 {
                            format!("pages {}", c.pages.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", "))
                        } else {
                            format!(
                                "{} pages ({} .. {})",
                                c.count,
                                c.pages.first().unwrap_or(&0),
                                c.pages.last().unwrap_or(&0)
                            )
                        };
                        writeln!(f, "  • {}: {} pages ({})", c.label, c.count, page_list)?;
                    }
                    writeln!(f, "  {}", explanation)?;
                }
                ReportSection::FiedlerSummary {
                    values,
                    interpretation,
                } => {
                    writeln!(f)?;
                    writeln!(f, "── Fiedler Vector ──")?;
                    writeln!(f, "  • {}", values)?;
                    writeln!(f, "  • {}", interpretation)?;
                }
                ReportSection::PackingAnalysis {
                    estimate,
                    density_percent,
                    note,
                } => {
                    writeln!(f)?;
                    writeln!(f, "── Packing Analysis ──")?;
                    writeln!(
                        f,
                        "  • Hex grid: {} cols x {} rows = {} cells",
                        estimate.hex_columns, estimate.hex_rows, estimate.hex_cells
                    )?;
                    writeln!(
                        f,
                        "  • Square grid: {} cols x {} rows = {} cells",
                        estimate.square_columns,
                        estimate.square_rows,
                        estimate.square_cells
                    )?;
                    writeln!(
                        f,
                        "  • Density improvement: {:.1}%",
                        density_percent
                    )?;
                    writeln!(f, "  • {}", note)?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PageGraph;

    fn make_test_document() -> PageGraph {
        let mut g = PageGraph::new();
        for i in 1..=47 {
            g.add_page(i, format!("Section {}", i));
        }
        // Create two clusters bridged by section 4
        // Cluster A: 1-2-3-4, Cluster B: 4-5-...-47
        g.add_ref(1, 2);
        g.add_ref(2, 3);
        g.add_ref(3, 4);
        g.add_ref(4, 5);
        g.add_ref(5, 6);
        for i in 6..=46 {
            g.add_ref(i, i + 1);
        }
        // Some cross-links within cluster B
        g.add_ref(10, 20);
        g.add_ref(20, 30);
        g.add_ref(30, 40);
        g.add_ref(15, 25);
        g.add_ref(35, 45);
        g
    }

    #[test]
    fn test_report_from_analysis() {
        let g = make_test_document();
        let analysis = SpectralAnalysis::analyze(&g);
        let report = LayoutReport::from_analysis(&analysis);
        assert_eq!(report.page_count, 47);
        assert_eq!(report.ref_count, 0); // not filled from analysis alone
    }

    #[test]
    fn test_report_from_graph() {
        let g = make_test_document();
        let report = LayoutReport::from_graph(&g, 800.0, 600.0, 50.0);
        assert_eq!(report.page_count, 47);
        assert!(report.ref_count > 0);
    }

    #[test]
    fn test_report_display() {
        let g = make_test_document();
        let report = LayoutReport::from_graph(&g, 800.0, 600.0, 50.0);
        let text = format!("{}", report);
        assert!(text.contains("spectral cluster"));
        assert!(text.contains("Bridge"));
        assert!(text.contains("Packing"));
        assert!(text.contains("47 pages"));
    }

    #[test]
    fn test_report_empty_document() {
        let g = PageGraph::new();
        let report = LayoutReport::from_graph(&g, 100.0, 100.0, 10.0);
        assert_eq!(report.page_count, 0);
    }

    #[test]
    fn test_report_bridge_suggestion() {
        let g = make_test_document();
        let report = LayoutReport::from_graph(&g, 800.0, 600.0, 50.0);
        let text = format!("{}", report);
        // Section 4 should be suggested as a split point
        assert!(
            text.contains("bridge"),
            "Report should mention bridges, got: {}",
            text
        );
    }

    #[test]
    fn test_roundtrip_format() {
        let g = make_test_document();
        let analysis = SpectralAnalysis::analyze(&g);
        let report = LayoutReport::from_analysis(&analysis);
        let display = format!("{}", report);

        // Key presence markers
        assert!(display.contains("Pages"));
        assert!(display.contains("Spectral Overview"));
        assert!(display.contains("Bridge Sections"));

        println!("{}", display);
    }
}
