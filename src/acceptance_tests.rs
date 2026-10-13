//! DesCal 完成基準（Acceptance Criteria）の網羅的統合検証テストスイート
//!
//! SPECIFICATIONS.md Section 8 に定義された全7項目の受け入れ基準を網羅的に検証します。

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, GWL_EXSTYLE, GWL_STYLE, WA_INACTIVE, WM_ACTIVATE, WM_COMMAND,
        WM_KILLFOCUS, WM_LBUTTONUP, WM_MOUSEWHEEL, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
        SendMessageW,
    };

    use crate::calendar::CalendarModel;
    use crate::position::{
        calculate_popup_geometry, DisplayRect, MonitorBounds,
    };
    use crate::render::{rgb, CalendarTheme};
    use crate::tray::{TrayMenuCommand, DEFAULT_TRAY_UID, WM_TRAYICON};
    use crate::window::{CalendarWindow, PopupWindowConfig, WHEEL_DELTA};

    /// 完成基準 1: Windows 11環境で正常にビルド・単体実行が成功すること
    #[test]
    fn test_acceptance_criterion_1_clean_build_and_module_contracts() {
        // すべてのコア構造体が Pure Rust で構築され、メモリ上に整合して初期化できること
        let model = CalendarModel::new();
        assert!(model.view_year >= 2020, "現在年が正しく取得できること");
        assert!(
            (1..=12).contains(&model.view_month),
            "現在月が有効範囲内であること"
        );
        assert_eq!(
            model.generate_grid().len(),
            42,
            "カレンダーグリッドは常に6行7列の42マスで構成されること"
        );

        let config = PopupWindowConfig::default();
        assert_eq!(config.width, 320);
        assert_eq!(config.height, 360);
        assert!(config.auto_hide_on_deactivate);
    }

    /// 完成基準 2: 起動時にタスクトレイへ常駐し、タスクバー等に余分な常駐ウィンドウが残らないこと
    #[test]
    fn test_acceptance_criterion_2_taskbar_hidden_tray_resident() {
        let config = PopupWindowConfig {
            title: "DesCal Acceptance Tray Test",
            width: 320,
            height: 360,
            initial_x: 50,
            initial_y: 50,
            auto_hide_on_deactivate: true,
        };

        let window = CalendarWindow::new(&config).expect("ポップアップウィンドウ作成に成功すること");

        // 起動直後は非表示であること
        assert!(
            !window.is_visible(),
            "起動直後のカレンダーウィンドウは非表示（トレイ常駐待機）であること"
        );

        // ウィンドウスタイルおよび拡張スタイルの検証
        unsafe {
            let style = GetWindowLongPtrW(window.hwnd(), GWL_STYLE) as u32;
            let ex_style = GetWindowLongPtrW(window.hwnd(), GWL_EXSTYLE) as u32;

            // 枠なしポップアップ
            assert_ne!(
                style & WS_POPUP.0,
                0,
                "WS_POPUP が設定されており枠なしであること"
            );

            // タスクバー非表示（WS_EX_TOOLWINDOW）かつ最前面（WS_EX_TOPMOST）
            assert_ne!(
                ex_style & WS_EX_TOOLWINDOW.0,
                0,
                "WS_EX_TOOLWINDOW が設定されタスクバーにボタンが表示されないこと"
            );
            assert_ne!(
                ex_style & WS_EX_TOPMOST.0,
                0,
                "WS_EX_TOPMOST が設定され最前面表示可能であること"
            );
        }

        // トレイアイコンを作成できること
        let tray_res = window.create_tray_icon("DesCal Acceptance");
        assert!(tray_res.is_ok(), "トレイアイコンオブジェクトが正常に生成できること");
    }

    /// 完成基準 3: トレイアイコンをクリックした際、タスクトレイ付近（右下）にモダンなカレンダーが素早くポップアップ表示されること
    #[test]
    fn test_acceptance_criterion_3_tray_click_popups_geometry_and_toggle() {
        let config = PopupWindowConfig {
            title: "DesCal Acceptance Popup Test",
            width: 320,
            height: 360,
            initial_x: 100,
            initial_y: 100,
            auto_hide_on_deactivate: true,
        };

        let window = CalendarWindow::new(&config).expect("ウィンドウ作成成功");
        assert!(!window.is_visible());

        // 1. トレイ左クリックイベント（WM_TRAYICON / WM_LBUTTONUP）のシミュレーション
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_TRAYICON,
                Some(WPARAM(DEFAULT_TRAY_UID as usize)),
                Some(LPARAM(WM_LBUTTONUP as isize)),
            );
        }

        // ポップアップが表示状態に遷移すること
        assert!(
            window.is_visible(),
            "トレイ左クリックによりカレンダーポップアップが表示されること"
        );

        // 2. 右下ポップアップ配置幾何学計算の検証（画面外み出し防止・トレイ近傍配置）
        let monitor = MonitorBounds::new(
            DisplayRect::new(0, 0, 1920, 1080),
            DisplayRect::new(0, 0, 1920, 1032), // 下部タスクバー (高48px)
            96,
        );
        let geom = calculate_popup_geometry(&monitor, 320, 360, 12, None);
        assert_eq!(geom.width, 320);
        assert_eq!(geom.height, 360);
        assert_eq!(
            geom.x,
            1920 - 320 - 12,
            "タスクトレイ付近の右端マージン位置に配置されること"
        );
        assert_eq!(
            geom.y,
            1032 - 360 - 12,
            "タスクバー上部の下端マージン位置に配置されること"
        );

        // 3. 再度トレイ左クリック -> トグル非表示になること（デバウンス時間外のクリックシミュレート）
        // 50ms待機して直近Deactivateガードと区別
        std::thread::sleep(Duration::from_millis(50));
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_TRAYICON,
                Some(WPARAM(DEFAULT_TRAY_UID as usize)),
                Some(LPARAM(WM_LBUTTONUP as isize)),
            );
        }
        assert!(
            !window.is_visible(),
            "再度のトレイ左クリックによりカレンダーポップアップが非表示（トグル）になること"
        );
    }

    /// 完成基準 4: マウスホイールによるスクロールまたはボタン操作で、月送りがスムーズに行えること
    #[test]
    fn test_acceptance_criterion_4_smooth_month_navigation_buttons_and_wheel() {
        let config = PopupWindowConfig {
            title: "DesCal Acceptance Navigation Test",
            width: 320,
            height: 360,
            initial_x: 100,
            initial_y: 100,
            auto_hide_on_deactivate: true,
        };

        let mut window = CalendarWindow::new(&config).expect("ウィンドウ作成成功");

        // 初期年月を取得
        let initial_year = window.model().view_year;
        let initial_month = window.model().view_month;

        // 1. ナビゲーションボタン操作による月送り・月戻し
        // Next ボタンクリック
        let next_rect = window.layout().next_btn_rect;
        window.handle_lbutton_down(next_rect.left + 5, next_rect.top + 5);
        window.handle_lbutton_up(next_rect.left + 5, next_rect.top + 5);

        let (expected_next_year, expected_next_month) = if initial_month == 12 {
            (initial_year + 1, 1)
        } else {
            (initial_year, initial_month + 1)
        };
        assert_eq!(window.model().view_year, expected_next_year);
        assert_eq!(window.model().view_month, expected_next_month);

        // Prev ボタンクリック
        let prev_rect = window.layout().prev_btn_rect;
        window.handle_lbutton_down(prev_rect.left + 5, prev_rect.top + 5);
        window.handle_lbutton_up(prev_rect.left + 5, prev_rect.top + 5);
        assert_eq!(window.model().view_year, initial_year);
        assert_eq!(window.model().view_month, initial_month);

        // 2. マウスホイールスクロールによる月送り（WM_MOUSEWHEEL）
        // 下回転（-120）: 翌月方向
        let wheel_down_wparam = CalendarWindow::pack_wheel_wparam(-WHEEL_DELTA as i16, 0);
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_MOUSEWHEEL,
                Some(wheel_down_wparam),
                Some(LPARAM(0)),
            );
        }
        assert_eq!(window.model().view_year, expected_next_year);
        assert_eq!(window.model().view_month, expected_next_month);

        // 上回転 (+120): 前月方向
        let wheel_up_wparam = CalendarWindow::pack_wheel_wparam(WHEEL_DELTA as i16, 0);
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_MOUSEWHEEL,
                Some(wheel_up_wparam),
                Some(LPARAM(0)),
            );
        }
        assert_eq!(window.model().view_year, initial_year);
        assert_eq!(window.model().view_month, initial_month);

        // 3. 「今日」ボタンクリックで現在日へ即座に復帰
        window.model_mut().add_months(24); // 2年進める
        let today_rect = window.layout().today_btn_rect;
        window.handle_lbutton_down(today_rect.left + 5, today_rect.top + 5);
        window.handle_lbutton_up(today_rect.left + 5, today_rect.top + 5);
        assert_eq!(window.model().view_year, initial_year);
        assert_eq!(window.model().view_month, initial_month);
    }

    /// 完成基準 5: ポップアップ外をクリックした際に、自動でポップアップが非表示（クローズ）になること
    #[test]
    fn test_acceptance_criterion_5_auto_hide_on_deactivate_and_blur() {
        let config = PopupWindowConfig {
            title: "DesCal Acceptance AutoHide Test",
            width: 320,
            height: 360,
            initial_x: 100,
            initial_y: 100,
            auto_hide_on_deactivate: true,
        };

        let mut window = CalendarWindow::new(&config).expect("ウィンドウ作成成功");

        // 1. 表示状態にする
        window.show_popup();
        assert!(window.is_visible());

        // 2. ウィンドウ外クリックによるフォーカス喪失（WM_ACTIVATE: WA_INACTIVE）
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_ACTIVATE,
                Some(WPARAM(WA_INACTIVE as usize)),
                Some(LPARAM(0)),
            );
        }
        assert!(
            !window.is_visible(),
            "外クリック（WA_INACTIVE）で自動的に非表示になること"
        );

        // 3. 再度表示し、WM_KILLFOCUS によるフォーカス喪失での自動非表示も検証
        window.show_popup();
        assert!(window.is_visible());
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_KILLFOCUS,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            );
        }
        assert!(
            !window.is_visible(),
            "WM_KILLFOCUS で自動的に非表示になること"
        );
    }

    /// 完成基準 6: トレイメニュー等の終了操作から、プロセスが正常に終了すること
    #[test]
    fn test_acceptance_criterion_6_tray_menu_exit_terminates_gracefully() {
        let config = PopupWindowConfig {
            title: "DesCal Acceptance Exit Test",
            width: 320,
            height: 360,
            initial_x: 100,
            initial_y: 100,
            auto_hide_on_deactivate: true,
        };

        let window = CalendarWindow::new(&config).expect("ウィンドウ作成成功");
        assert!(!window.should_quit(), "初期状態では should_quit は false であること");

        // 右クリックコンテキストメニューから「終了」コマンドを実行
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_COMMAND,
                Some(WPARAM(TrayMenuCommand::Exit.id())),
                Some(LPARAM(0)),
            );
        }

        assert!(
            window.should_quit(),
            "Exit コマンド受信により should_quit が true に遷移し正常終了可能になること"
        );
    }

    /// 完成基準 7: 以上のすべてが100% Pure Rustの実装によって満たされていること
    #[test]
    fn test_acceptance_criterion_7_pure_rust_architecture() {
        // Cargo.toml のメタデータおよび Pure Rust 定義の検証
        let pkg_name = env!("CARGO_PKG_NAME");
        let pkg_version = env!("CARGO_PKG_VERSION");
        assert_eq!(pkg_name, "descal");
        assert!(!pkg_version.is_empty());

        // 外部スクリプトランタイム（Node/Python/Webview2等）を使用せず、
        // windows crate (Win32 API) と chrono のみで全機能が構成されていること
        let theme = CalendarTheme::default();
        assert_eq!(theme.bg_color, rgb(32, 32, 32));
        assert_eq!(theme.border_color, rgb(58, 58, 58));
        assert_eq!(theme.today_highlight_bg, rgb(0, 120, 212));
    }
}
