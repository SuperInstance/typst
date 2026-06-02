//! Integration tests for typst-resource-guardian.
//!
//! These tests verify the full integration path: CLI argument parsing,
//! guardian initialisation, and snapshot status output.

use typst_resource_guardian::{parse_budget, Budget, Phase, ResourceGuardian};

/// Test that a complex budget string parses correctly.
#[test]
fn integration_parse_complex_budget() {
    let b = parse_budget("  time:45s , memory:1GB , pages:2500 ").unwrap();
    assert_eq!(b.max_time.as_secs(), 45);
    assert_eq!(b.max_memory_mb, 1024);
    assert_eq!(b.max_pages, 2500);
}

/// Test the full lifecycle: create guardian, record pages, check phases.
#[test]
fn integration_full_lifecycle() {
    let budget = Budget {
        max_time: std::time::Duration::from_secs(3600), // 1h so time won't trigger
        max_memory_mb: 99999,
        max_pages: 5,
    };

    let guardian = ResourceGuardian::new(budget);

    // Initially should not stop.
    assert!(!guardian.should_stop());
    let s = guardian.snapshot();
    assert_eq!(s.time_phase, Phase::Normal);
    assert_eq!(s.memory_phase, Phase::Normal);
    assert_eq!(s.page_phase, Phase::Normal);

    // Record pages.
    guardian.record_page();
    guardian.record_page();
    guardian.record_page();
    guardian.record_page();

    // 4/5 = 80% → Warning.
    let s = guardian.snapshot();
    assert_eq!(s.page_phase, Phase::Warning);
    assert!(!guardian.should_stop());

    guardian.record_page(); // 5/5 = 100% → HardStop
    assert!(guardian.should_stop());
}

/// Test the status string is well-formed.
#[test]
fn integration_status_string() {
    let guardian = ResourceGuardian::new(Budget::default());
    let status = guardian.status();
    assert!(status.starts_with("Resource Guardian:"));
    assert!(status.contains("compile"));
    assert!(status.contains("memory"));
}

/// Test consecutive full lifecycle with conservation counter.
#[test]
fn integration_conservation_counter() {
    let counter = typst_resource_guardian::ConservationCounter::new("pages", 100);
    assert_eq!(counter.label(), "pages");
    assert_eq!(counter.current(), 0);
    assert_eq!(counter.max(), 100);

    counter.add(50);
    assert_eq!(counter.current(), 50);
    assert_eq!(counter.ratio(), 0.5);
    assert_eq!(counter.phase(), Phase::Normal);

    counter.add(35);
    assert_eq!(counter.phase(), Phase::Degraded); // 85/100

    counter.add(15);
    assert_eq!(counter.phase(), Phase::HardStop); // 100/100
}
