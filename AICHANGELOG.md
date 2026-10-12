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
