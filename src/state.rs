use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

// ── Job ───────────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
pub enum JobPhase {
    Queued,
    Extracting,
    /// Completato; contiene l'istante di fine per l'animazione slide-out
    Done(Instant),
    Error(String),
}

pub struct JobState {
    pub path: PathBuf,
    pub phase: Mutex<JobPhase>,
    pub frames_done: AtomicU64,
    pub frames_total: AtomicU64,
}

impl JobState {
    pub fn new(path: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            path,
            phase: Mutex::new(JobPhase::Queued),
            frames_done: AtomicU64::new(0),
            frames_total: AtomicU64::new(0),
        })
    }

    pub fn frames_done(&self) -> u64 {
        self.frames_done.load(Ordering::Relaxed)
    }

    pub fn frames_total(&self) -> u64 {
        self.frames_total.load(Ordering::Relaxed)
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
pub enum AppPhase {
    StartupDownload,
    StartupUnpack,
    Ready,
    StartupError(String),
}

pub struct AppState {
    pub app_phase: Mutex<AppPhase>,
    pub dl_downloaded: AtomicU64,
    pub dl_total: AtomicU64,
    pub jobs: Mutex<Vec<Arc<JobState>>>,
}

impl AppState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            app_phase: Mutex::new(AppPhase::StartupDownload),
            dl_downloaded: AtomicU64::new(0),
            dl_total: AtomicU64::new(0),
            jobs: Mutex::new(Vec::new()),
        })
    }
}
