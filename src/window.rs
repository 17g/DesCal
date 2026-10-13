use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::{w, Error, Result, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWM_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, InvalidateRect, ScreenToClient, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetCursorPos, GetWindowLongPtrW,
    IsWindow, IsWindowVisible, LoadCursorW, PostQuitMessage, RegisterClassExW, SetCursor,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA,
    GWL_EXSTYLE, GWL_STYLE, IDC_ARROW, IDC_HAND, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE,
    SW_SHOW, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSEXW, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WM_COMMAND, WM_DESTROY, WM_ERASEBKGND,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_SETCURSOR,
};

const WM_MOUSELEAVE: u32 = 0x02A3;
/// 標準マウスホイールの1ノッチあたりの回転量閾値
pub const WHEEL_DELTA: i32 = 120;

use crate::calendar::CalendarModel;
use crate::position::{enable_per_monitor_dpi_v2, get_window_dpi, BASE_DPI};
use crate::render::{CalendarLayout, CalendarRenderer, CalendarTheme, NavButton};
use crate::tray::{
    parse_tray_message, TrayContextMenu, TrayEvent, TrayIcon, TrayMenuCommand, DEFAULT_TRAY_UID,
    WM_TRAYICON,
};

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
    pub hovered_nav: Option<NavButton>,
    pub pressed_nav: Option<NavButton>,
    pub wheel_delta_accum: i32,
    pub should_quit: bool,
}

impl WindowState {
    /// マウスホイール回転量（zDelta）を処理し、移動した月数（過去: 負、未来: 正、移動なし: 0）を返す。
    /// 高精度タッチパッド等の端数回転量（< 120）を蓄積し、逆方向への回転時は端数をリセットして即応性を保つ。
    pub fn process_mouse_wheel(&mut self, delta: i16) -> i32 {
        if delta == 0 {
            return 0;
        }
        let delta_i32 = delta as i32;
        // 前回の蓄積量と逆方向にスクロールされた場合は端数をリセット
        if (self.wheel_delta_accum > 0 && delta_i32 < 0)
            || (self.wheel_delta_accum < 0 && delta_i32 > 0)
        {
            self.wheel_delta_accum = 0;
        }

        self.wheel_delta_accum += delta_i32;
        let steps = self.wheel_delta_accum / WHEEL_DELTA;
        if steps != 0 {
            self.wheel_delta_accum %= WHEEL_DELTA;
            // ホイール上回転（delta > 0, steps > 0）は前月（過去方向: -steps）
            // ホイール下回転（delta < 0, steps < 0）は翌月（未来方向: -steps）
            let month_offset = -steps;
            self.calendar.add_months(month_offset);
            month_offset
        } else {
            0
        }
    }

