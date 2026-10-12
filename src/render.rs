use chrono::Weekday;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreatePen, CreateSolidBrush,
    DeleteDC, DeleteObject, DrawTextW, FillRect, RoundRect, SelectObject, SetBkMode, SetTextColor,
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, DT_CENTER,
    DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, FW_BOLD, FW_NORMAL, HDC, HFONT, HGDIOBJ,
    OUT_DEFAULT_PRECIS, PS_NULL, PS_SOLID, SRCCOPY, TRANSPARENT,
};

use crate::calendar::{CalendarCell, CalendarModel};
use crate::position::scale_value;

/// RGB値をCOLORREF型へ変換するヘルパー関数
pub const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

/// Windows 11調のモダンUIカラーテーマ
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarTheme {
    /// 全体背景色 (ダークモード: #202020)
    pub bg_color: COLORREF,
    /// 外枠・境界線色 (#383838)
    pub border_color: COLORREF,
    /// ヘッダー年月テキスト色 (#FFFFFF)
    pub header_text_color: COLORREF,
    /// ナビゲーションボタン背景色 (#2C2C2C)
    pub btn_bg_color: COLORREF,
    /// ナビゲーションボタン枠線色 (#3D3D3D)
    pub btn_border_color: COLORREF,
    /// ナビゲーションボタン文字色 (#D0D0D0)
    pub btn_text_color: COLORREF,
    /// 通常の曜日文字色 (月〜金: #A0A0A0)
    pub weekday_text_color: COLORREF,
    /// 日曜日の文字色 (ソフトレッド: #FF6B6B)
    pub weekday_sun_color: COLORREF,
    /// 土曜日の文字色 (ソフトブルー: #5AA9E6)
    pub weekday_sat_color: COLORREF,
    /// 当月の日付文字色 (#F0F0F0)
    pub day_text_current_month: COLORREF,
    /// 前月・翌月のはみ出し日付文字色 (#666666)
    pub day_text_other_month: COLORREF,
    /// 「今日」ハイライト背景色 (Windows 11 Accent Blue: #0078D4)
    pub today_highlight_bg: COLORREF,
    /// 「今日」ハイライト文字色 (#FFFFFF)
    pub today_highlight_text: COLORREF,
}

impl Default for CalendarTheme {
    fn default() -> Self {
        Self {
            bg_color: rgb(32, 32, 32),
            border_color: rgb(56, 56, 56),
            header_text_color: rgb(255, 255, 255),
            btn_bg_color: rgb(44, 44, 44),
            btn_border_color: rgb(62, 62, 62),
            btn_text_color: rgb(215, 215, 215),
            weekday_text_color: rgb(165, 165, 165),
            weekday_sun_color: rgb(255, 107, 107),
            weekday_sat_color: rgb(90, 169, 230),
            day_text_current_month: rgb(242, 242, 242),
            day_text_other_month: rgb(105, 105, 105),
            today_highlight_bg: rgb(0, 120, 212),
            today_highlight_text: rgb(255, 255, 255),
        }
    }
}

/// カレンダーUIのレイアウト幾何情報
#[derive(Debug, Clone, PartialEq)]
pub struct CalendarLayout {
    pub client_rect: RECT,
    pub header_rect: RECT,
    pub title_rect: RECT,
    pub prev_btn_rect: RECT,
    pub today_btn_rect: RECT,
    pub next_btn_rect: RECT,
    pub weekday_header_rect: RECT,
    pub weekday_rects: [RECT; 7],
    pub grid_rect: RECT,
    pub cell_rects: [RECT; 42],
}

impl CalendarLayout {
    /// クライアント領域の寸法とDPIから各コンポーネントの配置矩形を算出
    pub fn calculate(width: i32, height: i32, dpi: u32) -> Self {
        let pad_x = scale_value(16, dpi);
        let pad_y = scale_value(14, dpi);

        let client_rect = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };

        // --- ヘッダー部 ---
        let header_h = scale_value(36, dpi);
        let header_rect = RECT {
            left: pad_x,
            top: pad_y,
            right: width - pad_x,
            bottom: pad_y + header_h,
        };

        let btn_size = scale_value(28, dpi);
        let btn_spacing = scale_value(6, dpi);
        let today_btn_w = scale_value(42, dpi);

        let btn_y = header_rect.top + (header_h - btn_size) / 2;
        let btn_bottom = btn_y + btn_size;

        // 右から順に配置: [Prev] [Today] [Next]
        let next_right = header_rect.right;
        let next_left = next_right - btn_size;
        let next_btn_rect = RECT {
            left: next_left,
            top: btn_y,
            right: next_right,
            bottom: btn_bottom,
        };

