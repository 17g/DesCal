use windows::core::{Error, Result};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::HiDpi::{
    GetDpiForMonitor, GetDpiForSystem, GetDpiForWindow, SetProcessDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, MDT_EFFECTIVE_DPI,
};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// 基準となる標準論理DPI (100% スケーリング)
pub const BASE_DPI: u32 = 96;

/// デフォルトの基本ウィンドウサイズ（96 DPI基準）
pub const DEFAULT_BASE_WIDTH: i32 = 320;
pub const DEFAULT_BASE_HEIGHT: i32 = 360;
/// デフォルトのデスクトップ端マージン（96 DPI基準）
pub const DEFAULT_BASE_MARGIN: i32 = 12;

/// 矩形領域（OS非依存のテスト可能構造体）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl DisplayRect {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

impl From<RECT> for DisplayRect {
    fn from(rect: RECT) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        }
    }
}

/// タスクバーの端（位置）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskbarEdge {
    Bottom,
    Top,
    Left,
    Right,
    NoneOrUnknown,
}

/// モニター情報およびDPI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorBounds {
    pub monitor_rect: DisplayRect,
    pub work_rect: DisplayRect,
    pub dpi: u32,
}

impl MonitorBounds {
    pub fn new(monitor_rect: DisplayRect, work_rect: DisplayRect, dpi: u32) -> Self {
        Self {
            monitor_rect,
            work_rect,
            dpi: if dpi == 0 { BASE_DPI } else { dpi },
        }
    }

    /// ワークエリアとモニター領域の差分からタスクバーの位置を推測
    pub fn detect_taskbar_edge(&self) -> TaskbarEdge {
        let m = &self.monitor_rect;
        let w = &self.work_rect;

        if w.bottom < m.bottom {
            TaskbarEdge::Bottom
        } else if w.top > m.top {
            TaskbarEdge::Top
        } else if w.left > m.left {
            TaskbarEdge::Left
        } else if w.right < m.right {
            TaskbarEdge::Right
        } else {
            TaskbarEdge::NoneOrUnknown
        }
    }
}

/// 計算されたポップアップの位置と寸法
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopupGeometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// DPI値に基づいて論理ピクセル値を物理ピクセル値へスケーリング
pub fn scale_value(value: i32, dpi: u32) -> i32 {
    let dpi = if dpi == 0 { BASE_DPI } else { dpi };
    // 四捨五入を行う整数演算: (value * dpi + 48) / 96 (負の値も考慮)
    if value >= 0 {
        (value * dpi as i32 + (BASE_DPI as i32 / 2)) / BASE_DPI as i32
    } else {
        (value * dpi as i32 - (BASE_DPI as i32 / 2)) / BASE_DPI as i32
    }
}

/// プロセスの Per-Monitor V2 DPI awareness を有効化
pub fn enable_per_monitor_dpi_v2() -> bool {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_ok()
    }
}

/// 指定モニターの DPI を取得（取得不可時はフォールバック）
pub fn get_monitor_dpi(hmonitor: HMONITOR) -> u32 {
    let mut dpi_x = 0u32;
    let mut dpi_y = 0u32;
    unsafe {
        if GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok()
            && dpi_x > 0
        {
            dpi_x
        } else {
            let sys_dpi = GetDpiForSystem();
            if sys_dpi > 0 {
                sys_dpi
            } else {
                BASE_DPI
            }
        }
    }
}

/// 指定ウィンドウの DPI を取得
pub fn get_window_dpi(hwnd: HWND) -> u32 {
    unsafe {
        let dpi = GetDpiForWindow(hwnd);
        if dpi > 0 {
            dpi
        } else {
            let sys_dpi = GetDpiForSystem();
            if sys_dpi > 0 {
                sys_dpi
            } else {
                BASE_DPI
            }
        }
    }
}

