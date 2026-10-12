use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::{w, Error, Result, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWM_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND,
};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetWindowLongPtrW, IsWindow,
    IsWindowVisible, LoadCursorW, RegisterClassExW, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, GWL_EXSTYLE, GWL_STYLE, IDC_ARROW,
    SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_SHOW, WINDOW_EX_STYLE, WINDOW_STYLE,
    WNDCLASSEXW, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    WM_ERASEBKGND, WM_PAINT,
};

use crate::calendar::CalendarModel;
use crate::position::{enable_per_monitor_dpi_v2, get_window_dpi, BASE_DPI};
use crate::render::{CalendarLayout, CalendarRenderer, CalendarTheme};

const CLASS_NAME: PCWSTR = w!("DesCalCalendarWindowClass");
static CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);

/// ポップアップウィンドウの作成設定
#[derive(Debug, Clone)]
pub struct PopupWindowConfig {
    pub title: &'static str,
    pub width: i32,
    pub height: i32,
    pub initial_x: i32,
    pub initial_y: i32,
}

impl Default for PopupWindowConfig {
    fn default() -> Self {
        Self {
            title: "DesCal Calendar",
            width: 320,
            height: 360,
            initial_x: 100,
            initial_y: 100,
        }
    }
}

/// ウィンドウに関連付けられる状態データ
pub struct WindowState {
    pub calendar: CalendarModel,
    pub theme: CalendarTheme,
    pub dpi: u32,
}

/// タスクトレイ連動用カレンダーポップアップウィンドウ
pub struct CalendarWindow {
    hwnd: HWND,
    width: i32,
    height: i32,
    state: Box<WindowState>,
}

impl CalendarWindow {
    /// ポップアップウィンドウを作成
    pub fn new(config: &PopupWindowConfig) -> Result<Self> {
        let _ = enable_per_monitor_dpi_v2();

        let h_instance = unsafe { GetModuleHandleW(None)? };
        let h_instance: HINSTANCE = HINSTANCE(h_instance.0);

        Self::ensure_class_registered(h_instance)?;

        let mut state = Box::new(WindowState {
            calendar: CalendarModel::new(),
            theme: CalendarTheme::default(),
            dpi: BASE_DPI,
        });

        // スタイル:
        // WS_POPUP: タイトルバー・枠線のないポップアップウィンドウ
        // WS_CLIPCHILDREN | WS_CLIPSIBLINGS: 描画最適化
        let style = WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS;

        // 拡張スタイル:
        // WS_EX_TOOLWINDOW: タスクバー・Alt+Tabに表示されない
        // WS_EX_TOPMOST: 最前面ポップアップ
        let ex_style = WS_EX_TOOLWINDOW | WS_EX_TOPMOST;

        let hwnd = unsafe {
            CreateWindowExW(
                ex_style,
                CLASS_NAME,
                w!("DesCal"),
                style,
                config.initial_x,
                config.initial_y,
                config.width,
                config.height,
                None,
                None,
                Some(h_instance),
                None,
            )?
        };

        // ウィンドウの初期DPIを取得
        state.dpi = get_window_dpi(hwnd);

        // GWLP_USERDATA に状態ポインタを紐付け
        unsafe {
            let _ = SetWindowLongPtrW(
                hwnd,
                GWLP_USERDATA,
                &mut *state as *mut WindowState as isize,
            );
        }

        // Windows 11 の角丸設定 (DWMWCP_ROUND)
        unsafe {
            let preference = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &preference as *const DWM_WINDOW_CORNER_PREFERENCE as *const _,
                std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
            );
        }

