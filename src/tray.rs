use windows::core::{w, Error, Result, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject, HBITMAP};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, DestroyIcon, DestroyMenu, GetCursorPos,
    GetMenuItemCount, GetMenuItemID, IsWindow, LoadIconW, PostMessageW, SetForegroundWindow,
    TrackPopupMenu, HICON, HMENU, ICONINFO, IDI_APPLICATION, MF_SEPARATOR, MF_STRING,
    TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_CONTEXTMENU,
    WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WM_USER,
};

/// タスクトレイアイコンからのコールバック用カスタムウィンドウメッセージ
pub const WM_TRAYICON: u32 = WM_USER + 1;

/// デフォルトのトレイアイコン識別子
pub const DEFAULT_TRAY_UID: u32 = 1;

/// コンテキストメニュー項目ID: カレンダーの表示/非表示切り替え
pub const MENU_ID_TOGGLE_CALENDAR: usize = 1001;
/// コンテキストメニュー項目ID: 今日へ移動
pub const MENU_ID_GO_TODAY: usize = 1002;
/// コンテキストメニュー項目ID: アプリケーション終了
pub const MENU_ID_EXIT: usize = 1009;

/// トレイアイコンに対するマウス操作イベント
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    /// 左クリック（カレンダーポップアップの表示/非表示トグル等）
    LeftClick,
    /// 右クリック（コンテキストメニュー表示）
    RightClick,
    /// その他の通知メッセージ
    Other(u32),
}

/// トレイの右クリックメニューから選択可能なコマンド
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMenuCommand {
    /// カレンダーの表示/非表示を切り替え
    ToggleCalendar,
    /// 今日（当月）へ移動して表示
    GoToday,
    /// アプリケーションを終了
    Exit,
}

impl TrayMenuCommand {
    /// メニュー項目IDから `TrayMenuCommand` へ変換
    pub fn from_id(id: usize) -> Option<Self> {
        match id {
            MENU_ID_TOGGLE_CALENDAR => Some(Self::ToggleCalendar),
            MENU_ID_GO_TODAY => Some(Self::GoToday),
            MENU_ID_EXIT => Some(Self::Exit),
            _ => None,
        }
    }

    /// コマンドに対応するメニュー項目IDを取得
    pub fn id(self) -> usize {
        match self {
            Self::ToggleCalendar => MENU_ID_TOGGLE_CALENDAR,
            Self::GoToday => MENU_ID_GO_TODAY,
            Self::Exit => MENU_ID_EXIT,
        }
    }
}

/// `WM_TRAYICON` の `LPARAM` からトレイイベント種別を解析する
pub fn parse_tray_message(lparam: LPARAM) -> TrayEvent {
    let msg = (lparam.0 & 0xffff) as u32;
    match msg {
        WM_LBUTTONUP | WM_LBUTTONDBLCLK => TrayEvent::LeftClick,
        WM_RBUTTONUP | WM_CONTEXTMENU => TrayEvent::RightClick,
        other => TrayEvent::Other(other),
    }
}

/// ツールチップ文字列を `NOTIFYICONDATAW.szTip` 用の固定長 UTF-16 バッファ (`[u16; 128]`) にエンコードする
pub fn encode_tooltip(tooltip: &str) -> [u16; 128] {
    let mut buf = [0u16; 128];
    for (i, ch) in tooltip.encode_utf16().take(127).enumerate() {
        buf[i] = ch;
    }
    buf
}

