//! Resource Guardian — compilation budgets for Typst.
//!
//! Monitors compilation resource consumption (CPU time, memory, font cache)
//! and enforces user-defined budgets with phased escalation:
//!   - 70%  → warning
//!   - 85%  → degraded rendering
//!   - 100% → hard stop with partial output
//!
//! Per-chapter tracking identifies which sections are expensive.
//! Incremental compilation detection flags full recompilation triggers.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Budget specification
// ---------------------------------------------------------------------------

/// Resource budgets parsed from CLI `--budget=time:30s,memory:500MB,pages:1000`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    /// Maximum wall-clock compile time (default 30 s).
    pub max_time: Duration,
    /// Maximum estimated resident memory (default 500 MB).
    pub max_memory_mb: u64,
    /// Maximum number of pages (default 1000).
    pub max_pages: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            max_time: Duration::from_secs(30),
            max_memory_mb: 500,
            max_pages: 1000,
        }
    }
}

/// Parse a `--budget` value string.
///
/// Supported units: `s`, `ms` for time; `MB`, `GB` for memory; bare number for pages.
///
/// # Examples
///
/// ```
/// let b = typst_resource_guardian::parse_budget("time:30s,memory:500MB,pages:1000").unwrap();
/// assert_eq!(b.max_time.as_secs(), 30);
/// ```
pub fn parse_budget(s: &str) -> Result<Budget, String> {
    let mut b = Budget::default();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, val) = part
            .split_once(':')
            .ok_or_else(|| format!("budget entry must be key:value, got {part:?}"))?;
        match key.trim().to_lowercase().as_str() {
            "time" => {
                let v = val.trim();
                if let Some(secs) = v.strip_suffix("ms") {
                    let ms: u64 = secs
                        .parse()
                        .map_err(|e| format!("invalid time value {v}: {e}"))?;
                    b.max_time = Duration::from_millis(ms);
                } else if let Some(secs) = v.strip_suffix('s') {
                    let s: u64 = secs
                        .parse()
                        .map_err(|e| format!("invalid time value {v}: {e}"))?;
                    b.max_time = Duration::from_secs(s);
                } else {
                    let s: f64 = v
                        .parse()
                        .map_err(|e| format!("invalid time value {v}: {e}"))?;
                    b.max_time = Duration::from_secs_f64(s);
                }
            }
            "memory" => {
                let v = val.trim();
                if let Some(mb) = v.strip_suffix("MB") {
                    b.max_memory_mb = mb
                        .parse()
                        .map_err(|e| format!("invalid memory value {v}: {e}"))?;
                } else if let Some(gb) = v.strip_suffix("GB") {
                    let gb_val: f64 = gb
                        .parse()
                        .map_err(|e| format!("invalid memory value {v}: {e}"))?;
                    b.max_memory_mb = (gb_val * 1024.0) as u64;
                } else {
                    b.max_memory_mb = v
                        .parse()
                        .map_err(|e| format!("invalid memory value {v}: {e}"))?;
                }
            }
            "pages" => {
                b.max_pages = val
                    .trim()
                    .parse()
                    .map_err(|e| format!("invalid pages value: {e}"))?;
            }
            other => {
                return Err(format!("unknown budget key {other:?} (expected time, memory, pages)"));
            }
        }
    }
    Ok(b)
}

// ---------------------------------------------------------------------------
// Phase detection & escalation
// ---------------------------------------------------------------------------

/// Escalation phase determined by resource consumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    /// Under 70% — normal compilation.
    Normal,
    /// 70–84% — warning phase.
    Warning,
    /// 85–99% — degraded rendering (graceful quality reduction).
    Degraded,
    /// 100%+ — hard stop, produce partial output.
    HardStop,
}

impl Phase {
    /// Determine the current phase from a ratio in [0.0, …).
    pub fn from_ratio(ratio: f64) -> Self {
        if ratio >= 1.0 {
            Self::HardStop
        } else if ratio >= 0.85 {
            Self::Degraded
        } else if ratio >= 0.70 {
            Self::Warning
        } else {
            Self::Normal
        }
    }
}

// ---------------------------------------------------------------------------
// Per-chapter tracking
// ---------------------------------------------------------------------------

/// Statistics for a single chapter / section of the document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterStats {
    /// Chapter name or identifier.
    pub name: String,
    /// Cumulative CPU time (approximate wall-clock) spent on this chapter.
    pub cpu_time: Duration,
    /// Estimated memory delta during this chapter.
    pub memory_delta_kb: u64,
    /// Number of pages in this chapter.
    pub pages: usize,
}

/// Overall resource snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    /// Elapsed compile time so far.
    pub elapsed: Duration,
    /// Estimated RSS in KB (best-effort, platform-dependent).
    pub memory_kb: u64,
    /// Number of pages emitted so far.
    pub pages: usize,
    /// Current font cache entry count (approximate).
    pub font_cache_entries: usize,
    /// Phase for elapsed time.
    pub time_phase: Phase,
    /// Phase for memory.
    pub memory_phase: Phase,
    /// Phase for page count.
    pub page_phase: Phase,
    /// Per-chapter breakdown.
    pub chapters: Vec<ChapterStats>,
    /// Whether this compilation appears to be a full recompilation.
    pub is_full_recompilation: bool,
}

