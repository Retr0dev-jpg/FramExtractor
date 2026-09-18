pub mod widgets;

use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::Duration,
};

use eframe::egui;

use crate::{
    state::{AppPhase, AppState, JobPhase, JobState},
    worker::run_job,
};

// ── FilePicker: dialog asincrono su thread dedicato ──────────────────────────

pub struct FilePicker {
    rx: mpsc::Receiver<Vec<PathBuf>>,
    open: Arc<AtomicBool>,
}

impl FilePicker {
    pub fn new() -> Self {
        let (_dead_tx, rx) = mpsc::channel::<Vec<PathBuf>>();
        Self {
            rx,
            open: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn open(&mut self, ctx: egui::Context) {
        if self.open.load(Ordering::Relaxed) {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.rx = rx;
        self.open.store(true, Ordering::Relaxed);
        let flag = Arc::clone(&self.open);
        thread::spawn(move || {
            let paths = rfd::FileDialog::new()
                .set_title("Seleziona uno o più video")
                .add_filter(
                    "Video",
                    &["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "ts", "3gp"],
                )
                .add_filter("Tutti i file", &["*"])
                .pick_files()
                .unwrap_or_default();
            let _ = tx.send(paths);
            flag.store(false, Ordering::Relaxed);
            ctx.request_repaint();
        });
    }

    pub fn poll(&self) -> Option<Vec<PathBuf>> {
        self.rx.try_recv().ok().filter(|v| !v.is_empty())
    }

    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Relaxed)
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub state: Arc<AppState>,
    pub startup_done: Arc<AtomicBool>,
    pub drag_hover: bool,
    pub picker: FilePicker,
    pub pinned: bool,
}

impl App {
    pub fn new(state: Arc<AppState>, startup_done: Arc<AtomicBool>) -> Self {
        Self {
            state,
            startup_done,
            drag_hover: false,
            picker: FilePicker::new(),
            pinned: false,
        }
    }

    fn enqueue_job(&self, path: PathBuf, ctx: egui::Context) {
        let job = JobState::new(path);
        self.state.jobs.lock().unwrap().push(Arc::clone(&job));
        thread::spawn(move || run_job(job, ctx));
    }

    fn pick_and_enqueue(&mut self, ctx: egui::Context) {
        self.picker.open(ctx);
    }
}

// ── impl eframe::App ─────────────────────────────────────────────────────────

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        use std::time::Instant;

        let now = Instant::now();

        // ── Pulizia job completati da più di DONE_TIMEOUT ─────────────────────
        {
            let mut jobs = self.state.jobs.lock().unwrap();
            jobs.retain(|j| {
                let p = j.phase.lock().unwrap().clone();
                match p {
                    JobPhase::Done(t) => now.duration_since(t) < widgets::DONE_TIMEOUT,
                    _ => true,
                }
            });
        }

        // ── Repaint continuo se necessario ────────────────────────────────────
        let has_active = {
            let jobs = self.state.jobs.lock().unwrap();
            jobs.iter().any(|j| {
                let p = j.phase.lock().unwrap().clone();
                matches!(p, JobPhase::Queued | JobPhase::Extracting | JobPhase::Done(_))
            })
        };
        let startup_busy = !self.startup_done.load(Ordering::Relaxed);
        if has_active || startup_busy || self.picker.is_open() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }

        // ── Risultati file dialog ─────────────────────────────────────────────
        if let Some(picked) = self.picker.poll() {
            let app_phase = self.state.app_phase.lock().unwrap().clone();
            if matches!(app_phase, AppPhase::Ready) {
                for path in picked {
                    if is_video(&path) {
                        self.enqueue_job(path, ctx.clone());
                    }
                }
            }
        }

        // ── Ctrl+V ────────────────────────────────────────────────────────────
        let paste_triggered = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::V));
        if paste_triggered {
            let app_phase = self.state.app_phase.lock().unwrap().clone();
            if matches!(app_phase, AppPhase::Ready) {
                if let Some(text) = ctx.input(|i| {
                    i.events.iter().find_map(|e| {
                        if let egui::Event::Paste(s) = e {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                }) {
                    for line in text.lines() {
                        let p = PathBuf::from(line.trim().trim_matches('"'));
                        if p.exists() && is_video(&p) {
                            self.enqueue_job(p, ctx.clone());
                        }
                    }
                }
            }
        }

        // ── Drag & drop ───────────────────────────────────────────────────────
        let app_phase = self.state.app_phase.lock().unwrap().clone();
        let files_hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        self.drag_hover = files_hovering && matches!(app_phase, AppPhase::Ready);

        if files_hovering {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        let dropped_paths: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .filter(|p| is_video(p))
                .collect()
        });
        if !dropped_paths.is_empty() && matches!(app_phase, AppPhase::Ready) {
            for path in dropped_paths {
                self.enqueue_job(path, ctx.clone());
            }
        }

        // ── Render ────────────────────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render(ui, &app_phase, ctx);
        });
    }
}

