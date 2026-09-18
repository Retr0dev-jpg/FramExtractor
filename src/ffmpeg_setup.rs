use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};

use eframe::egui;
use ffmpeg_sidecar::download::{
    download_ffmpeg_package_with_progress, ffmpeg_download_url, unpack_ffmpeg,
    FfmpegDownloadProgressEvent,
};

use crate::state::{AppPhase, AppState};

// ── Path helpers ──────────────────────────────────────────────────────────────

pub fn ffmpeg_temp_dir() -> PathBuf {
    std::env::temp_dir().join("FramExtractor").join("ffmpeg")
}

pub fn ffmpeg_exe_path() -> PathBuf {
    ffmpeg_temp_dir().join("ffmpeg.exe")
}

pub fn ffmpeg_installed() -> bool {
    ffmpeg_exe_path().exists()
}

// ── Download all'avvio ────────────────────────────────────────────────────────

pub fn startup_download_ffmpeg(
    state: Arc<AppState>,
    done_flag: Arc<AtomicBool>,
    ctx: egui::Context,
) {
    thread::spawn(move || {
        if ffmpeg_installed() {
            *state.app_phase.lock().unwrap() = AppPhase::Ready;
            done_flag.store(true, Ordering::Relaxed);
            ctx.request_repaint();
            return;
        }

        *state.app_phase.lock().unwrap() = AppPhase::StartupDownload;
        ctx.request_repaint();

        let dest_dir = ffmpeg_temp_dir();
        if let Err(e) = fs::create_dir_all(&dest_dir) {
            *state.app_phase.lock().unwrap() =
                AppPhase::StartupError(format!("Impossibile creare la cartella temp: {e}"));
            done_flag.store(true, Ordering::Relaxed);
            ctx.request_repaint();
            return;
        }

        let url = match ffmpeg_download_url() {
            Ok(u) => u,
            Err(e) => {
                *state.app_phase.lock().unwrap() =
                    AppPhase::StartupError(format!("URL download non disponibile: {e}"));
                done_flag.store(true, Ordering::Relaxed);
                ctx.request_repaint();
                return;
            }
        };

        let state_cb = Arc::clone(&state);
        let ctx_cb = ctx.clone();

        let archive_result =
            download_ffmpeg_package_with_progress(url, &dest_dir, move |event| {
                if let FfmpegDownloadProgressEvent::Downloading {
                    total_bytes,
                    downloaded_bytes,
                } = event
                {
                    state_cb.dl_total.store(total_bytes, Ordering::Relaxed);
                    state_cb.dl_downloaded.store(downloaded_bytes, Ordering::Relaxed);
                }
                ctx_cb.request_repaint();
            });

        let archive_path = match archive_result {
            Ok(p) => p,
            Err(e) => {
                *state.app_phase.lock().unwrap() =
                    AppPhase::StartupError(format!("Errore download: {e}"));
                done_flag.store(true, Ordering::Relaxed);
                ctx.request_repaint();
                return;
            }
        };

        *state.app_phase.lock().unwrap() = AppPhase::StartupUnpack;
        ctx.request_repaint();

        if let Err(e) = unpack_ffmpeg(&archive_path, &dest_dir) {
            *state.app_phase.lock().unwrap() =
                AppPhase::StartupError(format!("Errore estrazione archivio: {e}"));
            done_flag.store(true, Ordering::Relaxed);
            ctx.request_repaint();
            return;
        }

        *state.app_phase.lock().unwrap() = if ffmpeg_installed() {
            AppPhase::Ready
        } else {
            AppPhase::StartupError(
                "ffmpeg.exe non trovato dopo l'installazione.".to_string(),
            )
        };

        done_flag.store(true, Ordering::Relaxed);
        ctx.request_repaint();
    });
}
