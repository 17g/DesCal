# AI 変更履歴 (AICHANGELOG.md)

## [2026-10-12] Phase 1.1: Cargo プロジェクトの初期化
- **変更内容**:
  - `Cargo.toml` の新規作成および依存関係（`chrono = "0.4"`, `windows = "0.62"`）の定義
  - `.gitignore` の新規作成（`/target` ディレクトリの除外）
  - `src/main.rs` の初期エントリーポイント作成
- **変更理由**:
  - Windows 11専用の100% Pure Rust製タスクトレイ常駐カレンダーアプリ「DesCal」の開発基盤を構築し、Windows APIおよび日付計算ライブラリの利用準備を整えるため。
- **影響範囲**:
  - プロジェクト全体のビルド基盤、依存関係定義。
- **検証結果**:
  - `cargo check`: Exit Code 0 (成功)
  - `cargo test`: Exit Code 0 (成功)

## [2026-10-12] Phase 1.2: カレンダー計算用コアロジックの実装
- **Before**:
  - `src/main.rs` のみの最小スケルトン構成であり、日付計算やカレンダーグリッド生成のロジックが存在しなかった。
- **After**:
  - `src/calendar.rs` を新規作成し、カレンダー用データ構造（`CalendarCell`, `CalendarModel`）および各機能（日曜始まり6行×7列のグリッド生成、前月・翌月移動、当日ハイライト判定、今日へ戻るナビゲーション）を実装。
  - `src/main.rs` に `pub mod calendar;` を追加。
  - グリッド配列の連続性・前月翌月の境界・当日判定・うるう年を検証する5件の単体テストを追加。
- **変更理由**:
  - カレンダーUI表示および月送りナビゲーションを支える純粋なドメイン計算モデルを確立し、Windows UI描画処理からビジネスロジックを分離・独立してテスト可能にするため。
- **影響範囲**:
  - カレンダーの日付計算ロジック（`src/calendar.rs`）、エントリーポイント（`src/main.rs`）。
- **検証結果**:
  - `cargo check`: Exit Code 0 (成功)
  - `cargo test`: 5 passed, 0 failed, Exit Code 0 (成功)
## [2026-10-12] Phase 2.1: ポップアップ用ウィンドウの作成（枠なし・タスクバー非表示・ツールチップスタイル等の属性設定）
- **Before**:
  - ウィンドウの生成や管理モジュールが存在せず、`src/calendar.rs` の日付計算ロジックのみが存在していた。
- **After**:
  - `src/window.rs` を新規作成し、カレンダー用ポップアップウィンドウ構造体 `CalendarWindow` および設定構造体 `PopupWindowConfig` を実装。
  - ウィンドウスタイルとして枠なし・タイトルバーなしの `WS_POPUP` を適用。
  - 拡張スタイルとしてタスクバー非表示の `WS_EX_TOOLWINDOW`、最前面表示の `WS_EX_TOPMOST` を設定。
  - Windows 11 の角丸設定 `DwmSetWindowAttribute(DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND)` を組み込み。
  - ウィンドウの表示（`show`）・非表示（`hide`）・位置設定（`set_position`）・属性取得・`Drop` 時の破棄処理を実装。
  - ポップアップウィンドウの作成、属性値の検証、表示/非表示切り替えを網羅する単体テスト `test_create_popup_window_attributes` を追加。
  - `for_agent/GUARDRAILS.md` に windows 0.62 におけるエラーコード扱いおよびポップアップ属性の知見を記録。
- **変更理由**:
  - タスクトレイと連動する軽量かつモダンなWindows 11調カレンダーポップアップの土台となるウィンドウ基盤を構築するため。
- **影響範囲**:
  - ウィンドウ管理モジュール（`src/window.rs`）、エントリーポイント（`src/main.rs`）、ガードレール（`for_agent/GUARDRAILS.md`）。
- **検証結果**:
  - `cargo check`: Exit Code 0 (成功)
  - `cargo test`: 6 passed, 0 failed, Exit Code 0 (成功)

## [2026-10-12] Phase 2.2: マルチモニターおよびDPIスケーリング（Per-Monitor V2）を考慮した画面右下ポップアップ配置座標算出ロジックの実装
- **Before**:
  - ウィンドウは固定の座標にしか配置できず、マルチモニター環境やDPIスケーリング（125%、150%、200%等）、タスクバー位置（作業領域 `rcWork`）を考慮した自動配置ロジックが存在しなかった。