        Ok(Self {
            hwnd,
            width: config.width,
            height: config.height,
            state,
        })
    }

    /// ウィンドウクラスの安全な登録（1度だけ実行）
    fn ensure_class_registered(h_instance: HINSTANCE) -> Result<()> {
        if CLASS_REGISTERED.load(Ordering::SeqCst) {
            return Ok(());
        }

        let cursor = unsafe { LoadCursorW(None, IDC_ARROW)? };

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
            lpfnWndProc: Some(Self::wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_instance,
            hIcon: Default::default(),
            hCursor: cursor,
            hbrBackground: Default::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: CLASS_NAME,
            hIconSm: Default::default(),
        };

        let atom = unsafe { RegisterClassExW(&wc) };
        if atom == 0 {
            let err = Error::from_thread();
            // すでに別スレッド等で登録済みの場合はエラーとしない (ERROR_CLASS_ALREADY_EXISTS: 0x80070582)
            if err.code().0 != 0x80070582u32 as i32 {
                return Err(err);
            }
        }

        CLASS_REGISTERED.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// ウィンドウハンドルを取得
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// ウィンドウの幅と高さを取得
    pub fn size(&self) -> (i32, i32) {
        (self.width, self.height)
    }

    /// 表示状態を取得
    pub fn is_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd).as_bool() }
    }

    /// ウィンドウハンドルが有効か確認
    pub fn is_valid(&self) -> bool {
        unsafe { IsWindow(Some(self.hwnd)).as_bool() }
    }

    /// ウィンドウを表示
    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    /// ウィンドウを非表示
    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    /// ウィンドウの位置を更新
    pub fn set_position(&self, x: i32, y: i32) {
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                None,
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    /// ウィンドウの位置とサイズ（ジオメトリ）を一括更新
    pub fn set_geometry(&mut self, geo: &crate::position::PopupGeometry) {
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                None,
                geo.x,
                geo.y,
                geo.width,
                geo.height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        self.width = geo.width;
        self.height = geo.height;
        self.state.dpi = get_window_dpi(self.hwnd);
        self.refresh();
    }

    /// カレンダーモデルへの参照を取得
    pub fn model(&self) -> &CalendarModel {
        &self.state.calendar
    }

    /// カレンダーモデルへの可変参照を取得
    pub fn model_mut(&mut self) -> &mut CalendarModel {
        &mut self.state.calendar
    }

    /// テーマへの参照を取得
    pub fn theme(&self) -> &CalendarTheme {
        &self.state.theme
    }

    /// テーマへの可変参照を取得
    pub fn theme_mut(&mut self) -> &mut CalendarTheme {
        &mut self.state.theme
    }

    /// 現在のDPI値を取得
    pub fn dpi(&self) -> u32 {
        self.state.dpi
    }

    /// 再描画要求をトリガー
    pub fn refresh(&self) {
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    /// 前月に移動して再描画
    pub fn prev_month(&mut self) {
        self.state.calendar.prev_month();
        self.refresh();
    }

    /// 翌月に移動して再描画
    pub fn next_month(&mut self) {
        self.state.calendar.next_month();
        self.refresh();
    }

    /// 今日の月に移動して再描画
    pub fn go_to_today(&mut self) {
        self.state.calendar.go_to_today();
        self.refresh();
    }

    /// 年月を直接指定して再描画
    pub fn set_date(&mut self, year: i32, month: u32) {
        self.state.calendar.view_year = year;
        self.state.calendar.view_month = month;
        self.refresh();
    }

    /// 現在のレイアウト幾何情報を取得
    pub fn layout(&self) -> CalendarLayout {
        CalendarLayout::calculate(self.width, self.height, self.state.dpi)
    }

    /// DPIおよびマルチモニターの作業領域を考慮し、タスクトレイ近傍の最適な座標へ再配置
    pub fn reposition_to_tray(
        &mut self,
        base_width: i32,
        base_height: i32,
        base_margin: i32,
    ) -> Result<crate::position::PopupGeometry> {
        let geo = crate::position::calculate_tray_popup_geometry(
            None,
            base_width,
            base_height,
            base_margin,
        )?;
        self.set_geometry(&geo);
        Ok(geo)
    }

    /// ウィンドウのウィンドウスタイル (WS_*) を取得
    pub fn get_style(&self) -> WINDOW_STYLE {
        let style = unsafe { GetWindowLongPtrW(self.hwnd, GWL_STYLE) };
        WINDOW_STYLE(style as u32)
    }

    /// ウィンドウの拡張スタイル (WS_EX_*) を取得
    pub fn get_ex_style(&self) -> WINDOW_EX_STYLE {
        let ex_style = unsafe { GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) };
        WINDOW_EX_STYLE(ex_style as u32)
    }

    /// ウィンドウプロシージャ（初期メッセージルーティング）
    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
        if !state_ptr.is_null() {
            let state = &mut *state_ptr;
            match msg {
                WM_PAINT => {
                    let mut ps = PAINTSTRUCT::default();
                    let hdc = BeginPaint(hwnd, &mut ps);
                    let mut rc = RECT::default();
                    let _ = GetClientRect(hwnd, &mut rc);
                    let width = rc.right - rc.left;
                    let height = rc.bottom - rc.top;

                    CalendarRenderer::paint(
                        hdc,
                        width,
                        height,
                        state.dpi,
                        &state.calendar,
                        &state.theme,
                    );

                    let _ = EndPaint(hwnd, &ps);
                    return LRESULT(0);
                }
                WM_ERASEBKGND => {
                    // チラつき防止のため背景消去はスキップ（CalendarRenderer で一括描画）
                    return LRESULT(1);
                }
                _ => {}
            }
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

impl Drop for CalendarWindow {
    fn drop(&mut self) {
        if self.is_valid() {
            unsafe {
                let _ = SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_create_popup_window_attributes() {
        let config = PopupWindowConfig {
            title: "DesCal Test Window",
            width: 300,
            height: 350,
            initial_x: 50,
            initial_y: 50,
        };

        let window = CalendarWindow::new(&config).expect("ポップアップウィンドウの作成に失敗");

        // ウィンドウハンドルが有効であること
        assert!(window.is_valid());
        assert_eq!(window.size(), (300, 350));

        // 初期状態は非表示であること
        assert!(!window.is_visible());

        // スタイル検証:
        // WS_POPUP を含んでいること（タイトルバーや枠なし）
        let style = window.get_style();
        assert!(
            (style.0 & WS_POPUP.0) != 0,
            "WS_POPUP スタイルが設定されていません: {:x}",
            style.0
        );

        // 拡張スタイル検証:
        // WS_EX_TOOLWINDOW を含んでいること（タスクバー非表示）
        let ex_style = window.get_ex_style();
        assert!(
            (ex_style.0 & WS_EX_TOOLWINDOW.0) != 0,
            "WS_EX_TOOLWINDOW が設定されていません: {:x}",
            ex_style.0
        );
        // WS_EX_TOPMOST を含んでいること（最前面）
        assert!(
            (ex_style.0 & WS_EX_TOPMOST.0) != 0,
            "WS_EX_TOPMOST が設定されていません: {:x}",
            ex_style.0
        );

        // 表示・非表示切り替えの確認
        window.show();
        assert!(window.is_visible());

        window.hide();
        assert!(!window.is_visible());
    }

    #[test]
    fn test_popup_window_reposition() {
        let config = PopupWindowConfig {
            title: "DesCal Reposition Test",
            width: 320,
            height: 360,
            initial_x: 0,
            initial_y: 0,
        };

        let mut window = CalendarWindow::new(&config).expect("ウィンドウ作成失敗");

        // 手動ジオメトリ設定のテスト
        let custom_geo = crate::position::PopupGeometry {
            x: 200,
            y: 300,
            width: 400,
            height: 450,
        };
        window.set_geometry(&custom_geo);
        assert_eq!(window.size(), (400, 450));

        // トレイ近傍への再配置テスト（実機のモニター情報から算出）
        let geo = window
            .reposition_to_tray(320, 360, 12)
            .expect("reposition_to_tray 失敗");

        // 算出されたサイズと同期していること
        assert_eq!(window.size(), (geo.width, geo.height));
        assert!(geo.width > 0 && geo.height > 0);
    }

    #[test]
    fn test_window_calendar_state_and_navigation() {
        let config = PopupWindowConfig {
            title: "DesCal Nav Test",
            width: 320,
            height: 360,
            initial_x: 10,
            initial_y: 10,
        };

        let mut window = CalendarWindow::new(&config).expect("ウィンドウ作成失敗");

        // 初期状態でテーマとカレンダーモデルが存在すること
        assert_eq!(window.theme().header_text_color, crate::render::rgb(255, 255, 255));
        assert!(window.dpi() > 0);

        // 日付手動変更
        window.set_date(2026, 10);
        assert_eq!(window.model().view_year, 2026);
        assert_eq!(window.model().view_month, 10);

        // 前月移動
        window.prev_month();
        assert_eq!(window.model().view_year, 2026);
        assert_eq!(window.model().view_month, 9);

        // 翌月移動
        window.next_month();
        assert_eq!(window.model().view_year, 2026);
        assert_eq!(window.model().view_month, 10);

        // 今日へジャンプ
        window.go_to_today();
        assert_eq!(window.model().view_year, window.model().today.year());
        assert_eq!(window.model().view_month, window.model().today.month());

        // レイアウト情報の整合性
        let layout = window.layout();
        assert_eq!(layout.cell_rects.len(), 42);
        assert_eq!(layout.weekday_rects.len(), 7);

        // 再描画要求の呼び出し確認
        window.refresh();
    }

    #[test]
    fn test_window_paint_lifecycle() {
        let config = PopupWindowConfig {
            title: "DesCal Paint Lifecycle Test",
            width: 320,
            height: 360,
            initial_x: 20,
            initial_y: 20,
        };

        let window = CalendarWindow::new(&config).expect("ウィンドウ作成失敗");
        window.show();
        assert!(window.is_visible());

        // 再描画メッセージの処理
        window.refresh();

        window.hide();
        assert!(!window.is_visible());
    }
}