    /// トレイのコンテキストメニューコマンドを実行する
    pub fn execute_menu_command(&mut self, hwnd: HWND, cmd: TrayMenuCommand) {
        match cmd {
            TrayMenuCommand::ToggleCalendar => unsafe {
                let visible = IsWindowVisible(hwnd).as_bool();
                if visible {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                } else {
                    let _ = ShowWindow(hwnd, SW_SHOW);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
            },
            TrayMenuCommand::GoToday => {
                self.wheel_delta_accum = 0;
                self.calendar.go_to_today();
                unsafe {
                    let _ = ShowWindow(hwnd, SW_SHOW);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
            }
            TrayMenuCommand::Exit => {
                self.should_quit = true;
                unsafe {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                    PostQuitMessage(0);
                }
            }
        }
    }
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
            hovered_nav: None,
            pressed_nav: None,
            wheel_delta_accum: 0,
            should_quit: false,
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
        self.state.wheel_delta_accum = 0;
        self.state.calendar.prev_month();
        self.refresh();
    }

    /// 翌月に移動して再描画
    pub fn next_month(&mut self) {
        self.state.wheel_delta_accum = 0;
        self.state.calendar.next_month();
        self.refresh();
    }

    /// 今日の月に移動して再描画
    pub fn go_to_today(&mut self) {
        self.state.wheel_delta_accum = 0;
        self.state.calendar.go_to_today();
        self.refresh();
    }

    /// 年月を直接指定して再描画
    pub fn set_date(&mut self, year: i32, month: u32) {
        self.state.wheel_delta_accum = 0;
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

    /// ホバー中のナビゲーションボタンを取得
    pub fn hovered_nav(&self) -> Option<NavButton> {
        self.state.hovered_nav
    }

    /// 押下中のナビゲーションボタンを取得
    pub fn pressed_nav(&self) -> Option<NavButton> {
        self.state.pressed_nav
    }

    /// 現在のマウスホイール端数蓄積量を取得
    pub fn wheel_delta_accum(&self) -> i32 {
        self.state.wheel_delta_accum
    }

    /// マウスホイール端数蓄積量をリセット
    pub fn reset_wheel_accum(&mut self) {
        self.state.wheel_delta_accum = 0;
    }

    /// マウスホイールスクロール（WM_MOUSEWHEEL）処理を行い、移動した月数（過去: 負、未来: 正、移動なし: 0）を返す
    pub fn handle_mouse_wheel(&mut self, delta: i16) -> i32 {
        let moved = self.state.process_mouse_wheel(delta);
        if moved != 0 {
            self.refresh();
        }
        moved
    }

    /// WPARAM からマウスホイール回転量（zDelta: 上位16ビット符号付き整数）を抽出
    pub fn extract_wheel_delta(wparam: WPARAM) -> i16 {
        ((wparam.0 >> 16) & 0xffff) as i16
    }

    /// マウスホイール回転量（zDelta）とキー状態から WM_MOUSEWHEEL 用の WPARAM を構築
    pub fn pack_wheel_wparam(delta: i16, keys: u16) -> WPARAM {
        let low = (keys as usize) & 0xffff;
        let high = ((delta as u16 as usize) & 0xffff) << 16;
        WPARAM(low | high)
    }

    /// 指定座標に対するマウスクリック処理を行い、発火したアクションを返す
    pub fn handle_mouse_click(&mut self, x: i32, y: i32) -> Option<NavButton> {
        let layout = self.layout();
        let pt = POINT { x, y };
        let hit = layout.hit_test_nav_button(pt);
        match hit {
            Some(NavButton::Prev) => {
                self.prev_month();
            }
            Some(NavButton::Today) => {
                self.go_to_today();
            }
            Some(NavButton::Next) => {
                self.next_month();
            }
            None => {}
        }
        hit
    }

    /// 左マウスボタン押下（WM_LBUTTONDOWN）処理
    pub fn handle_lbutton_down(&mut self, x: i32, y: i32) -> Option<NavButton> {
        let layout = self.layout();
        let pt = POINT { x, y };
        let hit = layout.hit_test_nav_button(pt);
        self.state.pressed_nav = hit;
        match hit {
            Some(NavButton::Prev) => {
                self.prev_month();
            }
            Some(NavButton::Today) => {
                self.go_to_today();
            }
            Some(NavButton::Next) => {
                self.next_month();
            }
            None => {}
        }
        self.refresh();
        hit
    }

    /// 左マウスボタン離脱（WM_LBUTTONUP）処理
    pub fn handle_lbutton_up(&mut self, _x: i32, _y: i32) -> bool {
        if self.state.pressed_nav.is_some() {
            self.state.pressed_nav = None;
            self.refresh();
            true
        } else {
            false
        }
    }

    /// マウス移動（WM_MOUSEMOVE）処理
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) -> bool {
        let layout = self.layout();
        let pt = POINT { x, y };
        let hit = layout.hit_test_nav_button(pt);
        if self.state.hovered_nav != hit {
            self.state.hovered_nav = hit;
            self.refresh();
            true
        } else {
            false
        }
    }

    /// マウス離脱（WM_MOUSELEAVE）処理
    pub fn handle_mouse_leave(&mut self) -> bool {
        let changed = self.state.hovered_nav.is_some() || self.state.pressed_nav.is_some();
        if changed {
            self.state.hovered_nav = None;
            self.state.pressed_nav = None;
            self.refresh();
        }
        changed
    }

    /// 終了要求フラグ（メニューの「終了」選択等）が立っているかを取得
    pub fn should_quit(&self) -> bool {
        self.state.should_quit
    }

    /// トレイのコンテキストメニューコマンドを実行する
    pub fn execute_menu_command(&mut self, cmd: TrayMenuCommand) {
        self.state.execute_menu_command(self.hwnd, cmd);
    }

    /// このウィンドウに紐づくシステムトレイアイコンを作成・常駐させる
    pub fn create_tray_icon(&self, tooltip: &str) -> Result<TrayIcon> {
        TrayIcon::new(self.hwnd, DEFAULT_TRAY_UID, tooltip)
    }

    /// ウィンドウプロシージャ（メッセージルーティング）
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
                        state.hovered_nav,
                        state.pressed_nav,
                    );