        let today_right = next_left - btn_spacing;
        let today_left = today_right - today_btn_w;
        let today_btn_rect = RECT {
            left: today_left,
            top: btn_y,
            right: today_right,
            bottom: btn_bottom,
        };

        let prev_right = today_left - btn_spacing;
        let prev_left = prev_right - btn_size;
        let prev_btn_rect = RECT {
            left: prev_left,
            top: btn_y,
            right: prev_right,
            bottom: btn_bottom,
        };

        let title_rect = RECT {
            left: header_rect.left,
            top: header_rect.top,
            right: prev_left - btn_spacing,
            bottom: header_rect.bottom,
        };

        // --- 曜日行 ---
        let weekday_top = header_rect.bottom + scale_value(6, dpi);
        let weekday_h = scale_value(22, dpi);
        let weekday_header_rect = RECT {
            left: pad_x,
            top: weekday_top,
            right: width - pad_x,
            bottom: weekday_top + weekday_h,
        };

        let grid_w = weekday_header_rect.right - weekday_header_rect.left;
        let mut weekday_rects = [RECT::default(); 7];
        for col in 0..7 {
            let left = pad_x + (col as i32 * grid_w) / 7;
            let right = pad_x + ((col as i32 + 1) * grid_w) / 7;
            weekday_rects[col] = RECT {
                left,
                top: weekday_top,
                right,
                bottom: weekday_top + weekday_h,
            };
        }

        // --- カレンダーグリッド (6行 x 7列 = 42マス) ---
        let grid_top = weekday_header_rect.bottom + scale_value(4, dpi);
        let grid_bottom = height - pad_y;
        let grid_h = (grid_bottom - grid_top).max(0);

        let grid_rect = RECT {
            left: pad_x,
            top: grid_top,
            right: width - pad_x,
            bottom: grid_bottom,
        };

        let mut cell_rects = [RECT::default(); 42];
        for idx in 0..42 {
            let row = (idx / 7) as i32;
            let col = (idx % 7) as i32;

            let left = pad_x + (col * grid_w) / 7;
            let right = pad_x + ((col + 1) * grid_w) / 7;
            let top = grid_top + (row * grid_h) / 6;
            let bottom = grid_top + ((row + 1) * grid_h) / 6;

            cell_rects[idx] = RECT {
                left,
                top,
                right,
                bottom,
            };
        }

        Self {
            client_rect,
            header_rect,
            title_rect,
            prev_btn_rect,
            today_btn_rect,
            next_btn_rect,
            weekday_header_rect,
            weekday_rects,
            grid_rect,
            cell_rects,
        }
    }

    /// 矩形内に指定座標が含まれているかを判定
    pub fn contains(rect: &RECT, pt: POINT) -> bool {
        pt.x >= rect.left && pt.x < rect.right && pt.y >= rect.top && pt.y < rect.bottom
    }

    pub fn hit_test_prev(&self, pt: POINT) -> bool {
        Self::contains(&self.prev_btn_rect, pt)
    }

    pub fn hit_test_next(&self, pt: POINT) -> bool {
        Self::contains(&self.next_btn_rect, pt)
    }

    pub fn hit_test_today(&self, pt: POINT) -> bool {
        Self::contains(&self.today_btn_rect, pt)
    }

    pub fn hit_test_cell(&self, pt: POINT) -> Option<usize> {
        for (i, cell) in self.cell_rects.iter().enumerate() {
            if Self::contains(cell, pt) {
                return Some(i);
            }
        }
        None
    }
}

// --- GDI RAII ラッパー ---

struct AutoDeleteGdi<T: Into<HGDIOBJ> + Copy>(T);

impl<T: Into<HGDIOBJ> + Copy> Drop for AutoDeleteGdi<T> {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.0.into());
        }
    }
}

struct AutoSelectObject {
    hdc: HDC,
    prev: HGDIOBJ,
}

impl AutoSelectObject {
    pub fn select(hdc: HDC, obj: impl Into<HGDIOBJ>) -> Self {
        let prev = unsafe { SelectObject(hdc, obj.into()) };
        Self { hdc, prev }
    }
}

impl Drop for AutoSelectObject {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.hdc, self.prev);
        }
    }
}

struct AutoDeleteDc(HDC);

impl Drop for AutoDeleteDc {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
}

/// GDIフォント作成ヘルパー
fn create_gdi_font(height: i32, weight: i32, face_name: PCWSTR) -> AutoDeleteGdi<HFONT> {
    let hfont = unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            face_name,
        )
    };
    AutoDeleteGdi(hfont)
}

