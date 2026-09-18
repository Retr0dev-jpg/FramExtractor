use std::time::{Duration, Instant};

use eframe::egui;

use crate::state::{JobPhase, JobState};

// ── Costanti di animazione ────────────────────────────────────────────────────

/// Durata totale della fase Done prima della rimozione
pub const DONE_TIMEOUT: Duration = Duration::from_millis(3500);
/// Durata dell'animazione slide-out verso destra
pub const SLIDE_DURATION: Duration = Duration::from_millis(400);

// ── Riga job ─────────────────────────────────────────────────────────────────

pub fn draw_job_row(ui: &mut egui::Ui, job: &std::sync::Arc<JobState>, t: f32) {
    let phase = job.phase.lock().unwrap().clone();
    let done = job.frames_done();
    let total = job.frames_total();
    let name = job
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let slide_offset = if let JobPhase::Done(finished_at) = &phase {
        let elapsed = Instant::now().duration_since(*finished_at).as_secs_f32();
        let slide_start = (DONE_TIMEOUT - SLIDE_DURATION).as_secs_f32();
        if elapsed > slide_start {
            let progress =
                ((elapsed - slide_start) / SLIDE_DURATION.as_secs_f32()).clamp(0.0, 1.0);
            progress * progress // ease-in quadratica
        } else {
            0.0
        }
    } else {
        0.0
    };

    let row_height = 52.0;
    let row_width = ui.available_width();

    if slide_offset > 0.0 {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(row_width, row_height),
            egui::Sense::hover(),
        );
        let offset_x = slide_offset * (row_width + 20.0);
        let translated = rect.translate(egui::vec2(offset_x, 0.0));

        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(translated)
                .layout(egui::Layout::left_to_right(egui::Align::TOP)),
        );
        child.set_clip_rect(rect);
        draw_job_row_contents(&mut child, &phase, done, total, &name, t);
    } else {
        ui.horizontal(|ui| {
            draw_job_row_contents(ui, &phase, done, total, &name, t);
        });
    }
}

fn draw_job_row_contents(
    ui: &mut egui::Ui,
    phase: &JobPhase,
    done: u64,
    total: u64,
    name: &str,
    t: f32,
) {
    let icon = match phase {
        JobPhase::Queued => "⏳",
        JobPhase::Extracting => "⚙",
        JobPhase::Done(_) => "✅",
        JobPhase::Error(_) => "❌",
    };
    ui.label(egui::RichText::new(icon).size(16.0));

    ui.vertical(|ui| {
        ui.label(egui::RichText::new(name.to_owned()).strong().size(13.0));

        match phase {
            JobPhase::Queued => {
                ui.add(
                    egui::ProgressBar::new(0.0)
                        .desired_width(ui.available_width() - 8.0)
                        .text("In coda…"),
                );
            }
            JobPhase::Extracting => {
                if total > 0 {
                    let ratio = (done as f32 / total as f32).clamp(0.0, 1.0);
                    ui.add(
                        egui::ProgressBar::new(ratio)
                            .desired_width(ui.available_width() - 8.0)
                            .text(format!("Frame {done} / {total}")),
                    );
                } else {
                    ui.add(
                        egui::ProgressBar::new(t * 0.5 % 1.0)
                            .desired_width(ui.available_width() - 8.0)
                            .text(format!("Frame estratti: {done}")),
                    );
                }
            }
            JobPhase::Done(finished_at) => {
                let elapsed = Instant::now().duration_since(*finished_at).as_secs_f32();
                let _ = elapsed;
                ui.add(
                    egui::ProgressBar::new(1.0)
                        .desired_width(ui.available_width() - 8.0)
                        .text(
                            egui::RichText::new(format!("✓ {done} frame salvati"))
                                .color(egui::Color32::from_rgb(80, 200, 100)),
                        ),
                );
            }
            JobPhase::Error(e) => {
                ui.colored_label(egui::Color32::from_rgb(220, 80, 80), e);
            }
        }
    });
}

// ── Drop zone ─────────────────────────────────────────────────────────────────

pub fn draw_drop_zone(ui: &mut egui::Ui, drag_hover: bool) {
    let available = ui.available_size();
    let (rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());

    let fill = if drag_hover {
        egui::Color32::from_rgba_unmultiplied(80, 160, 255, 30)
    } else {
        egui::Color32::from_rgba_unmultiplied(120, 120, 120, 10)
    };
    let stroke_color = if drag_hover {
        egui::Color32::from_rgb(80, 160, 255)
    } else {
        egui::Color32::from_rgb(100, 100, 100)
    };

    ui.painter().rect(
        rect.shrink(8.0),
        egui::CornerRadius::same(8),
        fill,
        egui::Stroke::new(if drag_hover { 2.0 } else { 1.0 }, stroke_color),
        egui::StrokeKind::Outside,
    );

    let center = rect.center();
    let text = if drag_hover {
        "📂  Rilascia i file qui"
    } else {
        "📂  Trascina video, clicca ➕ o premi Ctrl+V"
    };
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        egui::FontId::proportional(15.0),
        egui::Color32::GRAY,
    );
    ui.painter().galley(
        egui::pos2(
            center.x - galley.size().x / 2.0,
            center.y - galley.size().y / 2.0,
        ),
        galley,
        egui::Color32::GRAY,
    );
}

pub fn draw_drop_zone_compact(ui: &mut egui::Ui, drag_hover: bool) {
    let desired = egui::vec2(ui.available_width(), 40.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());

    let fill = if drag_hover {
        egui::Color32::from_rgba_unmultiplied(80, 160, 255, 30)
    } else {
        egui::Color32::TRANSPARENT
    };
    let stroke_color = if drag_hover {
        egui::Color32::from_rgb(80, 160, 255)
    } else {
        egui::Color32::from_rgb(80, 80, 80)
    };

    ui.painter().rect(
        rect.shrink(2.0),
        egui::CornerRadius::same(6),
        fill,
        egui::Stroke::new(1.0, stroke_color),
        egui::StrokeKind::Outside,
    );

    let galley = ui.painter().layout_no_wrap(
        "  ⬆  Trascina altri video, o Ctrl+V".to_string(),
        egui::FontId::proportional(12.0),
        egui::Color32::DARK_GRAY,
    );
    let center = rect.center();
    ui.painter().galley(
        egui::pos2(
            center.x - galley.size().x / 2.0,
            center.y - galley.size().y / 2.0,
        ),
        galley,
        egui::Color32::DARK_GRAY,
    );
}