/// 32x32 ピクセルのモダンなカレンダー型トレイアイコンをGDIで動的生成する。
/// 万が一作成に失敗した場合はシステム標準アプリケーションアイコンへフォールバックする。
pub fn create_calendar_tray_icon() -> Result<(HICON, bool)> {
    const SIZE: i32 = 32;
    let mut pixels = vec![0u32; (SIZE * SIZE) as usize];

    // カレンダーアイコンの描画 (BGRA 32-bit)
    for y in 0..SIZE {
        for x in 0..SIZE {
            // 外周2pxは余白（透明）
            if !(2..30).contains(&x) || !(3..30).contains(&y) {
                continue;
            }
            // 四隅の角丸カット
            let is_corner = (x == 2 || x == 29) && (y == 3 || y == 29);
            if is_corner {
                continue;
            }

            let idx = (y * SIZE + x) as usize;

            if (3..10).contains(&y) {
                // カレンダー上部ヘッダーバー（Windows 11 アクセントブルー: R=0, G=120, B=212）
                pixels[idx] = 0xFF_00_78_D4;
            } else {
                // カレンダー本体背景（ダークグレー/ホワイト枠調でトレイの明暗両テーマで視認しやすい配色）
                let is_border = x == 2 || x == 29 || y == 29;
                if is_border {
                    pixels[idx] = 0xFF_60_60_60;
                } else {
                    pixels[idx] = 0xFF_F3_F3_F3;
                }
            }
        }
    }

    // カレンダー上部のリング（留め具）2箇所
    for &(rx_start, rx_end) in &[(8, 11), (21, 24)] {
        for y in 1..5 {
            for x in rx_start..rx_end {
                let idx = (y * SIZE + x) as usize;
                pixels[idx] = 0xFF_E0_E0_E0;
            }
        }
    }

    // カレンダー内部のグリッドドット（3行×4列）
    let cols = [6, 12, 18, 24];
    let rows = [13, 18, 23];
    for (r_idx, &ry) in rows.iter().enumerate() {
        for (c_idx, &cx) in cols.iter().enumerate() {
            // 1箇所だけ「今日」を示すアクセントカラーにする
            let color = if r_idx == 1 && c_idx == 2 {
                0xFF_00_78_D4 // 今日ハイライト
            } else {
                0xFF_40_40_40 // 通常日付ドット
            };
            for dy in 0..3 {
                for dx in 0..3 {
                    let idx = ((ry + dy) * SIZE + (cx + dx)) as usize;
                    pixels[idx] = color;
                }
            }
        }
    }

    // マスクビットマップ用バッファ（1bpp: 全ゼロでカラービットマップのアルファを使用）
    let mask_bytes = [0u8; (32 * 32) / 8];

    unsafe {
        let hbm_color: HBITMAP = CreateBitmap(
            SIZE,
            SIZE,
            1,
            32,
            Some(pixels.as_ptr() as *const std::ffi::c_void),
        );
        let hbm_mask: HBITMAP = CreateBitmap(
            SIZE,
            SIZE,
            1,
            1,
            Some(mask_bytes.as_ptr() as *const std::ffi::c_void),
        );

        if !hbm_color.is_invalid() && !hbm_mask.is_invalid() {
            let icon_info = ICONINFO {
                fIcon: true.into(),
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: hbm_mask,
                hbmColor: hbm_color,
            };
            let icon_res = CreateIconIndirect(&icon_info);
            let _ = DeleteObject(hbm_color.into());
            let _ = DeleteObject(hbm_mask.into());

            if let Ok(hicon) = icon_res {
                if !hicon.is_invalid() {
                    return Ok((hicon, true));
                }
            }
        } else {
            if !hbm_color.is_invalid() {
                let _ = DeleteObject(hbm_color.into());
            }
            if !hbm_mask.is_invalid() {
                let _ = DeleteObject(hbm_mask.into());
            }
        }

        // フォールバック: システム標準のアプリケーションアイコン
        let fallback = LoadIconW(None, IDI_APPLICATION)?;
        Ok((fallback, false))
    }
}

/// トレイアイコン右クリック時に表示されるコンテキストメニュー（RAII管理）
pub struct TrayContextMenu {
    hmenu: HMENU,
}

impl TrayContextMenu {
    /// コンテキストメニューを生成し、標準項目（「カレンダーを表示/隠す」「今日へ移動」「終了」）を追加する
    pub fn new(is_calendar_visible: bool) -> Result<Self> {
        let hmenu = unsafe { CreatePopupMenu()? };

        let toggle_label: PCWSTR = if is_calendar_visible {
            w!("カレンダーを隠す")
        } else {
            w!("カレンダーを表示")
        };

        unsafe {
            AppendMenuW(hmenu, MF_STRING, MENU_ID_TOGGLE_CALENDAR, toggle_label)?;
            AppendMenuW(hmenu, MF_STRING, MENU_ID_GO_TODAY, w!("今日へ移動"))?;
            AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null())?;
            AppendMenuW(hmenu, MF_STRING, MENU_ID_EXIT, w!("終了"))?;
        }