/// カレンダー描画エンジン
pub struct CalendarRenderer;

impl CalendarRenderer {
    /// ダブルバッファリングを用いて指定のHDCへカレンダーUI全体を描画
    pub fn paint(
        hdc: HDC,
        width: i32,
        height: i32,
        dpi: u32,
        model: &CalendarModel,
        theme: &CalendarTheme,
    ) {
        if width <= 0 || height <= 0 {
            return;
        }

        let layout = CalendarLayout::calculate(width, height, dpi);

        // 1. バックバッファDCとビットマップの作成
        let mem_dc = unsafe { CreateCompatibleDC(Some(hdc)) };
        if mem_dc.0.is_null() {
            return;
        }
        let _auto_dc = AutoDeleteDc(mem_dc);

        let mem_bmp = unsafe { CreateCompatibleBitmap(hdc, width, height) };
        if mem_bmp.0.is_null() {
            return;
        }
        let auto_bmp = AutoDeleteGdi(mem_bmp);
        let _sel_bmp = AutoSelectObject::select(mem_dc, auto_bmp.0);

        // 2. 全体背景の描画
        let bg_brush = AutoDeleteGdi(unsafe { CreateSolidBrush(theme.bg_color) });
        unsafe {
            let _ = FillRect(mem_dc, &layout.client_rect, bg_brush.0);
            let _ = SetBkMode(mem_dc, TRANSPARENT);
        }

        // フォント定義 (DPI対応)
        let font_face = w!("Segoe UI");
        let title_font = create_gdi_font(-scale_value(16, dpi), FW_BOLD.0 as i32, font_face);
        let btn_font = create_gdi_font(-scale_value(11, dpi), FW_NORMAL.0 as i32, font_face);
        let weekday_font = create_gdi_font(-scale_value(11, dpi), FW_NORMAL.0 as i32, font_face);
        let body_font = create_gdi_font(-scale_value(13, dpi), FW_NORMAL.0 as i32, font_face);
        let today_font = create_gdi_font(-scale_value(13, dpi), FW_BOLD.0 as i32, font_face);

        // 3. ヘッダー部の描画 (タイトル & ナビゲーションボタン)
        Self::draw_header(mem_dc, &layout, model, theme, dpi, &title_font.0, &btn_font.0);

        // 4. 曜日行の描画 (日, 月, 火, 水, 木, 金, 土)
        Self::draw_weekdays(mem_dc, &layout, theme, &weekday_font.0);

        // 5. カレンダーグリッドの描画 (42マス & 当日ハイライト)
        Self::draw_calendar_grid(
            mem_dc,
            &layout,
            model,
            theme,
            dpi,
            &body_font.0,
            &today_font.0,
        );

        // 6. フロントバッファへ高速転送 (BitBlt)
        unsafe {
            let _ = BitBlt(hdc, 0, 0, width, height, Some(mem_dc), 0, 0, SRCCOPY);
        }
    }

    /// ヘッダー描画
    fn draw_header(
        hdc: HDC,
        layout: &CalendarLayout,
        model: &CalendarModel,
        theme: &CalendarTheme,
        dpi: u32,
        title_font: &HFONT,
        btn_font: &HFONT,
    ) {
        // 年月タイトル
        {
            let _sel = AutoSelectObject::select(hdc, *title_font);
            unsafe {
                let _ = SetTextColor(hdc, theme.header_text_color);
            }
            let title_text = format!("{}年 {}月", model.view_year, model.view_month);
            let mut title_utf16: Vec<u16> = title_text.encode_utf16().collect();
            let mut rect = layout.title_rect;
            unsafe {
                let _ = DrawTextW(
                    hdc,
                    &mut title_utf16,
                    &mut rect,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                );
            }
        }

        // ナビゲーションボタン (前月 "<" / 今日 "今日" / 翌月 ">")
        let corner = scale_value(6, dpi);
        let btn_pen = AutoDeleteGdi(unsafe { CreatePen(PS_SOLID, 1, theme.btn_border_color) });
        let btn_brush = AutoDeleteGdi(unsafe { CreateSolidBrush(theme.btn_bg_color) });

        let draw_btn = |rect: &RECT, text: &str| {
            let _sel_pen = AutoSelectObject::select(hdc, btn_pen.0);
            let _sel_brush = AutoSelectObject::select(hdc, btn_brush.0);
            unsafe {
                let _ = RoundRect(
                    hdc,
                    rect.left,
                    rect.top,
                    rect.right,
                    rect.bottom,
                    corner,
                    corner,
                );
            }

            let _sel_font = AutoSelectObject::select(hdc, *btn_font);
            unsafe {
                let _ = SetTextColor(hdc, theme.btn_text_color);
            }
            let mut utf16: Vec<u16> = text.encode_utf16().collect();
            let mut text_rect = *rect;
            unsafe {
                let _ = DrawTextW(
                    hdc,
                    &mut utf16,
                    &mut text_rect,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                );
            }
        };

        draw_btn(&layout.prev_btn_rect, "<");
        draw_btn(&layout.today_btn_rect, "今日");
        draw_btn(&layout.next_btn_rect, ">");
    }

