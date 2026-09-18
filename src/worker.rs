use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use eframe::egui;
use ffmpeg_sidecar::{
    command::FfmpegCommand,
    event::{FfmpegEvent, FfmpegProgress, OutputVideoFrame, StreamTypeSpecificData},
};

use crate::{
    ffmpeg_setup::ffmpeg_exe_path,
    state::{JobPhase, JobState},
};

// ── Entrypoint del thread worker ─────────────────────────────────────────────

pub fn run_job(job: Arc<JobState>, ctx: egui::Context) {
    let path = job.path.clone();

    let output_dir = match resolve_output_dir(&path) {
        Ok(d) => d,
        Err(e) => {
            *job.phase.lock().unwrap() = JobPhase::Error(e);
            ctx.request_repaint();
            return;
        }
    };

    *job.phase.lock().unwrap() = JobPhase::Extracting;
    ctx.request_repaint();

    if let Err(e) = extract_frames(&path, &output_dir, &job, &ctx) {
        let _ = fs::remove_dir_all(&output_dir);
        *job.phase.lock().unwrap() = JobPhase::Error(e);
        ctx.request_repaint();
        return;
    }

    *job.phase.lock().unwrap() = JobPhase::Done(Instant::now());
    ctx.request_repaint();
}

// ── Estrazione frame ──────────────────────────────────────────────────────────

fn extract_frames(
    video_path: &Path,
    output_dir: &Path,
    job: &JobState,
    ctx: &egui::Context,
) -> Result<(), String> {
    if !video_path.exists() {
        return Err(format!("File non trovato: {}", video_path.display()));
    }

    let video_str = video_path.to_string_lossy().to_string();

    let mut child = FfmpegCommand::new_with_path(ffmpeg_exe_path())
        .input(&video_str)
        .rawvideo()
        .spawn()
        .map_err(|e| format!("Impossibile avviare ffmpeg: {e}"))?;

    let iter = child
        .iter()
        .map_err(|e| format!("Errore comunicazione ffmpeg: {e}"))?;

    let mut duration_secs: Option<f64> = None;
    let mut got_video_stream = false;

    for event in iter {
        match event {
            FfmpegEvent::ParsedDuration(d) => {
                duration_secs = Some(d.duration);
            }
            FfmpegEvent::ParsedInputStream(stream) => {
                if let StreamTypeSpecificData::Video(v) = &stream.type_specific_data {
                    got_video_stream = true;
                    if v.fps > 0.0 {
                        if let Some(dur) = duration_secs {
                            let total = (v.fps as f64 * dur).round() as u64;
                            job.frames_total.store(total, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                }
            }
            FfmpegEvent::Progress(FfmpegProgress { frame, .. }) => {
                job.frames_done.store(frame as u64, std::sync::atomic::Ordering::Relaxed);
                ctx.request_repaint();
            }
            FfmpegEvent::OutputFrame(OutputVideoFrame {
                data,
                width,
                height,
                frame_num,
                ..
            }) => {
                let idx = frame_num as u64 + 1;
                job.frames_done.store(idx, std::sync::atomic::Ordering::Relaxed);
                let file_path = output_dir.join(format!("frame_{:06}.png", idx));
                save_png(&data, width, height, &file_path).map_err(|e| {
                    let msg = e.to_string();
                    if msg.contains("28") || msg.to_lowercase().contains("space") {
                        format!("Disco pieno durante il salvataggio del frame {idx}")
                    } else if msg.to_lowercase().contains("permission")
                        || msg.to_lowercase().contains("access")
                    {
                        format!("Permesso negato per frame_{idx:06}.png")
                    } else {
                        format!("Errore salvataggio frame {idx}: {msg}")
                    }
                })?;
                ctx.request_repaint();
            }
            FfmpegEvent::Log(ffmpeg_sidecar::event::LogLevel::Fatal, msg) => {
                return Err(format!("Errore ffmpeg: {msg}"));
            }
            FfmpegEvent::Error(msg) => {
                return Err(format!("Errore interno: {msg}"));
            }
            FfmpegEvent::Done => {
                if !got_video_stream && job.frames_done.load(std::sync::atomic::Ordering::Relaxed) == 0 {
                    return Err(
                        "Nessun frame estratto: il file potrebbe non contenere video, \
                         essere corrotto o usare un codec non supportato."
                            .to_string(),
                    );
                }
            }
            _ => {}
        }
    }

    Ok(())
}

// ── Risoluzione cartella output con suffisso (N) stile Windows ───────────────

pub fn resolve_output_dir(video_path: &Path) -> Result<PathBuf, String> {
    let stem = video_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let parent = video_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    if !parent.exists() {
        return Err(format!(
            "La cartella del video non esiste: {}",
            parent.display()
        ));
    }

    let candidate = parent.join(&stem);
    if !candidate.exists() {
        return fs::create_dir_all(&candidate)
            .map(|_| candidate)
            .map_err(|e| format!("Impossibile creare \"{}\": {}", stem, e));
    }

    for n in 1u32..=9999 {
        let name = format!("{} ({})", stem, n);
        let candidate = parent.join(&name);
        if !candidate.exists() {
            return fs::create_dir_all(&candidate)
                .map(|_| candidate)
                .map_err(|e| format!("Impossibile creare \"{}\": {}", name, e));
        }
    }

    Err(format!(
        "Impossibile trovare un nome disponibile per \"{}\" (troppe copie esistenti)",
        stem
    ))
}

// ── Salvataggio PNG ───────────────────────────────────────────────────────────

fn save_png(
    rgb_data: &[u8],
    width: u32,
    height: u32,
    path: &Path,
) -> Result<(), png::EncodingError> {
    let file = fs::File::create(path).map_err(png::EncodingError::IoError)?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgb_data)?;
    Ok(())
}
