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
    i18n,
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
        let path = video_path.display().to_string();
        return Err(i18n::tf("file_not_found", &[("path", &path)]));
    }

    let video_str = video_path.to_string_lossy().to_string();

    let mut child = FfmpegCommand::new_with_path(ffmpeg_exe_path())
        .input(&video_str)
        .rawvideo()
        .spawn()
        .map_err(|e| i18n::tf("ffmpeg_start", &[("error", &e.to_string())]))?;

    let iter = child
        .iter()
        .map_err(|e| i18n::tf("ffmpeg_comm", &[("error", &e.to_string())]))?;

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
                    let idx_s = idx.to_string();
                    if msg.contains("28") || msg.to_lowercase().contains("space") {
                        i18n::tf("disk_full", &[("idx", &idx_s)])
                    } else if msg.to_lowercase().contains("permission")
                        || msg.to_lowercase().contains("access")
                    {
                        let file = format!("frame_{idx:06}.png");
                        i18n::tf("permission_denied", &[("file", &file)])
                    } else {
                        i18n::tf("save_frame", &[("idx", &idx_s), ("error", &msg)])
                    }
                })?;
                ctx.request_repaint();
            }
            FfmpegEvent::Log(ffmpeg_sidecar::event::LogLevel::Fatal, msg) => {
                return Err(i18n::tf("ffmpeg_error", &[("error", &msg)]));
            }
            FfmpegEvent::Error(msg) => {
                return Err(i18n::tf("internal_error", &[("error", &msg)]));
            }
            FfmpegEvent::Done => {
                if !got_video_stream && job.frames_done.load(std::sync::atomic::Ordering::Relaxed) == 0 {
                    return Err(i18n::t("no_frames"));
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
        let path = parent.display().to_string();
        return Err(i18n::tf("parent_missing", &[("path", &path)]));
    }

    let candidate = parent.join(&stem);
    if !candidate.exists() {
        return fs::create_dir_all(&candidate)
            .map(|_| candidate)
            .map_err(|e| {
                i18n::tf(
                    "create_dir",
                    &[("name", stem.as_str()), ("error", &e.to_string())],
                )
            });
    }

    for n in 1u32..=9999 {
        let name = format!("{} ({})", stem, n);
        let candidate = parent.join(&name);
        if !candidate.exists() {
            return fs::create_dir_all(&candidate)
                .map(|_| candidate)
                .map_err(|e| {
                    i18n::tf(
                        "create_dir",
                        &[("name", name.as_str()), ("error", &e.to_string())],
                    )
                });
        }
    }

    Err(i18n::tf("too_many_copies", &[("name", &stem)]))
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