/// 指定座標（POINT）が含まれるモニターの領域およびDPIを取得
pub fn get_monitor_bounds_from_point(pt: POINT) -> Result<MonitorBounds> {
    unsafe {
        let hmonitor = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        if hmonitor.0.is_null() {
            return Err(Error::from_thread());
        }

        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        if !GetMonitorInfoW(hmonitor, &mut mi).as_bool() {
            return Err(Error::from_thread());
        }

        let dpi = get_monitor_dpi(hmonitor);

        Ok(MonitorBounds::new(
            DisplayRect::from(mi.rcMonitor),
            DisplayRect::from(mi.rcWork),
            dpi,
        ))
    }
}

/// 指定ウィンドウが存在するモニターの領域およびDPIを取得
pub fn get_monitor_bounds_from_window(hwnd: HWND) -> Result<MonitorBounds> {
    unsafe {
        let hmonitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if hmonitor.0.is_null() {
            return Err(Error::from_thread());
        }

        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        if !GetMonitorInfoW(hmonitor, &mut mi).as_bool() {
            return Err(Error::from_thread());
        }

        let dpi = get_monitor_dpi(hmonitor);

        Ok(MonitorBounds::new(
            DisplayRect::from(mi.rcMonitor),
            DisplayRect::from(mi.rcWork),
            dpi,
        ))
    }
}

/// 現在のマウスカーソル位置のモニター領域およびDPIを取得
pub fn get_cursor_monitor_bounds() -> Result<MonitorBounds> {
    let mut pt = POINT::default();
    unsafe {
        GetCursorPos(&mut pt)?;
    }
    get_monitor_bounds_from_point(pt)
}

/// 純粋ロジック: モニター情報と基本設定からポップアップの座標と寸法を算出
///
/// - `monitor`: モニターの矩形、ワークエリア、DPI情報
/// - `base_width`, `base_height`: 96 DPI基準のウィンドウ寸法
/// - `base_margin`: 96 DPI基準のタスクバー/画面端とのマージン
/// - `tray_icon_rect`: トレイアイコンの矩形が取得できる場合はその位置に合わせる（オプション）
pub fn calculate_popup_geometry(
    monitor: &MonitorBounds,
    base_width: i32,
    base_height: i32,
    base_margin: i32,
    tray_icon_rect: Option<DisplayRect>,
) -> PopupGeometry {
    let width = scale_value(base_width, monitor.dpi);
    let height = scale_value(base_height, monitor.dpi);
    let margin = scale_value(base_margin, monitor.dpi);

    let work = &monitor.work_rect;
    let edge = monitor.detect_taskbar_edge();

    // デフォルト: 画面右下（タスクバーが下にある場合、または判定不能時）
    let (mut x, mut y) = match edge {
        TaskbarEdge::Bottom | TaskbarEdge::NoneOrUnknown => {
            let x = work.right - width - margin;
            let y = work.bottom - height - margin;
            (x, y)
        }
        TaskbarEdge::Top => {
            let x = work.right - width - margin;
            let y = work.top + margin;
            (x, y)
        }
        TaskbarEdge::Left => {
            let x = work.left + margin;
            let y = work.bottom - height - margin;
            (x, y)
        }
        TaskbarEdge::Right => {
            let x = work.right - width - margin;
            let y = work.bottom - height - margin;
            (x, y)
        }
    };

    // トレイアイコンの矩形が提供された場合、X座標をトレイアイコンの中心付近に調整
    if let Some(tray) = tray_icon_rect {
        let tray_center_x = (tray.left + tray.right) / 2;
        // アイコン中央にポップアップの中央を合わせる
        let desired_x = tray_center_x - (width / 2);
        // ただし画面端に近すぎる場合はワークエリアの範囲内に収める
        x = desired_x;

        // タスクバーが上部にある場合はトレイアイコンの真下に配置
        if edge == TaskbarEdge::Top {
            y = tray.bottom + margin;
        } else if edge == TaskbarEdge::Bottom {
            // タスクバーが下部にある場合はトレイアイコンの真上に配置
            y = tray.top - height - margin;
        }
    }

    // ワークエリアを逸脱しないように安全にクランプ
    let min_x = work.left;
    let max_x = (work.right - width).max(min_x);
    let min_y = work.top;
    let max_y = (work.bottom - height).max(min_y);

    x = x.clamp(min_x, max_x);
    y = y.clamp(min_y, max_y);

    PopupGeometry {
        x,
        y,
        width,
        height,
    }
}

