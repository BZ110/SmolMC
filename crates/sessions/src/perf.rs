//! Private release-probe telemetry, identical in before/after runs. Never in production.
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use tokio::time::Instant;
pub struct Histogram([AtomicU64; 25]);
impl Histogram {
    pub const fn new() -> Self {
        Self([const { AtomicU64::new(0) }; 25])
    }
    pub fn record(&self, micros: u64) {
        let bucket = if micros <= 1 {
            0
        } else {
            (64 - (micros - 1).leading_zeros() as usize).min(24)
        };
        self.0[bucket].fetch_add(1, Relaxed);
    }
    fn values(&self) -> Vec<u64> {
        self.0.iter().map(|v| v.load(Relaxed)).collect()
    }
}
pub static COLLISION: Histogram = Histogram::new();
pub static BLOCK_COMMIT: Histogram = Histogram::new();
pub static BLOCK_WRITE: Histogram = Histogram::new();
pub static ACTION_PROCESS: Histogram = Histogram::new();
pub static CRITICAL_WRITE: Histogram = Histogram::new();
pub static DIRECTORY: Histogram = Histogram::new();
pub static FLUSH: Histogram = Histogram::new();
pub static AGE: Histogram = Histogram::new();
pub static FANOUT: AtomicU64 = AtomicU64::new(0);
pub static POSITIONS: AtomicU64 = AtomicU64::new(0);
pub struct Timer(&'static Histogram, Instant);
impl Timer {
    pub fn new(h: &'static Histogram) -> Self {
        Self(h, Instant::now())
    }
}
impl Drop for Timer {
    fn drop(&mut self) {
        self.0.record(self.1.elapsed().as_micros() as u64);
    }
}
pub fn snapshot() -> serde_json::Value {
    serde_json::json!({"collision_processing_us":COLLISION.values(),"block_commit_us":BLOCK_COMMIT.values(),"block_enqueue_to_write_us":BLOCK_WRITE.values(),"action_to_processing_us":ACTION_PROCESS.values(),"combat_event_to_write_us":CRITICAL_WRITE.values(),"directory_us":DIRECTORY.values(),"flush_us":FLUSH.values(),"publish_to_write_us_sampled":AGE.values(),"fanout_versions":FANOUT.load(Relaxed),"positions":POSITIONS.load(Relaxed)})
}