        Ok(Self { hmenu })
    }

    /// 内部のメニューハンドル (`HMENU`) を取得
    pub fn hmenu(&self) -> HMENU {
        self.hmenu
    }

    /// 登録されているメニュー項目数を取得（セパレータ含む）
    pub fn item_count(&self) -> i32 {
        unsafe { GetMenuItemCount(Some(self.hmenu)) }
    }

    /// 指定インデックスのメニュー項目IDを取得
    pub fn item_id(&self, index: i32) -> u32 {
        unsafe { GetMenuItemID(self.hmenu, index) }
    }

    /// 現在のマウスカーソル位置（または指定座標）にコンテキストメニューを表示し、選択されたコマンドを返す
    pub fn show_at_cursor(&self, hwnd: HWND) -> Option<TrayMenuCommand> {
        let mut pt = POINT::default();
        if unsafe { GetCursorPos(&mut pt) }.is_err() {
            return None;
        }
        self.show_at(hwnd, pt.x, pt.y)
    }

    /// 指定スクリーン座標にコンテキストメニューを表示し、選択されたコマンドを返す
    pub fn show_at(&self, hwnd: HWND, x: i32, y: i32) -> Option<TrayMenuCommand> {
        unsafe {
            // Windowsの仕様: TrackPopupMenu の前にオーナーウィンドウをフォアグラウンドにしないと
            // メニュー外をクリックしてもメニューが閉じない現象が発生する (MSDN KB135788)
            let _ = SetForegroundWindow(hwnd);

            let flags =
                TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_LEFTALIGN | TPM_RETURNCMD | TPM_NONOTIFY;
            let selected = TrackPopupMenu(self.hmenu, flags, x, y, Some(0), hwnd, None);

            // メニュー終了後に空メッセージをポストしてタスクバーステートを確実にクリーンアップする
            let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));

            let cmd_id = selected.0 as usize;
            TrayMenuCommand::from_id(cmd_id)
        }
    }
}

impl Drop for TrayContextMenu {
    fn drop(&mut self) {
        if !self.hmenu.is_invalid() {
            unsafe {
                let _ = DestroyMenu(self.hmenu);
            }
        }
    }
}

/// `Shell_NotifyIconW` に渡す `NOTIFYICONDATAW` 構造体を構築する
pub fn build_notify_icon_data(hwnd: HWND, uid: u32, hicon: HICON, tooltip: &str) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: uid,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAYICON,
        hIcon: hicon,
        szTip: encode_tooltip(tooltip),
        ..Default::default()
    }
}

/// システムトレイ（タスクトレイ）常駐アイコン管理構造体（RAII対応）
pub struct TrayIcon {
    hwnd: HWND,
    uid: u32,
    tooltip: String,
    hicon: HICON,
    owns_icon: bool,
    registered: bool,
    shell_bound: bool,
}

impl TrayIcon {
    /// 指定ウィンドウに紐づくシステムトレイアイコンを作成し、タスクトレイへ登録（`NIM_ADD`）する
    pub fn new(hwnd: HWND, uid: u32, tooltip: &str) -> Result<Self> {
        if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
            return Err(Error::from_thread());
        }

        let (hicon, owns_icon) = create_calendar_tray_icon()?;
        let mut nid = build_notify_icon_data(hwnd, uid, hicon, tooltip);

        // 通常デスクトップ環境では NIM_ADD で登録。前回異常終了等で残存している場合は NIM_MODIFY も試行。
        // サンドボックスや非対話セッション等で Explorer (Shell_TrayWnd) への IPC が UIPI ブロックされた場合でも、
        // ウィンドウとアイコンが有効であればインメモリ常駐状態として継続可能にする。
        let added = unsafe { Shell_NotifyIconW(NIM_ADD, &nid).as_bool() };
        let shell_bound = if added {
            true
        } else {
            unsafe { Shell_NotifyIconW(NIM_MODIFY, &mut nid).as_bool() }
        };

