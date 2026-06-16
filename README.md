# CodeSail

CodeSail 是一个用于管理 Codex CLI 配置的跨平台桌面应用。它把 provider、model、token、本地配置文件和历史会话集中到一个清晰的图形界面里，适合经常切换模型和供应商的 Codex 用户。

> 当前项目处于早期版本，主要面向本地配置管理。欢迎在开源后继续完善安装包、自动更新和更多平台适配。

## 功能特性

- 概览 Codex、Node.js、npm 的可用状态和版本
- 查看当前 app 版本、当前设置模型和历史记录摘要
- 管理 Codex CLI 的 `model_providers`
- 新增、编辑、复制、删除 provider 配置
- 拉取 provider 的 `/models` 列表并保存模型
- 一键设置当前 provider 和 model
- 可选择是否同步写入 Codex 配置文件
- 保存 token，并在切换当前模型时同步 `auth.json`
- 查看、恢复、删除本地 Codex 历史会话
- 支持浅色、深色、跟随系统主题
- 支持在终端中打开或重启 Codex

## 截图

截图待补充。

建议放置到 `docs/images/` 后在这里引用，例如：

```md
![CodeSail overview](docs/images/overview.png)
```

## 技术栈

- Tauri 2
- React 18
- Vite
- TypeScript
- Rust
- SQLite
- `toml_edit`

## 工作方式

CodeSail 会读取并维护 Codex CLI 使用的本地配置：

- 默认配置文件：`~/.codex/config.toml`
- 默认认证文件：`~/.codex/auth.json`
- 支持 `CODEX_CONFIG` 指定自定义 `config.toml`
- 本地 provider 数据：`~/.codex/codex-config-desktop.sqlite3`
- 本地 token key：`~/.codex/codex-config-desktop.key`

数据库文件名目前保留旧项目名，是为了兼容已经安装使用过的本地数据，避免项目改名后丢失已有 provider 和 token。

## 开发环境

需要先安装：

- Node.js
- npm
- Rust 和 Cargo
- macOS: Xcode Command Line Tools
- Windows: Tauri 2 所需的 WebView2 和 Visual Studio C++ Build Tools

如果刚安装 Rust，当前终端可能需要重新加载 PATH：

```bash
. "$HOME/.cargo/env"
```

## 本地开发

安装依赖：

```bash
npm install
```

启动桌面开发环境：

```bash
npm run tauri:dev
```

只启动前端开发服务：

```bash
npm run dev
```

默认前端端口是 `1420`。

## 构建

校验前端构建：

```bash
npm run build
```

校验 Rust 后端：

```bash
cd src-tauri
cargo check
```

打包桌面应用：

```bash
npm run tauri:build
```

构建产物会由 Tauri 输出到 `src-tauri/target/release/bundle/`。

## 项目结构

```text
.
├── docs/                 # 设计和执行计划文档
├── src/                  # React 前端
│   ├── lib/              # Tauri API 封装和类型
│   ├── App.tsx           # 主界面
│   └── styles.css        # 界面样式
├── src-tauri/            # Tauri/Rust 后端
│   ├── src/config.rs     # Codex 配置、provider、token、运行环境检测
│   ├── src/history.rs    # Codex 历史会话读取和管理
│   └── tauri.conf.json   # Tauri 应用配置
├── package.json
└── README.md
```

## 安全说明

- CodeSail 不上传 provider 配置或 token
- token 保存在本机 SQLite 数据库中，并使用本地 key 加密
- `auth.json` 只同步当前 provider 所需的 token
- 写入 `config.toml` 前会创建备份文件

请仍然把 `~/.codex` 视为敏感目录，不要把其中的数据库、key、`auth.json` 或包含 token 的配置提交到 GitHub。

## 路线图

- 增加正式安装包和发布流程
- 增加 GitHub Releases 版本检查
- 增加应用内更新提示
- 补充截图和使用文档
- 补充自动化测试
- 优化 Windows/Linux 平台体验

## 贡献

欢迎提交 issue 或 pull request。建议在提交前至少运行：

```bash
npm run build
cd src-tauri
cargo check
```

如果改动涉及 UI，请附上截图或录屏，方便确认布局和交互效果。

## 许可证

待补充。开源前建议添加一个明确的 `LICENSE` 文件，例如 MIT、Apache-2.0 或其他你希望采用的许可证。

## 文档

- [执行计划](docs/EXECUTION_PLAN.md)
