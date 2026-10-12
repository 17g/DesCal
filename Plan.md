# DesCal 実装計画 (Plan.md)

本計画は `for_agent/SPECIFICATIONS.md` および `for_agent/GUARDRAILS.md` に基づき、Windows 11向けの100% Pure Rust製タスクトレイ常駐カレンダーアプリ「DesCal」を段階的に実装・検証するためのタスクリストです。
1つのイテレーション（ループ）につき未完了タスクを1つずつ着実に実装・検証（Exit Code 0）します。

---

## Phase 1: プロジェクト基盤のセットアップ (Project Foundation)
- [x] 1.1 Cargo プロジェクトの初期化 (`Cargo.toml` 作成、Windows API / GUIクレート等の依存関係定義、`cargo check` 疎通確認)
- [x] 1.2 カレンダー計算用コアロジックの実装（日付・月・年・曜日の計算、月間グリッド配列生成、当日判定、および単体テスト）


## Phase 2: ウィンドウシステムと描画基盤の実装 (Window & Rendering System)
- [x] 2.1 ポップアップ用ウィンドウの作成（枠なし・タスクバー非表示・ツールチップスタイル等の属性設定）
- [x] 2.2 マルチモニターおよびDPIスケーリング（Per-Monitor V2）を考慮した画面右下（タスクトレイ近傍）のポップアップ配置座標算出ロジックの実装
- [ ] 2.3 カレンダーの基本UI描画基盤の実装（ヘッダー部、曜日行、日付グリッドセル、当日ハイライト表示）

## Phase 3: カレンダー操作と状態管理の実装 (Interactions & State Management)
- [ ] 3.1 前月・翌月切り替えボタンのクリックイベントおよび「今日」へジャンプするナビゲーション機能の実装
- [ ] 3.2 マウスホイールスクロールによるスムーズな月送り・月戻し機能の実装

## Phase 4: タスクトレイ常駐とポップアップ制御の実装 (Tray & Focus Control)
- [ ] 4.1 システムトレイアイコンの常駐と右クリックメニュー（「終了」等）の実装
- [ ] 4.2 トレイアイコン左クリックによるカレンダーポップアップの表示/非表示（トグル）制御の実装
- [ ] 4.3 ポップアップ外クリック時（フォーカス喪失 `WM_KILLFOCUS` / Deactivate）の自動非表示ロジックの実装

## Phase 5: UIブラッシュアップ・品質検証と統合テスト (Refinement & Acceptance Verification)
- [ ] 5.1 Windows 11調のモダンUIスタイル調整（角丸、配色、ホバー効果、フォント設定）
- [ ] 5.2 全体ビルド（`cargo check` / `cargo test` / `cargo build`）および完成基準（Acceptance Criteria）の網羅的検証
