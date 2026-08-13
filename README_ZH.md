# CC MiniRoute

面向 Windows 的桌面配置与本地路由管理工具，服务于 Codex、Claude、Gemini
及相关编程智能体。

[English](README.md) | [日本語](README_JA.md) | [Deutsch](README_DE.md) |
[更新日志](CHANGELOG.md)

## 项目定位

CC MiniRoute 基于 CC Switch，保留其供应商、MCP、Skills、会话和本地代理等
工作流的兼容性。本分支主要关注：

- CC MiniRoute 的名称、应用标识、图标、仓库链接与更新元数据。
- Windows 桌面环境以及现有本地配置数据的兼容性。
- Codex 供应商与模型目录，包括非 OpenAI 供应商原生 Responses API 模型。
- 与上游项目隔离、可独立验证的本地路由实验。

本仓库不为任何 API 中转服务商背书或投放广告。内置供应商预设仅是配置
模板，服务质量、价格、隐私与账号安全需要用户自行核实。

## 主要能力

- 管理 Codex、Claude Code、Claude Desktop、Gemini CLI、OpenCode、
  OpenClaw、Grok Build 和 Hermes 的供应商配置。
- 从桌面界面或系统托盘切换供应商。
- 提供带路由、协议适配、故障转移和请求诊断的本地代理。
- 管理 MCP 服务器、提示词、Skills、用量记录与本地会话。
- 在接管配置前创建备份，并在关闭接管时恢复原始配置。
- 为支持原生 Responses API 的供应商维护 Codex 联合模型目录。

## 试验性 Codex 固定代理入口

可选的固定入口模式让 Codex 始终连接同一个本地 `custom` Responses API
入口，由 CC MiniRoute 在内部切换实际上游。其目标是减少 Codex 重启，并
避免热切换时反复重写 Codex 的活动供应商配置。

该设置默认关闭。请先在非关键机器上测试。第三方模型的兼容性、工具调用、
上下文处理和计费方式仍由实际上游服务决定。

## 从源码构建

环境要求：

- Node.js 与 pnpm
- Rust 工具链
- 用于生成 MSI/NSIS 的 Windows 构建工具

```powershell
pnpm install
pnpm typecheck
pnpm tauri build --bundles nsis,msi
```

Windows 安装包生成在：

```text
src-tauri/target/release/bundle/
```

## 数据与兼容性

为兼容既有安装，CC MiniRoute 当前继续使用原 CC Switch 数据目录。测试新
版本前请备份本地供应商和应用配置。不要把 API Key 写入 Issue、日志、截图
或仓库文件。

## 上游与许可证

CC MiniRoute 基于 farion1231 及其贡献者维护的
[CC Switch](https://github.com/farion1231/cc-switch) 开发。本项目继续使用
MIT License，详见 [LICENSE](LICENSE)。