    /// 曜日行描画
    fn draw_weekdays(
        hdc: HDC,
        layout: &CalendarLayout,
        theme: &CalendarTheme,
        weekday_font: &HFONT,
    ) {
        let _sel = AutoSelectObject::select(hdc, *weekday_font);
        let weekdays = ["日", "月", "火", "水", "木", "金", "土"];

        for (col, name) in weekdays.iter().enumerate() {
            let text_color = match col {
                0 => theme.weekday_sun_color,
                6 => theme.weekday_sat_color,
                _ => theme.weekday_text_color,
            };

            unsafe {
                let _ = SetTextColor(hdc, text_color);
            }
            let mut utf16: Vec<u16> = name.encode_utf16().collect();
            let mut rect = layout.weekday_rects[col];
            unsafe {
                let _ = DrawTextW(
                    hdc,
                    &mut utf16,
                    &mut rect,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                );
            }
        }
    }

    /// カレンダー日付セル描画 (当日ハイライト含む)
    fn draw_calendar_grid(
        hdc: HDC,
        layout: &CalendarLayout,
        model: &CalendarModel,
        theme: &CalendarTheme,
        dpi: u32,
        body_font: &HFONT,
        today_font: &HFONT,
    ) {
        let grid = model.generate_grid();
        let today_bg_brush =
            AutoDeleteGdi(unsafe { CreateSolidBrush(theme.today_highlight_bg) });
        let today_pen = AutoDeleteGdi(unsafe { CreatePen(PS_NULL, 0, theme.today_highlight_bg) });

        let highlight_corner = scale_value(6, dpi);
        let highlight_pad = scale_value(2, dpi);

        for (idx, cell) in grid.iter().enumerate() {
            let rect = layout.cell_rects[idx];

            if cell.is_today {
                // 当日セルの背景ハイライト（角丸ピル/角丸矩形）
                let hl_rect = RECT {
                    left: rect.left + highlight_pad,
                    top: rect.top + highlight_pad,
                    right: rect.right - highlight_pad,
                    bottom: rect.bottom - highlight_pad,
                };

                let _sel_pen = AutoSelectObject::select(hdc, today_pen.0);
                let _sel_brush = AutoSelectObject::select(hdc, today_bg_brush.0);
                unsafe {
                    let _ = RoundRect(
                        hdc,
                        hl_rect.left,
                        hl_rect.top,
                        hl_rect.right,
                        hl_rect.bottom,
                        highlight_corner,
                        highlight_corner,
                    );
                }

                // 当日の文字（白・太字）
                let _sel_font = AutoSelectObject::select(hdc, *today_font);
                unsafe {
                    let _ = SetTextColor(hdc, theme.today_highlight_text);
                }
                Self::draw_cell_number(hdc, cell, rect);
            } else {
                // 通常日の文字
                let text_color = if !cell.is_current_month {
                    theme.day_text_other_month
                } else {
                    match cell.weekday {
                        Weekday::Sun => theme.weekday_sun_color,
                        Weekday::Sat => theme.weekday_sat_color,
                        _ => theme.day_text_current_month,
                    }
                };

                let _sel_font = AutoSelectObject::select(hdc, *body_font);
                unsafe {
                    let _ = SetTextColor(hdc, text_color);
                }
                Self::draw_cell_number(hdc, cell, rect);
            }
        }
    }