impl App {
    fn render(&mut self, ui: &mut egui::Ui, app_phase: &AppPhase, ctx: &egui::Context) {
        use std::sync::atomic::Ordering;

        match app_phase {
            AppPhase::StartupDownload | AppPhase::StartupUnpack => {
                ui.vertical_centered(|ui| {
                    ui.add_space(30.0);
                    ui.heading("FramExtractor");
                    ui.add_space(16.0);

                    if matches!(app_phase, AppPhase::StartupDownload) {
                        let downloaded = self.state.dl_downloaded.load(Ordering::Relaxed);
                        let total = self.state.dl_total.load(Ordering::Relaxed);
                        ui.label("Scarico ffmpeg (solo al primo avvio)…");
                        ui.add_space(8.0);
                        if total > 0 {
                            let ratio = (downloaded as f32 / total as f32).clamp(0.0, 1.0);
                            ui.add(
                                egui::ProgressBar::new(ratio)
                                    .desired_width(360.0)
                                    .text(format!(
                                        "{:.1} MB / {:.1} MB",
                                        downloaded as f64 / 1_048_576.0,
                                        total as f64 / 1_048_576.0
                                    )),
                            );
                        } else {
                            let t = ui.input(|i| i.time) as f32;
                            ui.add(
                                egui::ProgressBar::new(t * 0.3 % 1.0)
                                    .desired_width(360.0)
                                    .text("Connessione…"),
                            );
                        }
                    } else {
                        ui.label("Estrazione archivio ffmpeg…");
                        ui.add_space(8.0);
                        let t = ui.input(|i| i.time) as f32;
                        ui.add(
                            egui::ProgressBar::new(t * 0.4 % 1.0)
                                .desired_width(360.0)
                                .text("Unpack…"),
                        );
                    }
                });
            }

            AppPhase::StartupError(err) => {
                ui.vertical_centered(|ui| {
                    ui.add_space(30.0);
                    ui.heading("FramExtractor");
                    ui.add_space(20.0);
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 80, 80),
                        format!("❌ Errore durante l'avvio:\n{err}"),
                    );
                });
            }

            AppPhase::Ready => {
                let jobs_snap: Vec<Arc<JobState>> = self.state.jobs.lock().unwrap().clone();

                // ── Header ────────────────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.heading("FramExtractor");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("  ➕  Aggiungi…  ").clicked() {
                            self.pick_and_enqueue(ctx.clone());
                        }
                        ui.add_space(4.0);

                        let pin_label = if self.pinned { "📌" } else { "📍" };
                        let pin_btn = ui.button(pin_label).on_hover_text(if self.pinned {
                            "Sblocca dalla prima piano"
                        } else {
                            "Blocca in primo piano"
                        });
                        if pin_btn.clicked() {
                            self.pinned = !self.pinned;
                            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                                if self.pinned {
                                    egui::WindowLevel::AlwaysOnTop
                                } else {
                                    egui::WindowLevel::Normal
                                },
                            ));
                        }
                    });
                });

                ui.separator();

                if jobs_snap.is_empty() {
                    widgets::draw_drop_zone(ui, self.drag_hover);
                } else {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for job in &jobs_snap {
                                widgets::draw_job_row(ui, job, ui.input(|i| i.time) as f32);
                                ui.separator();
                            }
                            ui.add_space(6.0);
                            widgets::draw_drop_zone_compact(ui, self.drag_hover);
                        });
                }
            }
        }
    }
}

// ── Helper ────────────────────────────────────────────────────────────────────

pub fn is_video(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "ts" | "3gp")
    )
}