                    let _ = EndPaint(hwnd, &ps);
                    return LRESULT(0);
                }
                WM_ERASEBKGND => {
                    // チラつき防止のため背景消去はスキップ（CalendarRenderer で一括描画）
                    return LRESULT(1);
                }
                WM_LBUTTONDOWN => {
                    let x = (lparam.0 & 0xffff) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
                    let mut rc = RECT::default();
                    let _ = GetClientRect(hwnd, &mut rc);
                    let width = rc.right - rc.left;
                    let height = rc.bottom - rc.top;

                    let layout = CalendarLayout::calculate(width, height, state.dpi);
                    let pt = POINT { x, y };
                    let hit = layout.hit_test_nav_button(pt);
                    state.pressed_nav = hit;
                    match hit {
                        Some(NavButton::Prev) => {
                            state.wheel_delta_accum = 0;
                            state.calendar.prev_month();
                        }
                        Some(NavButton::Today) => {
                            state.wheel_delta_accum = 0;
                            state.calendar.go_to_today();
                        }
                        Some(NavButton::Next) => {
                            state.wheel_delta_accum = 0;
                            state.calendar.next_month();
                        }
                        None => {}
                    }
                    let _ = InvalidateRect(Some(hwnd), None, false);
                    return LRESULT(0);
                }
                WM_LBUTTONUP => {
                    if state.pressed_nav.is_some() {
                        state.pressed_nav = None;
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                    return LRESULT(0);
                }
                WM_MOUSEMOVE => {
                    let x = (lparam.0 & 0xffff) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
                    let mut rc = RECT::default();
                    let _ = GetClientRect(hwnd, &mut rc);
                    let width = rc.right - rc.left;
                    let height = rc.bottom - rc.top;

                    let layout = CalendarLayout::calculate(width, height, state.dpi);
                    let pt = POINT { x, y };
                    let hit = layout.hit_test_nav_button(pt);
                    if state.hovered_nav != hit {
                        state.hovered_nav = hit;
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }

                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut tme);
                    return LRESULT(0);
                }
                WM_MOUSELEAVE => {
                    if state.hovered_nav.is_some() || state.pressed_nav.is_some() {
                        state.hovered_nav = None;
                        state.pressed_nav = None;
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                    return LRESULT(0);
                }
                WM_MOUSEWHEEL => {
                    let delta = Self::extract_wheel_delta(wparam);
                    let moved = state.process_mouse_wheel(delta);
                    if moved != 0 {
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                    return LRESULT(0);
                }
                WM_SETCURSOR => {
                    let mut pt = POINT::default();
                    if GetCursorPos(&mut pt).is_ok() {
                        let _ = ScreenToClient(hwnd, &mut pt);
                        let mut rc = RECT::default();
                        let _ = GetClientRect(hwnd, &mut rc);
                        let width = rc.right - rc.left;
                        let height = rc.bottom - rc.top;

                        let layout = CalendarLayout::calculate(width, height, state.dpi);
                        if layout.hit_test_nav_button(pt).is_some() {
                            if let Ok(cursor) = LoadCursorW(None, IDC_HAND) {
                                SetCursor(Some(cursor));
                                return LRESULT(1);
                            }
                        }
                    }
                }
                WM_COMMAND => {
                    let cmd_id = (wparam.0 & 0xffff) as usize;
                    if let Some(cmd) = TrayMenuCommand::from_id(cmd_id) {
                        state.execute_menu_command(hwnd, cmd);
                        return LRESULT(0);
                    }
                }
                WM_TRAYICON => {
                    match parse_tray_message(lparam) {
                        TrayEvent::RightClick => {
                            let is_vis = IsWindowVisible(hwnd).as_bool();
                            if let Ok(menu) = TrayContextMenu::new(is_vis) {
                                if let Some(cmd) = menu.show_at_cursor(hwnd) {
                                    state.execute_menu_command(hwnd, cmd);
                                }
                            }
                            return LRESULT(0);
                        }
                        TrayEvent::LeftClick => {
                            return LRESULT(0);
                        }
                        TrayEvent::Other(_) => {}
                    }
                }
                WM_DESTROY => {
                    state.should_quit = true;
                    PostQuitMessage(0);
                    return LRESULT(0);
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
    use windows::Win32::UI::WindowsAndMessaging::SendMessageW;

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

    #[test]
    fn test_mouse_navigation_click_events() {
        let config = PopupWindowConfig {
            title: "DesCal Mouse Click Test",
            width: 320,
            height: 360,
            initial_x: 0,
            initial_y: 0,
        };

        let mut window = CalendarWindow::new(&config).expect("ウィンドウ作成失敗");
        window.set_date(2026, 10);
        assert_eq!(window.model().view_year, 2026);
        assert_eq!(window.model().view_month, 10);

        let layout = window.layout();

        // 1. Prev ボタンのクリック -> 2026年9月へ移動
        let prev_pt = POINT {
            x: (layout.prev_btn_rect.left + layout.prev_btn_rect.right) / 2,
            y: (layout.prev_btn_rect.top + layout.prev_btn_rect.bottom) / 2,
        };
        let action = window.handle_mouse_click(prev_pt.x, prev_pt.y);
        assert_eq!(action, Some(NavButton::Prev));
        assert_eq!(window.model().view_year, 2026);
        assert_eq!(window.model().view_month, 9);

        // 2. Next ボタンのクリック -> 2026年10月へ移動
        let next_pt = POINT {
            x: (layout.next_btn_rect.left + layout.next_btn_rect.right) / 2,
            y: (layout.next_btn_rect.top + layout.next_btn_rect.bottom) / 2,
        };
        let action = window.handle_mouse_click(next_pt.x, next_pt.y);
        assert_eq!(action, Some(NavButton::Next));
        assert_eq!(window.model().view_year, 2026);
        assert_eq!(window.model().view_month, 10);

        // 3. もう一度 Next ボタンのクリック -> 2026年11月へ移動
        let action = window.handle_mouse_click(next_pt.x, next_pt.y);
        assert_eq!(action, Some(NavButton::Next));
        assert_eq!(window.model().view_year, 2026);
        assert_eq!(window.model().view_month, 11);

        // 4. Today ボタンのクリック -> 今日の年月へジャンプ
        let today_pt = POINT {
            x: (layout.today_btn_rect.left + layout.today_btn_rect.right) / 2,
            y: (layout.today_btn_rect.top + layout.today_btn_rect.bottom) / 2,
        };
        let action = window.handle_mouse_click(today_pt.x, today_pt.y);
        assert_eq!(action, Some(NavButton::Today));
        assert_eq!(window.model().view_year, window.model().today.year());
        assert_eq!(window.model().view_month, window.model().today.month());

        // 5. ボタン外（カレンダーグリッドのセル）をクリック -> アクションなし
        let cell_pt = POINT {
            x: (layout.cell_rects[15].left + layout.cell_rects[15].right) / 2,
            y: (layout.cell_rects[15].top + layout.cell_rects[15].bottom) / 2,
        };
        let action = window.handle_mouse_click(cell_pt.x, cell_pt.y);
        assert_eq!(action, None);
    }

    #[test]
    fn test_mouse_hover_and_press_states() {
        let config = PopupWindowConfig {
            title: "DesCal Hover State Test",
            width: 320,
            height: 360,
            initial_x: 0,
            initial_y: 0,
        };

        let mut window = CalendarWindow::new(&config).expect("ウィンドウ作成失敗");
        let layout = window.layout();

        let prev_pt = POINT {
            x: (layout.prev_btn_rect.left + layout.prev_btn_rect.right) / 2,
            y: (layout.prev_btn_rect.top + layout.prev_btn_rect.bottom) / 2,
        };
        let today_pt = POINT {
            x: (layout.today_btn_rect.left + layout.today_btn_rect.right) / 2,
            y: (layout.today_btn_rect.top + layout.today_btn_rect.bottom) / 2,
        };

        // 初期状態はホバーなし
        assert_eq!(window.hovered_nav(), None);
        assert_eq!(window.pressed_nav(), None);

        // Prev ボタンへマウス移動
        let changed = window.handle_mouse_move(prev_pt.x, prev_pt.y);
        assert!(changed);
        assert_eq!(window.hovered_nav(), Some(NavButton::Prev));

        // 同一ボタン上での再移動 -> 変化なし
        let changed = window.handle_mouse_move(prev_pt.x, prev_pt.y);
        assert!(!changed);

        // Today ボタンへマウス移動
        let changed = window.handle_mouse_move(today_pt.x, today_pt.y);
        assert!(changed);
        assert_eq!(window.hovered_nav(), Some(NavButton::Today));

        // ボタン押下
        let hit = window.handle_lbutton_down(today_pt.x, today_pt.y);
        assert_eq!(hit, Some(NavButton::Today));
        assert_eq!(window.pressed_nav(), Some(NavButton::Today));

        // ボタン離脱
        let released = window.handle_lbutton_up(today_pt.x, today_pt.y);
        assert!(released);
        assert_eq!(window.pressed_nav(), None);

        // ウィンドウ外離脱
        let left = window.handle_mouse_leave();
        assert!(left);
        assert_eq!(window.hovered_nav(), None);
        assert_eq!(window.pressed_nav(), None);
    }

    #[test]
    fn test_window_proc_mouse_messages() {
        let config = PopupWindowConfig {
            title: "DesCal WndProc Mouse Test",
            width: 320,
            height: 360,
            initial_x: 0,
            initial_y: 0,
        };

        let mut window = CalendarWindow::new(&config).expect("ウィンドウ作成失敗");
        window.set_date(2026, 5);
        let layout = window.layout();

        let make_lparam = |pt: POINT| -> LPARAM {
            let val = ((pt.x as u32) & 0xffff) | (((pt.y as u32) & 0xffff) << 16);
            LPARAM(val as isize)
        };

        let prev_pt = POINT {
            x: (layout.prev_btn_rect.left + layout.prev_btn_rect.right) / 2,
            y: (layout.prev_btn_rect.top + layout.prev_btn_rect.bottom) / 2,
        };
        let next_pt = POINT {
            x: (layout.next_btn_rect.left + layout.next_btn_rect.right) / 2,
            y: (layout.next_btn_rect.top + layout.next_btn_rect.bottom) / 2,
        };

        // WM_LBUTTONDOWN メッセージを Prev ボタン座標へ送信
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_LBUTTONDOWN,
                Some(WPARAM(0)),
                Some(make_lparam(prev_pt)),
            );
        }
        assert_eq!(window.model().view_year, 2026);
        assert_eq!(window.model().view_month, 4);

        // WM_LBUTTONUP メッセージ送信
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_LBUTTONUP,
                Some(WPARAM(0)),
                Some(make_lparam(prev_pt)),
            );
        }
        assert_eq!(window.pressed_nav(), None);

        // WM_LBUTTONDOWN メッセージを Next ボタン座標へ送信
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_LBUTTONDOWN,
                Some(WPARAM(0)),
                Some(make_lparam(next_pt)),
            );
        }
        assert_eq!(window.model().view_year, 2026);
        assert_eq!(window.model().view_month, 5);

        // WM_MOUSEMOVE メッセージ送信
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_MOUSEMOVE,
                Some(WPARAM(0)),
                Some(make_lparam(next_pt)),
            );
        }
        assert_eq!(window.hovered_nav(), Some(NavButton::Next));

        // WM_MOUSELEAVE メッセージ送信
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_MOUSELEAVE,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            );
        }
        assert_eq!(window.hovered_nav(), None);
    }

    #[test]
    fn test_mouse_wheel_standard_and_smooth_scroll() {
        let config = PopupWindowConfig {
            title: "DesCal Mouse Wheel Test",
            width: 320,
            height: 360,
            initial_x: 0,
            initial_y: 0,
        };

        let mut window = CalendarWindow::new(&config).expect("ウィンドウ作成失敗");
        window.set_date(2026, 10);

        // 1. 標準ホイール上回転 (+120) -> 前月 (2026年9月)
        let moved = window.handle_mouse_wheel(120);
        assert_eq!(moved, -1);
        assert_eq!(window.model().view_year, 2026);
        assert_eq!(window.model().view_month, 9);
        assert_eq!(window.wheel_delta_accum(), 0);

        // 2. 標準ホイール下回転 (-120) -> 翌月 (2026年10月)
        let moved = window.handle_mouse_wheel(-120);
        assert_eq!(moved, 1);
        assert_eq!(window.model().view_year, 2026);
        assert_eq!(window.model().view_month, 10);
        assert_eq!(window.wheel_delta_accum(), 0);

        // 3. 複数ノッチ一括回転 (-360 = 3ノッチ下回転) -> 3ヶ月先 (2027年1月)
        let moved = window.handle_mouse_wheel(-360);
        assert_eq!(moved, 3);
        assert_eq!(window.model().view_year, 2027);
        assert_eq!(window.model().view_month, 1);

        // 4. 高精度タッチパッド等の細かい端数スクロール (+40 × 3回 = +120)
        assert_eq!(window.handle_mouse_wheel(40), 0);
        assert_eq!(window.wheel_delta_accum(), 40);
        assert_eq!(window.model().view_month, 1);

        assert_eq!(window.handle_mouse_wheel(40), 0);
        assert_eq!(window.wheel_delta_accum(), 80);
        assert_eq!(window.model().view_month, 1);

        assert_eq!(window.handle_mouse_wheel(40), -1);
        assert_eq!(window.wheel_delta_accum(), 0);
        assert_eq!(window.model().view_year, 2026);
        assert_eq!(window.model().view_month, 12);

        // 5. 端数蓄積中に逆方向へスクロールした場合、端数がリセットされて即応すること
        assert_eq!(window.handle_mouse_wheel(60), 0);
        assert_eq!(window.wheel_delta_accum(), 60);
        // 逆方向へ 1ノッチ (-120) -> 60 がキャンセルされ即座に翌月 (2027年1月) へ移動
        assert_eq!(window.handle_mouse_wheel(-120), 1);
        assert_eq!(window.wheel_delta_accum(), 0);
        assert_eq!(window.model().view_year, 2027);
        assert_eq!(window.model().view_month, 1);
    }

    #[test]
    fn test_window_proc_mouse_wheel_messages() {
        let config = PopupWindowConfig {
            title: "DesCal WndProc Wheel Test",
            width: 320,
            height: 360,
            initial_x: 0,
            initial_y: 0,
        };

        let mut window = CalendarWindow::new(&config).expect("ウィンドウ作成失敗");
        window.set_date(2026, 1);

        // WPARAM パック・抽出ヘルパーの検証
        let wp_up = CalendarWindow::pack_wheel_wparam(120, 0);
        assert_eq!(CalendarWindow::extract_wheel_delta(wp_up), 120);

        let wp_down = CalendarWindow::pack_wheel_wparam(-120, 0);
        assert_eq!(CalendarWindow::extract_wheel_delta(wp_down), -120);

        // WM_MOUSEWHEEL (上回転 +120) 送信 -> 2025年12月へ移動
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_MOUSEWHEEL,
                Some(wp_up),
                Some(LPARAM(0)),
            );
        }
        assert_eq!(window.model().view_year, 2025);
        assert_eq!(window.model().view_month, 12);

        // WM_MOUSEWHEEL (下回転 -120) 送信 -> 2026年1月へ移動
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_MOUSEWHEEL,
                Some(wp_down),
                Some(LPARAM(0)),
            );
        }
        assert_eq!(window.model().view_year, 2026);
        assert_eq!(window.model().view_month, 1);

        // 端数 (+50, +50, +50 = +150) を WM_MOUSEWHEEL で連続送信 -> 3回目で前月 (2025年12月) へ移動し、余り 30 が残る
        let wp_partial = CalendarWindow::pack_wheel_wparam(50, 0);
        unsafe {
            let _ = SendMessageW(window.hwnd(), WM_MOUSEWHEEL, Some(wp_partial), Some(LPARAM(0)));
            assert_eq!(window.model().view_month, 1);
            let _ = SendMessageW(window.hwnd(), WM_MOUSEWHEEL, Some(wp_partial), Some(LPARAM(0)));
            assert_eq!(window.model().view_month, 1);
            let _ = SendMessageW(window.hwnd(), WM_MOUSEWHEEL, Some(wp_partial), Some(LPARAM(0)));
        }
        assert_eq!(window.model().view_year, 2025);
        assert_eq!(window.model().view_month, 12);
        assert_eq!(window.wheel_delta_accum(), 30);
    }

    #[test]
    fn test_tray_menu_commands_and_wm_command() {
        use crate::tray::{MENU_ID_EXIT, MENU_ID_GO_TODAY, MENU_ID_TOGGLE_CALENDAR};
        use windows::Win32::UI::WindowsAndMessaging::{
            PeekMessageW, MSG, PM_REMOVE, WM_QUIT,
        };

        let config = PopupWindowConfig {
            title: "DesCal Tray Command Test",
            width: 320,
            height: 360,
            initial_x: 0,
            initial_y: 0,
        };

        let mut window = CalendarWindow::new(&config).expect("ウィンドウ作成失敗");
        assert!(!window.is_visible());
        assert!(!window.should_quit());

        // 1. MENU_ID_TOGGLE_CALENDAR 送信 -> 非表示から表示へ切り替わること
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_COMMAND,
                Some(WPARAM(MENU_ID_TOGGLE_CALENDAR)),
                Some(LPARAM(0)),
            );
        }
        assert!(window.is_visible());

        // 2. 再度 MENU_ID_TOGGLE_CALENDAR 送信 -> 表示から非表示へ切り替わること
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_COMMAND,
                Some(WPARAM(MENU_ID_TOGGLE_CALENDAR)),
                Some(LPARAM(0)),
            );
        }
        assert!(!window.is_visible());

        // 3. 過去の年月を表示中に MENU_ID_GO_TODAY 送信 -> 今日の年月へ戻り、表示されること
        window.set_date(2020, 3);
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_COMMAND,
                Some(WPARAM(MENU_ID_GO_TODAY)),
                Some(LPARAM(0)),
            );
        }
        assert!(window.is_visible());
        assert_eq!(window.model().view_year, window.model().today.year());
        assert_eq!(window.model().view_month, window.model().today.month());

        // 4. MENU_ID_EXIT（「終了」）送信 -> should_quit が true になり、非表示化され、WM_QUIT がポストされること
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_COMMAND,
                Some(WPARAM(MENU_ID_EXIT)),
                Some(LPARAM(0)),
            );
        }
        assert!(window.should_quit());
        assert!(!window.is_visible());

        // スレッドキューにポストされた WM_QUIT を確認・回収
        unsafe {
            let mut msg = MSG::default();
            let has_quit = PeekMessageW(&mut msg, None, WM_QUIT, WM_QUIT, PM_REMOVE).as_bool();
            assert!(has_quit, "WM_QUIT メッセージがポストされていません");
            assert_eq!(msg.message, WM_QUIT);
        }
    }
}

