# AGENTS.md

## 1. プロジェクト概要 & 技術スタック
- Framework: Pure Rust (GUI / System Tray)
- Language: Rust
- Package Manager: cargo

## 2. 検証コマンド (Verification Ground Truth)
エージェントはコード修正後、以下のコマンドを実行して正常終了（Exit Code 0）を確認すること。
- 構文・型チェック: `cargo check`
- 単体テスト: `cargo test` （※テストコードが存在する場合のみ実行）

## 3. 参照ディレクトリ
- 仕様書正本: `for_agent/` 配下のすべての .md ファイル
- 進捗管理: `Plan.md`
- 変更ログ: `AICHANGELOG.md`
