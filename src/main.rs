#![windows_subsystem = "windows"]

mod ffmpeg_setup;
mod state;
mod ui;
mod worker;

use std::sync::{atomic::AtomicBool, Arc};

use eframe::egui;
use ffmpeg_setup::startup_download_ffmpeg;
use state::AppState;
use ui::App;

fn main() -> eframe::Result<()> {
    let app_state = AppState::new();
    let startup_done = Arc::new(AtomicBool::new(false));

    let win_size = [480.0_f32, 220.0_f32];
    let center_pos = centered_position(win_size[0], win_size[1]);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("FramExtractor")
            .with_inner_size(win_size)
            .with_position(center_pos)
            .with_resizable(true)
            .with_drag_and_drop(true),
        ..Default::default()
    };

    let state_clone = Arc::clone(&app_state);
    let done_clone = Arc::clone(&startup_done);

    eframe::run_native(
        "FramExtractor",
        options,
        Box::new(move |cc| {
            startup_download_ffmpeg(
                Arc::clone(&state_clone),
                Arc::clone(&done_clone),
                cc.egui_ctx.clone(),
            );
            Ok(Box::new(App::new(state_clone, done_clone)))
        }),
    )
}

/// Calcola la posizione di partenza per centrare la finestra sul monitor principale.
fn centered_position(win_w: f32, win_h: f32) -> egui::Pos2 {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
        };
        let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) } as f32;
        let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) } as f32;
        egui::pos2(
            ((screen_w - win_w) / 2.0).max(0.0),
            ((screen_h - win_h) / 2.0).max(0.0),
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        egui::pos2(100.0, 100.0)
    }
}
