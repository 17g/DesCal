# ガードレール（作業手順・注意点・再発防止策）

本ドキュメントは、DesCalプロジェクトにおける実装・修正作業時のミスやトラブルを未然に防ぎ、品質と一貫性を保つための手順および注意事項をまとめたものです。問題の発見や指示があった場合は、随時更新を行います。

---

## 1. 技術スタック・アーキテクチャ上の注意点
- **100% Pure Rust の遵守**:
  - Webviewや他言語ランタイム（Python, Node.js 等）を抱え込まず、ネイティブRustのGUI・トレイ制御クレートを用いて構築すること。
  - Windows 11専用の軽快な動作と省メモリ性を最優先とする。
- **Windows API / タスクトレイ・ウィンドウ制御**:
  - タスクトレイアイコンの右クリックメニューおよび左クリックでのポップアップトグル制御の競合に注意すること。
  - ポップアップ外クリック時のフォーカス喪失検知（`WM_KILLFOCUS` やウィンドウのDeactivateイベント等）が確実に行われるように実装・テストすること。
  - ディスプレイのスケーリング（DPIスケーリング）やマルチモニター環境でのタスクトレイ座標取得・ウィンドウ配置のズレに配慮すること。

---

## 2. 実装および検証手順
- **コード変更後の検証**:
  - コード修正後は、必ず `cargo check` または `cargo test` / `cargo build` を実行し、コンパイルエラーや警告がないことを確認すること。
- **文字コード・ファイル操作**:
  - ファイルの読み書きは常に UTF-8（BOMなし）で行うこと。
- **段階的な検証**:
  - トレイ常駐とイベント検知 -> カレンダーウィンドウ描画 -> ポップアップ・フォーカス制御 -> 月送りロジック の順に小さく検証を重ねること。

---

## 3. トラブルシューティング・改訂履歴
- **[2026-10-12] Windows API (windowsクレート 0.62) におけるエラーコードとウィンドウ生成**:
  - `Error::from_win32()` は存在せず、直前のWin32エラー取得には `windows::core::Error::from_thread()` を使用する。
  - Win32のエラーHRESULT比較（例: `ERROR_CLASS_ALREADY_EXISTS = 0x80070582`）はRustの `i32` 境界外のため、`0x80070582u32 as i32` のように明示的キャストを行うこと。
  - タスクバー非表示および枠なしポップアップを実現するには、`WS_POPUP` および拡張スタイル `WS_EX_TOOLWINDOW | WS_EX_TOPMOST` を設定し、Windows 11のモダンな角丸には `DwmSetWindowAttribute(DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND)` を併用すること。
- **[2026-10-12] マルチモニター・DPIスケーリング（Per-Monitor V2）計算時の注意点**:
  - `GetCursorPos` などの一部 Win32 API は `BOOL` ではなく `windows::core::Result<()>` を返すシグネチャになっているため、`.as_bool()` ではなく `?` 演算子で伝播する。
  - プライマリモニターの左側や上側に配置されたセカンダリモニターでは、作業領域（`rcWork`）の座標が負の値になる。座標計算およびクランプ処理（`min_x = work.left`, `max_x = work.right - width`）では必ず負の座標を考慮し、`0` に決め打ちしないこと。
  - DPI スケーリング計算では `(value * dpi + 48) / 96` を用いることで浮動小数点誤差を排し、高速かつ決定論的なスケーリングを実現する。
- **[2026-10-12] GDIダブルバッファリング描画とウィンドウ状態管理（GWLP_USERDATA）**:
  - `RECT` 構造体は `PartialEq` のみ実装されており `Eq` を実装していないため、これを含む構造体の derive マクロには `Eq` を含めず `PartialEq` のみとする。
  - ウィンドウ再描画時のチラつき（フリッカー）を完全に防止するため、`WM_ERASEBKGND` メッセージで `LRESULT(1)` を返してシステム標準の背景消去を抑制し、`WM_PAINT` 内部で互換DC（`CreateCompatibleDC`）と互換ビットマップ（`CreateCompatibleBitmap`）によるダブルバッファリングを行い、`BitBlt` で一括転送する。
  - GDIオブジェクト（`HBRUSH`, `HPEN`, `HFONT`, `HBITMAP`, メモリ `HDC`）のリークを完全に防止するため、RAIIラッパー（`Drop` 時に `DeleteObject(obj.into())` や `DeleteDC` を呼ぶ構造体）を用い、また `SelectObject` は選択前の旧オブジェクトを保持してスコープ終了時に確実に復元する。
  - `wnd_proc` からウィンドウの内部状態（`CalendarModel`, `CalendarTheme`, DPI等）にアクセスするため `SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr)` を用い、ウィンドウ破棄時（`Drop`）には `SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0)` で登録を解除することで、不正メモリアクセスを防止する。
- **[2026-10-13] マウスイベント制御と windows 0.62 入力 API**:
  - `TrackMouseEvent` / `TRACKMOUSEEVENT` / `TME_LEAVE` は `windows::Win32::UI::Input::KeyboardAndMouse` モジュール（Cargo feature: `Win32_UI_Input_KeyboardAndMouse`）に所属している。
  - `WM_MOUSELEAVE` は定数 `0x02A3` を使用する。
  - `SendMessageW` の `wparam` / `lparam` は windows 0.62 では `Option<WPARAM>` / `Option<LPARAM>` を要求するため、`Some(WPARAM(...))` / `Some(LPARAM(...))` でラップすること。
  - マウスホバー・押下状態（`NavButton`）に応じたUIフィードバック（テーマ色切り替え）と、`WM_SETCURSOR` での `IDC_HAND` カーソル設定を併用することで、ボタンの視認性とクリック操作の直感性を高める。
  - `WM_MOUSEWHEEL` メッセージにおけるホイール回転量（`zDelta`）は `((wparam.0 >> 16) & 0xffff) as i16` で上位16ビットを符号付き整数として抽出する。高精度タッチパッド等の端数回転量（`< 120`）に対応するため、`wheel_delta_accum` に蓄積して `WHEEL_DELTA (120)` 単位で月送りを行い、逆方向へのスクロール転換時には蓄積値を `0` にリセットすることでスムーズさと即応性を両立させる。
- **[2026-10-13] システムトレイ常駐（`Shell_NotifyIconW`）と右クリックコンテキストメニュー制御**:
  - トレイアイコンの右クリックメニュー表示（`TrackPopupMenu`）では、MSDN KB135788 の定石に従い、呼び出し直前に `SetForegroundWindow(hwnd)`、呼び出し直後に `PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0))` を実行すること。これを省略すると、メニュー外をクリックしてもコンテキストメニューが閉じない不具合が発生する。
  - `TrackPopupMenu` のフラグに `TPM_RETURNCMD | TPM_NONOTIFY` を指定して選択されたコマンドIDを直接受け取る方式と、`WM_COMMAND` メッセージ経由で処理する方式の両方をサポートすることで、実機操作と自動テスト（`SendMessageW(hwnd, WM_COMMAND, ...)`）の双方で一貫した動作検証が可能になる。
  - サンドボックスや非対話型テストランナー環境では、Explorer（`Shell_TrayWnd`）へのIPCがUIPI（User Interface Privilege Isolation）によって制限され、`Shell_NotifyIconW(NIM_ADD, ...)` が `FALSE`（`HRESULT(0x80004005)`）を返す場合がある。そのため、`IsWindow` でウィンドウハンドルの有効性を検証した上で、`shell_bound` フラグによって実シェルバインド状態とインメモリ常駐状態を適切に管理すること。