- **After**:
  - `src/position.rs` を新規作成し、以下の機能を実装：
    - プロセス全体の `Per-Monitor V2` DPI awareness 有効化（`enable_per_monitor_dpi_v2`）
    - モニター/ウィンドウ/システムごとの実効DPI取得およびDPIスケーリング計算関数（`scale_value`）
    - 純粋データモデル（`DisplayRect`, `MonitorBounds`, `TaskbarEdge`, `PopupGeometry`）
    - 作業領域とモニター領域の差分に基づくタスクバー位置検出およびトレイ近傍（画面右下）ポップアップ配置・クランプ計算（`calculate_popup_geometry`）
    - カーソル位置やターゲット座標に基づく実モニター取得と配置座標自動算出（`calculate_tray_popup_geometry`）
  - `src/window.rs` の `CalendarWindow` に `set_geometry` および `reposition_to_tray` メソッドを追加。
    - 多様なDPI（100%, 125%, 150%, 200%）、マルチモニター負座標（セカンダリモニター）、タスクバー位置別、画面端クランプ処理、トレイアイコン連動、実ウィンドウ再配置を網羅する9件の単体テストを追加（合計15件すべてパス）。
  - `for_agent/GUARDRAILS.md` に DPIスケーリング・マルチモニター負座標・Win32 APIのResult型扱いの注意点を追記。
- **変更理由**:
  - 複数モニター環境や異なるディスプレイ倍率環境でも、常にタスクトレイ近傍（作業領域の右下）に適切なサイズと位置でカレンダーがポップアップ表示されるようにするため。
- **影響範囲**:
  - 座標・DPI計算モジュール（`src/position.rs`）、ウィンドウ管理（`src/window.rs`）、エントリーポイント（`src/main.rs`）、ガードレール（`for_agent/GUARDRAILS.md`）。
- **検証結果**:
  - `cargo check`: Exit Code 0 (成功)
  - `cargo test`: 15 passed, 0 failed, Exit Code 0 (成功)

## [2026-10-13] Phase 2.3: カレンダーの基本UI描画基盤の実装（ヘッダー部、曜日行、日付グリッドセル、当日ハイライト表示）
- **Before**:
  - `CalendarWindow` は枠なし・タスクバー非表示のポップアップウィンドウを生成・配置できるのみで、カレンダー描画処理（`WM_PAINT` / GDI）やカレンダーデータ・UI状態の保持・表示基盤が存在しなかった。
- **After**:
  - `src/render.rs` を新規作成し、以下のUI描画システムを実装：
    - Windows 11調のモダンUIカラーテーマ（`CalendarTheme`）およびRGB変換ヘルパー（`rgb`）
    - DPIスケーリングを考慮したUIレイアウト幾何計算（`CalendarLayout`: クライアント領域、ヘッダー部、年月タイトル、ナビゲーションボタン、曜日行、42マスグリッドセル）およびヒットテスト関数
    - GDIオブジェクトの安全な自動破棄（RAIIパターン: `AutoDeleteGdi`, `AutoSelectObject`, `AutoDeleteDc`）によるリーク防止
    - 高品位なDPI対応フォント作成ヘルパー（`create_gdi_font`: Segoe UI / ClearType）
    - チラつきを防止するダブルバッファリング描画エンジン（`CalendarRenderer::paint`）
      - 背景塗りつぶし
      - ヘッダー描画（年月タイトル、前月/今日/翌月ボタン）
      - 曜日行描画（日, 月, 火, 水, 木, 金, 土、日祝ソフトレッド・土曜ソフトブルー）
      - 42マス日付セル描画（当月白、前月翌月グレー、日曜・土曜カラー、当日ハイライトの角丸ピル背景＆白太字）
      - `BitBlt` による高速フロントバッファ転送
  - `src/window.rs` の `CalendarWindow` を拡張：
    - ウィンドウ状態構造体 `WindowState`（`CalendarModel`, `CalendarTheme`, `dpi`）を保持し、`GWLP_USERDATA` を通じて `wnd_proc` と連携
    - `WM_PAINT` でのダブルバッファリング描画実行、`WM_ERASEBKGND` でのチラつき抑制
    - カレンダーモデルやテーマへの参照・可変参照取得、`refresh()`、`prev_month()`、`next_month()`、`go_to_today()`、`set_date()`、`layout()` などの操作APIを追加
    - ウィンドウ破棄時に `GWLP_USERDATA` を安全にリセットするクリーンアップ
  - 単体テスト・統合テストを追加（レイアウト計算、ヒットテスト、実機DCによる描画パイプライン検証、ウィンドウ状態操作、ライフサイクル等、全21テスト成功）
  - `for_agent/GUARDRAILS.md` に GDIダブルバッファリング、RAII管理、`RECT` の `Eq` 未実装、`GWLP_USERDATA` 管理に関するガイドラインを追記。
- **変更理由**:
  - カレンダー基本UI描画基盤（ヘッダー部、曜日行、日付グリッドセル、当日ハイライト表示）をWindows 11調のモダンなデザインで美しく・高速・チラつきなく実現するため。
- **影響範囲**:
  - カレンダー描画エンジン（`src/render.rs`）、ウィンドウ管理（`src/window.rs`）、エントリーポイント（`src/main.rs`）、ガードレール（`for_agent/GUARDRAILS.md`）。
- **検証結果**:
  - `cargo check`: Exit Code 0 (成功)
  - `cargo test`: 21 passed, 0 failed, Exit Code 0 (成功)


