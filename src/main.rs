#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod calendar;
pub mod position;
pub mod render;
pub mod tray;
pub mod window;

use windows::core::Result;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG,
};

use crate::window::{CalendarWindow, PopupWindowConfig};

/// Windows メッセージループを実行し、`WM_QUIT` 受信時に終了する
pub fn run_message_loop() {
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// アプリケーションを初期化してシステムトレイに常駐させる
pub fn run_app() -> Result<()> {
    let config = PopupWindowConfig::default();
    let mut window = CalendarWindow::new(&config)?;
    let _ = window.reposition_to_tray(config.width, config.height, 12);

    let _tray = window.create_tray_icon("DesCal - カレンダー")?;

    run_message_loop();
    Ok(())
}

fn main() {
    if let Err(err) = run_app() {
        eprintln!("DesCal error: {err}");
    }
}
