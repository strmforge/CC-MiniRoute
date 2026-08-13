# CC MiniRoute

Windows-focused desktop configuration and local routing manager for Codex,
Claude, Gemini, and related coding agents.

[中文](README_ZH.md) | [日本語](README_JA.md) | [Deutsch](README_DE.md) |
[Changelog](CHANGELOG.md)

## About This Fork

CC MiniRoute is based on CC Switch and keeps compatibility with its provider,
MCP, Skills, session, and local proxy workflows. This fork focuses on:

- CC MiniRoute branding, application identity, icons, repository links, and
  update metadata.
- Windows desktop use and compatibility with existing local configuration
  data.
- Codex provider and model-catalog handling, including native Responses API
  models from non-OpenAI providers.
- Local routing experiments that can be tested without changing the upstream
  project.

This repository does not endorse or advertise API relay vendors. Provider
presets are configuration templates only; users are responsible for checking
service quality, pricing, privacy, and account safety.

## Main Capabilities

- Manage providers for Codex, Claude Code, Claude Desktop, Gemini CLI, OpenCode,
  OpenClaw, Grok Build, and Hermes.
- Switch provider configurations from the desktop interface or system tray.
- Run a local proxy with routing, protocol adaptation, failover, and request
  diagnostics.
- Manage MCP servers, prompts, Skills, usage records, and local sessions.
- Back up configuration before managed changes and restore it when takeover is
  disabled.
- Maintain a combined Codex model catalog for supported native Responses API
  providers.

## Experimental Codex Stable Proxy Entry

The optional stable-entry mode keeps Codex on one local `custom` Responses API
endpoint while CC MiniRoute changes the selected upstream target internally.
This is intended to reduce Codex restarts and avoid rewriting the live Codex
provider configuration during a hot switch.

The setting is disabled by default. Test it on a non-critical machine before
depending on it. Third-party model compatibility, tool calling, context
handling, and billing remain properties of the selected upstream service.

## Build From Source

Requirements:

- Node.js and pnpm
- Rust toolchain
- Windows build tools for MSI/NSIS packaging

```powershell
pnpm install
pnpm typecheck
pnpm tauri build --bundles nsis,msi
```

Windows packages are generated under:

```text
src-tauri/target/release/bundle/
```

## Data And Compatibility

CC MiniRoute currently preserves the existing CC Switch data location for
compatibility. Back up local provider and application configuration before
testing a new build. Do not place API keys in issues, logs, screenshots, or
repository files.

## Upstream And License

CC MiniRoute is derived from
[CC Switch](https://github.com/farion1231/cc-switch) by farion1231 and its
contributors. The project remains available under the MIT License; see
[LICENSE](LICENSE).
