# CodeSail 执行计划

## 目标

把 `codex-config-script/switch-codex-config.sh` 的能力重构为跨平台桌面 App，支持 macOS 和 Windows 用户直接管理 Codex CLI 配置，不再依赖 Git Bash、WSL、openssl、awk、sed、perl 等命令行环境。

## 产品范围

第一版只做本地配置管理，不做云同步，不上传 token，不托管用户配置。

必须支持：

- 读取默认配置文件：`~/.codex/config.toml` 或 `%USERPROFILE%\.codex\config.toml`
- 支持 `CODEX_CONFIG` 指定自定义配置路径
- 查看当前 `model_provider` 和 `model`
- 查看、添加、编辑、删除 `model_providers`
- 为 provider 保存 token
- 拉取 provider 的模型列表
- 设置当前 provider 和 model
- 同步写入 `auth.json` 的 `OPENAI_API_KEY`
- 写入前自动备份配置

暂不支持：

- 多设备同步
- 远程配置仓库
- 内置 Codex 启停控制
- 管理非 Codex 应用的密钥
- 旧版 `provider-keys.json` 自动迁移

## 技术方案

采用 Tauri 2 + React + Rust。

选择原因：

- Tauri 打包体积小，适合本地工具
- Rust 适合处理 SQLite、TOML、JSON 和文件权限
- Windows/macOS 原生集成能力比 shell 脚本稳定
- 前端可以快速做出清晰的 provider 管理界面

核心模块：

- `config`: 读取和写入 `config.toml`
- `providers`: provider 增删改查
- `storage`: SQLite 保存 provider、token 和当前选择
- `models`: 请求 `{base_url}/models`
- `backup`: 写入前创建 `.bak.YYYYMMDD`
- `ui`: provider 列表、详情、当前模型、token 状态

## UI 方向

用户是经常切换模型和 provider 的 Codex 使用者，界面应像本地配置控制台，而不是营销页。

设计约束：

- 左侧 provider 列表，右侧详情和当前模型
- 不做大 hero，不做装饰卡片堆叠
- 操作用图标按钮，复杂动作配短文字
- provider、base URL、model 保持高密度可扫描
- 颜色偏系统工具感：浅灰底、深墨文字、橙色强调当前选择

当前视觉 token：

- Surface: `#f8fafb`
- Workspace: `#eef1f4`
- Ink: `#1d2430`
- Muted: `#66717e`
- Accent: `#f07f4f`
- Success: `#2d9f78`

## 里程碑

### M1: 项目骨架

状态：已开始。

交付内容：

- Tauri + React + TypeScript 项目结构
- Rust `get_app_state` 命令
- 前端读取并展示当前配置状态
- 初版 README 和执行计划

验收标准：

- `npm install` 后可以运行 `npm run tauri:dev`
- App 能显示配置路径、当前 provider、当前 model、provider 列表

### M2: 配置读写

交付内容：

- 添加 provider
- 编辑 provider
- 删除 provider
- 保留 TOML 注释和未知字段
- 写入前创建按天备份

验收标准：

- 对现有 `config.toml` 执行增删改后，Codex CLI 仍可读取
- 删除当前 provider 时提示用户选择替代 provider

### M3: Token 管理

交付内容：

- SQLite 保存 provider token
- SQLite 记录当前 provider 和 model
- 当前 provider 切换时写入 `auth.json`

验收标准：

- 不再使用 `provider-keys.json`
- provider token 从 SQLite 读取
- `auth.json` 只保存当前 provider 的 token

### M4: 模型列表和当前模型切换

交付内容：

- 根据 provider 的 `base_url` 请求 `/models`
- 展示模型列表
- 支持搜索模型
- 请求失败时支持手动输入 model id
- 保存 `model_provider` 和 `model`

验收标准：

- 支持 OpenAI 兼容接口
- HTTP 错误、网络错误、JSON 格式错误都有明确提示
- 切换完成后提示需要重启 Codex

### M5: 跨平台打包

交付内容：

- macOS `.dmg` 或 `.app`
- Windows `.msi` 或 `.exe`
- GitHub Actions 构建流程
- 发布说明和安装说明

验收标准：

- macOS 和 Windows 都能安装启动
- 默认配置路径在两端正确解析
- 打包产物不包含用户 token、auth 文件或本地备份

## 风险和处理

### TOML 格式保留

风险：用户的 `config.toml` 可能包含注释和未知字段。

处理：用 `toml_edit` 修改指定字段，尽量保留原文件结构，不做全量重排。

### SQLite 可用性

风险：直接使用系统 SQLite C API，Windows 打包时需要确认运行环境提供 SQLite。

处理：macOS 使用系统 SQLite；Windows 打包阶段确认链接方式，必要时改为随包分发 SQLite 动态库。

### Windows 文件权限

风险：`chmod 600` 在 Windows 不适用。

处理：SQLite 数据库和 `auth.json` 在 Windows 上依赖系统默认用户目录权限，打包前再补充平台专项检查。

### 模型接口差异

风险：不同 provider 的 `/models` 响应可能不完全一致。

处理：先兼容 OpenAI 标准 `data[].id`，再保留手动输入入口。

## 开发顺序

1. 完成 `get_app_state` 并跑通桌面窗口
2. 实现 provider 增删改查
3. 实现备份和 TOML 写入测试
4. 实现 SQLite token 保存
5. 实现 Codex 配置文件同步
6. 实现模型列表请求和切换
7. 做 macOS/Windows 打包
8. 补充端到端 smoke test

## 测试计划

- Rust 单元测试：TOML 读写、备份路径、provider 解析
- 前端组件测试：空状态、provider 选中、当前模型展示
- 手动 smoke test：真实 `config.toml` 复制件
- 打包测试：macOS 和 Windows 各安装一次

## 下一步

下一步建议先完成 M1 验收：安装依赖，启动 Tauri dev，确认页面能读取本机 Codex 配置。随后进入 M2，开始替换脚本里的 TOML 写入逻辑。