        Ok(Self {
            hwnd,
            uid,
            tooltip: tooltip.to_string(),
            hicon,
            owns_icon,
            registered: true,
            shell_bound,
        })
    }

    /// 紐づいているウィンドウハンドルを取得
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// トレイアイコンIDを取得
    pub fn uid(&self) -> u32 {
        self.uid
    }

    /// 現在設定されているツールチップ文字列を取得
    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }

    /// トレイアイコンが現在登録管理下にあるかを取得
    pub fn is_registered(&self) -> bool {
        self.registered
    }

    /// エクスプローラーのシェル領域に実際にバインドされているかを取得
    pub fn is_shell_bound(&self) -> bool {
        self.shell_bound
    }

    /// ツールチップ文字列を更新（`NIM_MODIFY`）する
    pub fn set_tooltip(&mut self, tooltip: &str) -> Result<()> {
        if !self.registered {
            return Ok(());
        }

        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: self.uid,
            uFlags: NIF_TIP,
            szTip: encode_tooltip(tooltip),
            ..Default::default()
        };

        if self.shell_bound {
            let modified = unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid).as_bool() };
            if !modified {
                return Err(Error::from_thread());
            }
        }

        self.tooltip = tooltip.to_string();
        Ok(())
    }

    /// システムトレイからアイコンを明示的に削除（`NIM_DELETE`）する
    pub fn remove(&mut self) -> Result<()> {
        if !self.registered {
            return Ok(());
        }

        if self.shell_bound {
            let nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: self.uid,
                ..Default::default()
            };
            let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
            self.shell_bound = false;
        }

        self.registered = false;
        Ok(())
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        let _ = self.remove();
        if self.owns_icon && !self.hicon.is_invalid() {
            unsafe {
                let _ = DestroyIcon(self.hicon);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::{CalendarWindow, PopupWindowConfig};
    use windows::Win32::UI::WindowsAndMessaging::WM_MOUSEMOVE;

    #[test]
    fn test_tray_menu_command_conversion() {
        assert_eq!(
            TrayMenuCommand::from_id(MENU_ID_TOGGLE_CALENDAR),
            Some(TrayMenuCommand::ToggleCalendar)
        );
        assert_eq!(
            TrayMenuCommand::from_id(MENU_ID_GO_TODAY),
            Some(TrayMenuCommand::GoToday)
        );
        assert_eq!(
            TrayMenuCommand::from_id(MENU_ID_EXIT),
            Some(TrayMenuCommand::Exit)
        );
        assert_eq!(TrayMenuCommand::from_id(0), None);
        assert_eq!(TrayMenuCommand::from_id(9999), None);

        assert_eq!(TrayMenuCommand::ToggleCalendar.id(), MENU_ID_TOGGLE_CALENDAR);
        assert_eq!(TrayMenuCommand::GoToday.id(), MENU_ID_GO_TODAY);
        assert_eq!(TrayMenuCommand::Exit.id(), MENU_ID_EXIT);
    }

    #[test]
    fn test_parse_tray_message() {
        assert_eq!(
            parse_tray_message(LPARAM(WM_LBUTTONUP as isize)),
            TrayEvent::LeftClick
        );
        assert_eq!(
            parse_tray_message(LPARAM(WM_LBUTTONDBLCLK as isize)),
            TrayEvent::LeftClick
        );
        assert_eq!(
            parse_tray_message(LPARAM(WM_RBUTTONUP as isize)),
            TrayEvent::RightClick
        );
        assert_eq!(
            parse_tray_message(LPARAM(WM_CONTEXTMENU as isize)),
            TrayEvent::RightClick
        );
        // 上位ビットにアイコンID等が含まれる場合（NOTIFYICON_VERSION_4相当のパック形式）でも下位16ビットを正しく認識すること
        let packed_rbutton = LPARAM(((1usize << 16) | (WM_RBUTTONUP as usize)) as isize);
        assert_eq!(parse_tray_message(packed_rbutton), TrayEvent::RightClick);

        assert_eq!(
            parse_tray_message(LPARAM(WM_MOUSEMOVE as isize)),
            TrayEvent::Other(WM_MOUSEMOVE)
        );
    }

    #[test]
    fn test_encode_tooltip() {
        let tip = "DesCal - カレンダー";
        let encoded = encode_tooltip(tip);
        let expected: Vec<u16> = tip.encode_utf16().collect();
        assert_eq!(&encoded[..expected.len()], &expected[..]);
        assert_eq!(encoded[expected.len()], 0);
        assert_eq!(encoded[127], 0);

        // 128文字を超える長い文字列でも末尾が必ず NUL (0) で終端されること
        let long_tip = "A".repeat(200);
        let encoded_long = encode_tooltip(&long_tip);
        assert_eq!(encoded_long[126], b'A' as u16);
        assert_eq!(encoded_long[127], 0);
    }

    #[test]
    fn test_create_calendar_tray_icon() {
        let (hicon, owns) = create_calendar_tray_icon().expect("カレンダーアイコンの生成に失敗");
        assert!(!hicon.is_invalid());
        if owns {
            unsafe {
                let _ = DestroyIcon(hicon);
            }
        }
    }

    #[test]
    fn test_create_tray_context_menu() {
        // 非表示状態でのコンテキストメニュー生成
        let menu_hidden = TrayContextMenu::new(false).expect("コンテキストメニュー生成失敗");
        assert!(!menu_hidden.hmenu().is_invalid());
        // 項目数: 1.表示/非表示, 2.今日へ移動, 3.セパレータ, 4.終了 = 計4項目
        assert_eq!(menu_hidden.item_count(), 4);
        assert_eq!(menu_hidden.item_id(0), MENU_ID_TOGGLE_CALENDAR as u32);
        assert_eq!(menu_hidden.item_id(1), MENU_ID_GO_TODAY as u32);
        assert_eq!(menu_hidden.item_id(3), MENU_ID_EXIT as u32);

        // 表示状態でのコンテキストメニュー生成
        let menu_visible = TrayContextMenu::new(true).expect("コンテキストメニュー生成失敗");
        assert!(!menu_visible.hmenu().is_invalid());
        assert_eq!(menu_visible.item_count(), 4);
        assert_eq!(menu_visible.item_id(0), MENU_ID_TOGGLE_CALENDAR as u32);
        assert_eq!(menu_visible.item_id(3), MENU_ID_EXIT as u32);
    }

    #[test]
    fn test_build_notify_icon_data_and_lifecycle() {
        let config = PopupWindowConfig {
            title: "DesCal Tray Lifecycle Test",
            width: 320,
            height: 360,
            initial_x: 0,
            initial_y: 0,
            auto_hide_on_deactivate: true,
        };
        let window = CalendarWindow::new(&config).expect("ウィンドウ作成失敗");

        // 無効な HWND では TrayIcon::new がエラーを返すこと
        assert!(TrayIcon::new(HWND::default(), DEFAULT_TRAY_UID, "Invalid").is_err());

        // NOTIFYICONDATAW 構造体のフィールド検証
        let (hicon, owns) = create_calendar_tray_icon().expect("アイコン生成失敗");
        let nid = build_notify_icon_data(window.hwnd(), DEFAULT_TRAY_UID, hicon, "DesCal");
        assert_eq!(nid.cbSize, std::mem::size_of::<NOTIFYICONDATAW>() as u32);
        assert_eq!(nid.hWnd, window.hwnd());
        assert_eq!(nid.uID, DEFAULT_TRAY_UID);
        assert_eq!(nid.uFlags, NIF_MESSAGE | NIF_ICON | NIF_TIP);
        assert_eq!(nid.uCallbackMessage, WM_TRAYICON);
        if owns {
            unsafe {
                let _ = DestroyIcon(hicon);
            }
        }

        // トレイアイコンのライフサイクル検証
        let mut tray = window
            .create_tray_icon("DesCal テスト常駐")
            .expect("トレイアイコン登録失敗");
        assert!(tray.is_registered());
        assert_eq!(tray.uid(), DEFAULT_TRAY_UID);
        assert_eq!(tray.tooltip(), "DesCal テスト常駐");
        assert_eq!(tray.hwnd(), window.hwnd());

        // ツールチップの動的変更 (NIM_MODIFY)
        tray.set_tooltip("DesCal 2026年10月")
            .expect("ツールチップ更新失敗");
        assert_eq!(tray.tooltip(), "DesCal 2026年10月");

        // トレイアイコンの明示的削除 (NIM_DELETE)
        tray.remove().expect("トレイアイコン削除失敗");
        assert!(!tray.is_registered());

        // 2回目の remove 呼び出しも安全に成功すること（冪等性）
        tray.remove().expect("2回目の remove 失敗");
    }
}


