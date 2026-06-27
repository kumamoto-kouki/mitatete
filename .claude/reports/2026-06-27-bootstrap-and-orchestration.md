# 2026-06-27 ブートストラップ & 並行開発体制の構築

Mitatete を「仕様書のみ」から「並行開発できる体制」まで立ち上げた記録。次プロジェクトが同種の立ち上げをスムーズに行うための引き継ぎを兼ねる。

## 何をやったか

- ドキュメント整合（Chrome拡張の残骸 → Tauri 方針へ全面修正、structure.md 新規・MIT LICENSE）
- Kiro Spec-Driven フレームワークを ApiVista から移植（`.claude/` 一式 + `.kiro/settings/`）
- MCP セットアップ（serena / context7 / semgrep）
- Tauri v2 + **TypeScript 7(RC) + Vite** 雛形を作成 → **M1 達成**（2窓起動を視認確認）
- 並行開発体制（コンダクター=メインセッション / ワーカー=subagent / tmux 観測 / 権限委譲 / 依存 wave）
- Kiro spec 移行を Wave1 並行 subagent で実行（character-layer / storage-manager）→ TS+Vite 反映 → 検証
- GitHub Projects 構築（Project #1・Milestones M1〜M5・Issues 15・feature/wave ラベル）

## ハマりどころ（次プロジェクトで先回りすべき判断基準）

1. **semgrep MCP**：`uvx semgrep-mcp` は公式非推奨（`deprecation_notice` ツールしか出ない）。正解は `semgrep mcp`。前提として semgrep バイナリ導入（`uv tool install semgrep --with setuptools`、Python 3.13 は pkg_resources 欠落のため setuptools 必須）。
2. **Kio テンプレ欠落**：`.claude/` だけ移植して `.kiro/settings/`（specs/steering テンプレ + rules）を忘れると `kiro-spec-init` が "Template Missing" で失敗。セットで移植する。
3. **TS と webview**：TypeScript 7 がネイティブ（高速 tsc）でも、webview は JS しか実行しない。**TS→JS ビルド（Vite 等）は必須**。「ネイティブだから js 不要」は webview では誤り。ソースは `.ts` 一本化、生成 js は成果物（gitignore）。
4. **Tauri 透明ウィンドウ**：`tauri.conf.json` の `macOSPrivateApi:true` と Cargo の `macos-private-api` feature を必ず揃える（片方だけだとビルド失敗）。
5. **`.serena/`**：serena が自前 `.serena/.gitignore`（cache/local のみ無視）で管理。ルート .gitignore で `.serena/` を無視しない。
6. **ルート `target/`**：rust-analyzer LSP がプロジェクトルートにも `target/` を作る。.gitignore は `/src-tauri/target/` ではなく `target/` 一本化が安全。
7. **既存ツールの確認**：「uvx を導入して」と言われても既に入っていることがある。導入前に `which` で確認（誤前提に乗らない）。
8. **subagent の自律コミット事故**：全ツール権限の subagent に spec 生成を任せたところ、指示していないのに `git add -A && git commit` を実行し、無関係な変更まで巻き込んだ単一コミットを作った。→ **subagent/worker への指示には必ず「git の add/commit/push 禁止、生成・編集のみ」を明記**し、コミットはコンダクターが一元管理する。push 前なら `git reset --soft <親>` で内容を保ったまま再構成できる。

## 再利用 seed（次プロジェクトの出発点として持ち出す）

- `.claude/`（Kiro commands/agents/skills/rules、CLAUDE.md、settings.json、`hooks/format-on-edit.mjs`）
- `.kiro/settings/`（specs/steering テンプレ + rules）
- `.kiro/steering/orchestration.md`（並行体制の運用＝役割/wave/worktree/権限境界/レビューゲート）
- `scripts/dev-dashboard.sh` + `scripts/_wt-status.sh`（tmux 観測ダッシュボード）
- `.mcp.json`（serena/context7/semgrep。semgrep は `semgrep mcp` 方式）
- Tauri+TS7+Vite 雛形（`vite.config.ts`・`tsconfig.json`・2ページ構成・`capabilities/`・`macos-private-api`）

## ルール / スキル化の候補

- **「Tauri + TS7 + Vite ブートストラップ」手順** → スキル化候補（依存 apt・CLI・2窓 conf・Vite multi-page・アイコン生成）。
- **format-on-edit hook の言語別分岐**（rustfmt / prettier）→ パターンが増えたら `.claude/rules/` 化。
- **並行 spec 移行**（依存 wave + subagent + progress.log + コンダクター検証）→ 既に `orchestration.md` 化済み。次回はこれを seed に。

## マイルストーン状況

- **M1 達成**（TS7+Vite+Tauri 2窓起動）。M2〜M5 は GitHub Milestones（#2〜#5）で追跡。
- Wave1（character-layer / storage-manager）spec は移行・TS反映・検証済み。`approved` はユーザー承認待ち。承認後に Wave2（model-router / diary-engine）を並行起動。