/// システムトレイ近傍のポップアップ配置座標を自動算出
///
/// ターゲット座標（またはトレイアイコン座標）が渡されればそれを基準とし、
/// 指定がなければ現在のマウスカーソル位置（またはプライマリモニター）を基準とする。
pub fn calculate_tray_popup_geometry(
    target_point: Option<POINT>,
    base_width: i32,
    base_height: i32,
    base_margin: i32,
) -> Result<PopupGeometry> {
    let monitor_bounds = if let Some(pt) = target_point {
        get_monitor_bounds_from_point(pt)?
    } else {
        get_cursor_monitor_bounds().or_else(|_| {
            // カーソル取得失敗時は原点 (0,0) をフォールバック
            get_monitor_bounds_from_point(POINT { x: 0, y: 0 })
        })?
    };

    Ok(calculate_popup_geometry(
        &monitor_bounds,
        base_width,
        base_height,
        base_margin,
        None,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_value_various_dpis() {
        // 96 DPI = 100%
        assert_eq!(scale_value(100, 96), 100);
        assert_eq!(scale_value(320, 96), 320);
        assert_eq!(scale_value(360, 96), 360);
        assert_eq!(scale_value(12, 96), 12);

        // 120 DPI = 125%
        // 100 * 120 / 96 = 125
        assert_eq!(scale_value(100, 120), 125);
        // 320 * 1.25 = 400
        assert_eq!(scale_value(320, 120), 400);
        // 360 * 1.25 = 450
        assert_eq!(scale_value(360, 120), 450);
        // 12 * 1.25 = 15
        assert_eq!(scale_value(12, 120), 15);

        // 144 DPI = 150%
        // 320 * 1.5 = 480
        assert_eq!(scale_value(320, 144), 480);
        // 360 * 1.5 = 540
        assert_eq!(scale_value(360, 144), 540);
        // 12 * 1.5 = 18
        assert_eq!(scale_value(12, 144), 18);

        // 192 DPI = 200%
        // 320 * 2 = 640
        assert_eq!(scale_value(320, 192), 640);
        // 360 * 2 = 720
        assert_eq!(scale_value(360, 192), 720);
        // 12 * 2 = 24
        assert_eq!(scale_value(12, 192), 24);
    }

    #[test]
    fn test_detect_taskbar_edge() {
        // 標準: 下部タスクバー (1920x1080、ワークエリア高さ 1032、タスクバー 48px)
        let bounds_bottom = MonitorBounds::new(
            DisplayRect::new(0, 0, 1920, 1080),
            DisplayRect::new(0, 0, 1920, 1032),
            96,
        );
        assert_eq!(bounds_bottom.detect_taskbar_edge(), TaskbarEdge::Bottom);

        // 上部タスクバー (ワークエリア top = 48)
        let bounds_top = MonitorBounds::new(
            DisplayRect::new(0, 0, 1920, 1080),
            DisplayRect::new(0, 48, 1920, 1080),
            96,
        );
        assert_eq!(bounds_top.detect_taskbar_edge(), TaskbarEdge::Top);

        // 左部タスクバー (ワークエリア left = 60)
        let bounds_left = MonitorBounds::new(
            DisplayRect::new(0, 0, 1920, 1080),
            DisplayRect::new(60, 0, 1920, 1080),
            96,
        );
        assert_eq!(bounds_left.detect_taskbar_edge(), TaskbarEdge::Left);

        // 右部タスクバー (ワークエリア right = 1860)
        let bounds_right = MonitorBounds::new(
            DisplayRect::new(0, 0, 1920, 1080),
            DisplayRect::new(0, 0, 1860, 1080),
            96,
        );
        assert_eq!(bounds_right.detect_taskbar_edge(), TaskbarEdge::Right);
    }

    #[test]
    fn test_calculate_popup_geometry_standard_bottom_taskbar() {
        // FHD 100% (96 DPI)
        // 画面: 0..1920, 0..1080, ワークエリア: 0..1920, 0..1032
        // 基本サイズ: 320x360, マージン: 12
        let monitor = MonitorBounds::new(
            DisplayRect::new(0, 0, 1920, 1080),
            DisplayRect::new(0, 0, 1920, 1032),
            96,
        );

        let geo = calculate_popup_geometry(&monitor, 320, 360, 12, None);

        assert_eq!(geo.width, 320);
        assert_eq!(geo.height, 360);
        // x = 1920 - 320 - 12 = 1588
        assert_eq!(geo.x, 1588);
        // y = 1032 - 360 - 12 = 660
        assert_eq!(geo.y, 660);
    }

    #[test]
    fn test_calculate_popup_geometry_hidpi_150_percent() {
        // 4K 150% (144 DPI)
        // 画面: 0..3840, 0..2160, ワークエリア: 0..3840, 0..2088 (タスクバー72px)
        // 基本サイズ: 320x360, マージン: 12
        let monitor = MonitorBounds::new(
            DisplayRect::new(0, 0, 3840, 2160),
            DisplayRect::new(0, 0, 3840, 2088),
            144,
        );

        let geo = calculate_popup_geometry(&monitor, 320, 360, 12, None);

        // スケール後幅: 320 * 1.5 = 480
        assert_eq!(geo.width, 480);
        // スケール後高さ: 360 * 1.5 = 540
        assert_eq!(geo.height, 540);
        // スケール後マージン: 12 * 1.5 = 18
        // x = 3840 - 480 - 18 = 3342
        assert_eq!(geo.x, 3342);
        // y = 2088 - 540 - 18 = 1530
        assert_eq!(geo.y, 1530);
    }

    #[test]
    fn test_calculate_popup_geometry_multimonitor_negative_coords() {
        // 左側のセカンダリモニター（負の座標: -1920..0, 0..1080）
        // ワークエリア: -1920..0, 0..1032, 96 DPI
        let monitor = MonitorBounds::new(
            DisplayRect::new(-1920, 0, 0, 1080),
            DisplayRect::new(-1920, 0, 0, 1032),
            96,
        );

        let geo = calculate_popup_geometry(&monitor, 320, 360, 12, None);

        assert_eq!(geo.width, 320);
        assert_eq!(geo.height, 360);
        // x = 0 - 320 - 12 = -332
        assert_eq!(geo.x, -332);
        // y = 1032 - 360 - 12 = 660
        assert_eq!(geo.y, 660);
    }

    #[test]
    fn test_calculate_popup_geometry_clamping() {
        // 小さな画面または極端に大きなウィンドウでワークエリアをはみ出さないか確認
        let monitor = MonitorBounds::new(
            DisplayRect::new(0, 0, 400, 300),
            DisplayRect::new(0, 0, 400, 260),
            96,
        );

        // ワークエリア幅400、高さ260に対して、幅380、高さ300を指定
        let geo = calculate_popup_geometry(&monitor, 380, 300, 20, None);

        // 高さはワークエリアを超えているため、y は min_y (0) にクランプされる
        assert_eq!(geo.y, 0);
        assert!(geo.x >= 0 && geo.x <= 400 - 380);
    }

    #[test]
    fn test_calculate_popup_geometry_with_tray_icon() {
        // トレイアイコンの矩形を指定した場合のテスト
        let monitor = MonitorBounds::new(
            DisplayRect::new(0, 0, 1920, 1080),
            DisplayRect::new(0, 0, 1920, 1032),
            96,
        );

        // トレイアイコンが (1800, 1040, 1840, 1072) にある場合 (中心 x=1820)
        let tray_rect = DisplayRect::new(1800, 1040, 1840, 1072);
        let geo = calculate_popup_geometry(&monitor, 320, 360, 12, Some(tray_rect));

        // 幅320の中心は160。中心1820に合わせると desired_x = 1820 - 160 = 1660
        // max_x = 1920 - 320 = 1600 なので 1600 にクランプされる
        assert_eq!(geo.x, 1600);
        // y = 1040 (tray.top) - 360 - 12 = 668
        assert_eq!(geo.y, 668);
    }

    #[test]
    fn test_dpi_awareness_call() {
        // 実行環境でクラッシュせず呼び出せること
        let _ = enable_per_monitor_dpi_v2();
    }
}
