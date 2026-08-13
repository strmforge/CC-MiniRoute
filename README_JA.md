# CC MiniRoute

Codex、Claude、Gemini、および関連するコーディングエージェント向けの、
Windows を中心としたデスクトップ設定・ローカルルーティング管理ツールです。

[English](README.md) | [中文](README_ZH.md) | [Deutsch](README_DE.md) |
[Changelog](CHANGELOG.md)

## このフォークについて

CC MiniRoute は CC Switch を基盤とし、プロバイダー、MCP、Skills、セッション、
ローカルプロキシのワークフローとの互換性を維持します。このフォークでは、
主に次の領域を扱います。

- CC MiniRoute の名称、アプリ識別子、アイコン、リポジトリリンク、更新情報。
- Windows デスクトップと既存のローカル設定データとの互換性。
- OpenAI 以外のネイティブ Responses API モデルを含む Codex の
  プロバイダーおよびモデルカタログ管理。
- 上流プロジェクトに影響を与えず検証できるローカルルーティング実験。

このリポジトリは API 中継業者の広告や推奨を行いません。プロバイダー
プリセットは設定テンプレートにすぎません。品質、料金、プライバシー、
アカウントの安全性は利用者自身で確認してください。

## 主な機能

- Codex、Claude Code、Claude Desktop、Gemini CLI、OpenCode、OpenClaw、
  Grok Build、Hermes のプロバイダー設定管理。
- デスクトップ画面またはシステムトレイからの設定切り替え。
- ルーティング、プロトコル変換、フェイルオーバー、診断を備えたローカル
  プロキシ。
- MCP サーバー、プロンプト、Skills、使用量、ローカルセッションの管理。
- 管理対象の設定を変更する前のバックアップと、管理解除時の復元。
- ネイティブ Responses API プロバイダー向け Codex 統合モデルカタログ。

## 実験的な Codex 固定プロキシ入口

任意の固定入口モードでは、Codex は単一のローカル `custom` Responses API
エンドポイントへ接続し続け、CC MiniRoute が内部で上流を切り替えます。
Codex の再起動や、切り替えごとの設定ファイル書き換えを減らすための機能です。

この設定は既定で無効です。重要でない環境で先に検証してください。第三者
モデルの互換性、ツール呼び出し、コンテキスト処理、課金は上流サービスに
依存します。

## ソースからのビルド

```powershell
pnpm install
pnpm typecheck
pnpm tauri build --bundles nsis,msi
```

Windows パッケージの出力先：

```text
src-tauri/target/release/bundle/
```

## 上流とライセンス

CC MiniRoute は farion1231 とコントリビューターによる
[CC Switch](https://github.com/farion1231/cc-switch) を基にしています。
ライセンスは MIT License です。詳細は [LICENSE](LICENSE) を参照してください。