// ---------------------------------------------------------------------------
// Guardian state holder
// ---------------------------------------------------------------------------

/// The central resource guardian that tracks and enforces budgets.
///
/// This is designed to be shared across compilation boundaries via `Arc`.
pub struct ResourceGuardian {
    budget: Budget,
    start: Instant,
    chapters: Mutex<Vec<ChapterStats>>,
    current_chapter: Mutex<Option<String>>,
    chapter_start: Mutex<Instant>,
    pages_compiled: AtomicU64,
    memory_peak_kb: AtomicU64,
    font_cache_size: AtomicU64,
    full_recompile_flag: AtomicBool,
}

// Use parking_lot-style Mutex or std — we just need simple interior mutability.
// To keep deps minimal, we use std::sync::Mutex.
use std::sync::Mutex;

impl ResourceGuardian {
    /// Create a new guardian with the given budget.
    pub fn new(budget: Budget) -> Arc<Self> {
        Arc::new(Self {
            budget,
            start: Instant::now(),
            chapters: Mutex::new(Vec::new()),
            current_chapter: Mutex::new(None),
            chapter_start: Mutex::new(Instant::now()),
            pages_compiled: AtomicU64::new(0),
            memory_peak_kb: AtomicU64::new(0),
            font_cache_size: AtomicU64::new(0),
            full_recompile_flag: AtomicBool::new(false),
        })
    }

    /// Access the budget.
    pub fn budget(&self) -> &Budget {
        &self.budget
    }

    /// Record that a page was compiled.
    pub fn record_page(&self) {
        self.pages_compiled.fetch_add(1, Ordering::Relaxed);
    }

    /// Update the peak memory estimate (KB).
    pub fn update_memory(&self, kb: u64) {
        let prev = self.memory_peak_kb.load(Ordering::Relaxed);
        if kb > prev {
            self.memory_peak_kb.store(kb, Ordering::Relaxed);
        }
    }

    /// Approximate the font cache size and record it.
    pub fn update_font_cache(&self, entries: u64) {
        self.font_cache_size.store(entries, Ordering::Relaxed);
    }

    /// Mark this compilation as triggering a full recompilation.
    pub fn mark_full_recompile(&self) {
        self.full_recompile_flag.store(true, Ordering::Relaxed);
    }

    /// Begin a new chapter.
    pub fn begin_chapter(&self, name: &str) {
        let mut cur = self.current_chapter.lock().unwrap();
        *cur = Some(name.to_string());
        *self.chapter_start.lock().unwrap() = Instant::now();
    }

    /// End the current chapter and record its stats.
    pub fn end_chapter(&self) {
        let chapter = self.current_chapter.lock().unwrap().take();
        let elapsed = self.chapter_start.lock().unwrap().elapsed();
        if let Some(name) = chapter {
            let mut chapters = self.chapters.lock().unwrap();
            chapters.push(ChapterStats {
                name,
                cpu_time: elapsed,
                memory_delta_kb: 0, // delta difficult without a baseline; set to 0, memory is tracked globally
                pages: self.pages_compiled.load(Ordering::Relaxed) as usize,
            });
        }
    }

    /// Take a resource snapshot and check escalation.
    pub fn snapshot(&self) -> ResourceSnapshot {
        let elapsed = self.start.elapsed();
        let memory_kb = self.memory_peak_kb.load(Ordering::Relaxed);
        let pages = self.pages_compiled.load(Ordering::Relaxed) as usize;
        let font_entries = self.font_cache_size.load(Ordering::Relaxed) as usize;

        let time_ratio = elapsed.as_secs_f64() / self.budget.max_time.as_secs_f64();
        let mem_ratio = memory_kb as f64 / (self.budget.max_memory_mb as f64 * 1024.0);
        let page_ratio = pages as f64 / self.budget.max_pages as f64;

        let chapters = self.chapters.lock().unwrap().clone();
        let is_full_recomp = self.full_recompile_flag.load(Ordering::Relaxed);

        ResourceSnapshot {
            elapsed,
            memory_kb,
            pages,
            font_cache_entries: font_entries,
            time_phase: Phase::from_ratio(time_ratio),
            memory_phase: Phase::from_ratio(mem_ratio),
            page_phase: Phase::from_ratio(page_ratio),
            chapters,
            is_full_recompilation: is_full_recomp,
        }
    }

    /// Check whether compilation should be stopped (hard-stop phase reached).
    /// Returns `true` if the caller should abort.
    pub fn should_stop(&self) -> bool {
        let s = self.snapshot();
        s.time_phase == Phase::HardStop
            || s.memory_phase == Phase::HardStop
            || s.page_phase == Phase::HardStop
    }