    /// セルの日付数値を中央揃えで描画
    fn draw_cell_number(hdc: HDC, cell: &CalendarCell, mut rect: RECT) {
        let text = format!("{}", cell.day);
        let mut utf16: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            let _ = DrawTextW(
                hdc,
                &mut utf16,
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use windows::Win32::Graphics::Gdi::{GetDC, ReleaseDC};

    #[test]
    fn test_rgb_macro() {
        let white = rgb(255, 255, 255);
        assert_eq!(white.0, 0x00FFFFFF);

        let black = rgb(0, 0, 0);
        assert_eq!(black.0, 0x00000000);

        let blue = rgb(0, 120, 212);
        assert_eq!(blue.0, 0x00D47800); // 0x00BBGGRR -> 212 is 0xD4, 120 is 0x78
    }

    #[test]
    fn test_layout_calculation_coverage_and_bounds() {
        let width = 320;
        let height = 360;
        let dpi = 96;

        let layout = CalendarLayout::calculate(width, height, dpi);

        assert_eq!(layout.client_rect.right, 320);
        assert_eq!(layout.client_rect.bottom, 360);

        // ヘッダーがクライアント領域内に収まっていること
        assert!(layout.header_rect.left < layout.header_rect.right);
        assert!(layout.header_rect.top < layout.header_rect.bottom);
        assert!(layout.header_rect.bottom <= layout.weekday_header_rect.top);

        // ボタンとタイトルが重ならず順番に並んでいること
        assert!(layout.title_rect.right <= layout.prev_btn_rect.left);
        assert!(layout.prev_btn_rect.right <= layout.today_btn_rect.left);
        assert!(layout.today_btn_rect.right <= layout.next_btn_rect.left);
        assert!(layout.next_btn_rect.right <= layout.header_rect.right);

        // 曜日セルが7列存在し、境界が正しく連続していること
        assert_eq!(layout.weekday_rects.len(), 7);
        for col in 0..6 {
            assert_eq!(
                layout.weekday_rects[col].right,
                layout.weekday_rects[col + 1].left
            );
        }

        // カレンダーグリッドが42マス存在し、各マスが妥当なサイズを持つこと
        assert_eq!(layout.cell_rects.len(), 42);
        for idx in 0..42 {
            let cell = &layout.cell_rects[idx];
            assert!(
                cell.right > cell.left,
                "Cell {} width must be positive",
                idx
            );
            assert!(
                cell.bottom > cell.top,
                "Cell {} height must be positive",
                idx
            );
            assert!(cell.left >= layout.grid_rect.left);
            assert!(cell.right <= layout.grid_rect.right);
            assert!(cell.top >= layout.grid_rect.top);
            assert!(cell.bottom <= layout.grid_rect.bottom);
        }
    }

    #[test]
    fn test_layout_hit_testing() {
        let layout = CalendarLayout::calculate(320, 360, 96);

        // Prevボタン中心のヒットテスト
        let prev_center = POINT {
            x: (layout.prev_btn_rect.left + layout.prev_btn_rect.right) / 2,
            y: (layout.prev_btn_rect.top + layout.prev_btn_rect.bottom) / 2,
        };
        assert!(layout.hit_test_prev(prev_center));
        assert!(!layout.hit_test_next(prev_center));
        assert!(!layout.hit_test_today(prev_center));

        // Todayボタン中心のヒットテスト
        let today_center = POINT {
            x: (layout.today_btn_rect.left + layout.today_btn_rect.right) / 2,
            y: (layout.today_btn_rect.top + layout.today_btn_rect.bottom) / 2,
        };
        assert!(layout.hit_test_today(today_center));

        // Nextボタン中心のヒットテスト
        let next_center = POINT {
            x: (layout.next_btn_rect.left + layout.next_btn_rect.right) / 2,
            y: (layout.next_btn_rect.top + layout.next_btn_rect.bottom) / 2,
        };
        assert!(layout.hit_test_next(next_center));

        // セル[10]中心のヒットテスト
        let cell_center = POINT {
            x: (layout.cell_rects[10].left + layout.cell_rects[10].right) / 2,
            y: (layout.cell_rects[10].top + layout.cell_rects[10].bottom) / 2,
        };
        assert_eq!(layout.hit_test_cell(cell_center), Some(10));

        // 画面外の点
        let outside = POINT { x: -10, y: -10 };
        assert_eq!(layout.hit_test_cell(outside), None);
        assert!(!layout.hit_test_prev(outside));
    }

    #[test]
    fn test_renderer_paint_headless_execution() {
        // 実機DCを取得して互換描画パイプラインの動作を検証
        let hdc = unsafe { GetDC(None) };
        assert!(!hdc.0.is_null());

        let today = NaiveDate::from_ymd_opt(2026, 10, 12).unwrap();
        let model = CalendarModel::with_date(today, 2026, 10);
        let theme = CalendarTheme::default();

        // 96 DPI での描画実行（クラッシュ・リソースリークがないことを検証）
        CalendarRenderer::paint(hdc, 320, 360, 96, &model, &theme);

        // 144 DPI (150%) での描画実行
        CalendarRenderer::paint(hdc, 480, 540, 144, &model, &theme);

        unsafe {
            let _ = ReleaseDC(None, hdc);
        }
    }
}