    /// Return a human-readable status string.
    pub fn status(&self) -> String {
        let s = self.snapshot();
        format!(
            "Resource Guardian: {:.1}s/{}s compile, {}MB/{}MB memory, {} pages/{} pages, fonts: {} \
             | time: {:?}, memory: {:?}, pages: {:?}{}",
            s.elapsed.as_secs_f64(),
            self.budget.max_time.as_secs_f64(),
            s.memory_kb / 1024,
            self.budget.max_memory_mb,
            s.pages,
            self.budget.max_pages,
            s.font_cache_entries,
            s.time_phase,
            s.memory_phase,
            s.page_phase,
            if s.is_full_recompilation {
                " [FULL RECOMPILE]"
            } else {
                ""
            },
        )
    }
}

// ---------------------------------------------------------------------------
// Convenience module
// ---------------------------------------------------------------------------

/// Provides the conservation-checker pattern for resource tracking.
/// A one-sided counter that only increments (tracking cumulative resource use).
pub struct ConservationCounter {
    value: AtomicU64,
    label: String,
    max: u64,
}

impl ConservationCounter {
    pub fn new(label: &str, max: u64) -> Self {
        Self {
            value: AtomicU64::new(0),
            label: label.to_string(),
            max,
        }
    }

    /// Add an amount to the counter.
    pub fn add(&self, amount: u64) {
        self.value.fetch_add(amount, Ordering::Relaxed);
    }

    /// Get the current consumption as a ratio in [0, 1+).
    pub fn ratio(&self) -> f64 {
        let v = self.value.load(Ordering::Relaxed);
        if self.max == 0 {
            return 0.0;
        }
        v as f64 / self.max as f64
    }

    /// Current phase based on this counter.
    pub fn phase(&self) -> Phase {
        Phase::from_ratio(self.ratio())
    }

    /// Label for display.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Current absolute value.
    pub fn current(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Maximum allowed.
    pub fn max(&self) -> u64 {
        self.max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_budget_defaults() {
        let b = parse_budget("time:30s,memory:500MB,pages:1000").unwrap();
        assert_eq!(b.max_time, Duration::from_secs(30));
        assert_eq!(b.max_memory_mb, 500);
        assert_eq!(b.max_pages, 1000);
    }

    #[test]
    fn test_parse_budget_partial() {
        let b = parse_budget("time:10s").unwrap();
        assert_eq!(b.max_time, Duration::from_secs(10));
        assert_eq!(b.max_memory_mb, 500); // default
        assert_eq!(b.max_pages, 1000); // default
    }

    #[test]
    fn test_parse_budget_ms() {
        let b = parse_budget("time:500ms").unwrap();
        assert_eq!(b.max_time, Duration::from_millis(500));
    }

    #[test]
    fn test_parse_budget_gb() {
        let b = parse_budget("memory:2GB").unwrap();
        assert_eq!(b.max_memory_mb, 2048);
    }

    #[test]
    fn test_parse_budget_invalid_key() {
        assert!(parse_budget("foo:bar").is_err());
    }

    #[test]
    fn test_phase_from_ratio() {
        assert_eq!(Phase::from_ratio(0.0), Phase::Normal);
        assert_eq!(Phase::from_ratio(0.69), Phase::Normal);
        assert_eq!(Phase::from_ratio(0.70), Phase::Warning);
        assert_eq!(Phase::from_ratio(0.84), Phase::Warning);
        assert_eq!(Phase::from_ratio(0.85), Phase::Degraded);
        assert_eq!(Phase::from_ratio(0.99), Phase::Degraded);
        assert_eq!(Phase::from_ratio(1.0), Phase::HardStop);
        assert_eq!(Phase::from_ratio(2.0), Phase::HardStop);
    }

    #[test]
    fn test_guardian_page_budget() {
        let budget = Budget { max_pages: 3, ..Budget::default() };
        let g = ResourceGuardian::new(budget);
        assert!(!g.should_stop());
        g.record_page();
        g.record_page();
        assert!(!g.should_stop());
        g.record_page();
        assert!(g.should_stop()); // 3 >= 3 → HardStop
    }

    #[test]
    fn test_conservation_counter() {
        let counter = ConservationCounter::new("test", 100);
        assert_eq!(counter.ratio(), 0.0);
        assert_eq!(counter.phase(), Phase::Normal);
        counter.add(70);
        assert_eq!(counter.phase(), Phase::Warning);
        counter.add(15);
        assert_eq!(counter.phase(), Phase::Degraded);
        counter.add(15);
        assert_eq!(counter.phase(), Phase::HardStop);
    }

    #[test]
    fn test_chapter_tracking() {
        let g = ResourceGuardian::new(Budget::default());
        g.begin_chapter("Introduction");
        g.end_chapter();
        g.begin_chapter("Methods");
        g.end_chapter();
        let s = g.snapshot();
        assert_eq!(s.chapters.len(), 2);
        assert_eq!(s.chapters[0].name, "Introduction");
        assert_eq!(s.chapters[1].name, "Methods");
    }

    #[test]
    fn test_full_recompile_flag() {
        let g = ResourceGuardian::new(Budget::default());
        assert!(!g.snapshot().is_full_recompilation);
        g.mark_full_recompile();
        assert!(g.snapshot().is_full_recompilation);
    }
}
