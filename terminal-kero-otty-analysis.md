# LingXia Terminal 对标 Kero / Otty 的能力建议

> 调研日期：2026-07-30  
> 范围：只讨论 Terminal 本身。本文明确区分 `lingxia-terminal` crate 与 macOS/Windows 宿主的职责，是方向草案，不是承诺版本表。

## 结论

LingXia Terminal 已经具备可用原生终端的主要骨架：跨平台 PTY、Alacritty VT 状态机、scrollback、真彩色与常用字符样式、IME、鼠标/滚轮、复制粘贴、标签、分屏、拖拽重排、缩放和动态标题。

后续工作不能笼统地写成“Terminal 实现”。建议固定以下边界：

- **`lingxia-terminal` crate 是单个 terminal session 的跨平台 engine**：负责 PTY、VT、scrollback、协议解析、语义事件、查询和平台无关的数据 contract。
- **macOS/Windows 是 terminal workspace 与 OS integration**：负责 tabs、panes、布局、绘制、输入法、快捷键、剪贴板、系统通知、配置保存和磁盘持久化。
- **协作项通过 contract 连接**：crate 提供数据和操作，两个宿主做一致的 UI 与系统行为；宿主不能各自从可见 cell 重新推断终端语义。

优先级上，crate 应先补 session spec、OSC 语义、scrollback search、hyperlink 和可恢复状态导出；宿主应同步补 find UI、布局恢复、安全确认、通知、配置和渲染一致性。GPU、inline images 和自动补全后置。

## Owner 定义

全文使用三个 Owner：

| Owner | 含义 |
|---|---|
| **crate** | `crates/lingxia-terminal`。不得依赖 AppKit、Win32、系统通知中心或平台配置目录。 |
| **host** | macOS `lingxia-sdk/apple/.../terminal/` 与 Windows `lingxia-windows-sdk/.../terminal_*`。两个平台分别实现 UI/OS integration，但共享同一 crate contract。 |
| **crate + host** | crate 产出结构化状态/事件/查询，host 消费并呈现；验收必须覆盖两个平台。 |

一个简单判断原则：

- 与“终端字节代表什么”有关，放 crate。
- 与“窗口中如何展示和操作”有关，放 host。
- 与“写哪个文件、弹哪个系统窗口、请求哪个系统权限”有关，只能放 host。

## 当前代码的实际职责

| 层 | 当前已有 | 当前缺口 |
|---|---|---|
| **crate** | `portable-pty`、`alacritty_terminal`、内存 session registry、PTY I/O、256 色/true color、常用 SGR、宽字符与组合字符、scrollback、cursor/title、alternate screen、bracketed paste、mouse wheel、渲染 snapshot | 创建 API 只有 cols/rows；没有 cwd/argv/env spec、typed event、OSC 7/8/9/52/99/133 语义输出、command blocks、search API、restore-state export/import |
| **macOS host** | 原生网格、IME、selection、copy/paste、scrollbar、tabs、rename、四向 split、拖拽重排/调整、pane/surface zoom、read-only | 无 find UI、链接交互、布局持久化/恢复、状态徽标、权限 UI、统一配置；当前显式关闭 ligature；未渲染 strike |
| **Windows host** | 原生网格、selection/copy/paste、IME、scrollbar、tabs、rename、split、pane focus/drag | 同样缺少 find UI、链接交互、布局持久化/恢复、状态徽标、权限 UI 和统一配置；GDI 路径不适合长期承载复杂 shaping、彩色 emoji 和 GPU 滚动 |

这里有两个容易混淆的 “snapshot”：

- 当前 crate 的 `TerminalSnapshot` 是**渲染快照**，服务当前 viewport 绘制。
- session recovery 需要的是**恢复状态**，包括可恢复 scrollback/cwd/profile 等。它应由 crate 提供导出/导入 contract，但 crash-safe 文件写入与 tabs/panes 布局属于 host。

## 竞品中值得借鉴的 Terminal 能力

### Kero

[Kero](https://kero.sh/) 的 Terminal 部分值得借鉴：

- 多 tab、每个 tab 可继续分屏，支持 focus/resize/zoom 快捷键；
- 重启后恢复 tabs、pane layout、cwd 和旧 scrollback，在旧内容下启动新 shell；
- 保持用户原有 shell、prompt、alias 和 dotfiles；
- bell、notification escape 和 OSC 9;4 progress 进入宿主 UI；
- 字体、Nerd Font symbols、scrollback find；
- 危险粘贴和 OSC 52 剪贴板访问确认。

其[公开 changelog](https://kero.sh/changelog/)还说明：隐藏 tab 不应持续占用绘制资源；IME、键盘布局、动态标题、TUI 背景、恢复内容和大型 scrollback 都需要专项处理。

### Otty

[Otty](https://otty.sh/) 值得借鉴的 Terminal 能力包括：

- GPU 渲染、平滑滚动、ligature、Unicode cluster、emoji、true color 和丰富 SGR；
- tabs、自由分屏、拖拽、状态徽标、unread 状态；
- recovery、reopen closed、profiles、themes、fonts、keybindings、config hot reload；
- find、command outline、hint mode、vi mode、read-only mode；
- OSC 8 hyperlink、普通 URL/路径识别、Kitty/iTerm2 inline images；
- shell integration、progress 与 notification 协议；
- inline autocomplete、command/snippet/layout recipes。

最值得优先采用的是协议驱动的语义层：[OSC 7/133 shell integration](https://docs.otty.sh/terminal-features/shell-integration)、[OSC 9;4 progress](https://docs.otty.sh/terminal-features/progress-state)、[OSC 9/777/99 notifications](https://docs.otty.sh/terminal-features/notifications)、[session recovery](https://docs.otty.sh/workflows/session-recovery) 和 [OSC 8/URL/path links](https://docs.otty.sh/user-interface/files-and-links)。

Otty 官网目前仍只提供 macOS 下载，Windows/Linux 标为后续，因此其跨平台与 DirectX 描述只能作为设计参考。

## 能力归属总表

| 能力 | `lingxia-terminal` crate | macOS/Windows host |
|---|---|---|
| Session 创建 | 定义 `TerminalSessionSpec`；按 cwd/program/argv/env/size 创建 PTY | 读取用户 profile，解析为 spec，决定何时创建/关闭 session |
| Lifecycle | 产出 created/running/exited/closed、exit code、foreground process | 映射到 tab/pane 状态，决定关闭确认和 UI 生命周期 |
| Tabs / panes | 不拥有 tab 或 pane tree | 完全负责 tab/pane model、split ratio、focus、drag、zoom、tab switcher |
| VT / OSC | 解析 VT 与 OSC，产出 typed event；响应必要 query | 不重复解析 escape bytes；只消费事件并呈现 |
| Cwd / command blocks | 跟踪 OSC 7、OSC 133，提供 cwd 与命令边界查询 | 新 tab/split 继承 cwd；提供 command navigation UI |
| Scrollback | 保存逻辑行；提供 search、match range、scroll target | 提供 find bar、快捷键、结果高亮与滚动交互 |
| Hyperlink | 保留 OSC 8 metadata；可提供 URL/path detector 和解析结果 | modifier hover/click、鼠标形态、右键菜单、调用系统打开 |
| Progress / bell | 将 BEL、OSC 9;4、OSC 9/777/99 变成 typed event | tab badge、unread、声音、系统通知、点击通知定位 pane |
| Clipboard / paste | 暴露 OSC 52 request；提供 bracketed-paste 状态与危险文本分类 helper | 访问系统剪贴板、显示确认 UI、保存 ask/allow/deny 决策 |
| 单 session 恢复数据 | 定义 versioned `TerminalRestoreState`；导出/导入 cwd、title、profile reference、受限 scrollback | 调用 export/import，并决定何时 snapshot |
| Workspace 恢复 | 不负责 tabs/panes 文件，不执行磁盘 I/O | 定义 tab/pane/layout wrapper；原子落盘、crash recovery、损坏回退、reopen closed |
| Theme colors | 接收并解析 ANSI palette/default fg/bg；输出 resolved cell colors | 保存主题选择，跟随 light/dark，绘制 chrome/cursor/selection |
| Fonts / shaping | 输出完整 text cluster、cell span、style、hyperlink 等 renderer contract | 选择字体、fallback、CoreText/DirectWrite shaping 与实际绘制 |
| Input | 提供平台无关 key/mouse protocol encoding、PTY write | IME、key event、快捷键冲突、selection、mouse hit testing |
| Accessibility | 提供逻辑行、cursor、selection 等可查询语义 | 实现 NSAccessibility/UI Automation tree |
| GPU / animation | 不依赖具体 GPU API | Metal、DirectWrite/Direct2D 或其他平台 renderer |
| Settings persistence | 只定义必要的 spec/DTO，不读写用户配置 | 设置 UI、profile 存储、热更新、平台路径 |

## 第一阶段：Semantic Core 与基础体验

先补齐跨平台语义 contract 和每天都会使用的基础能力。完成这一阶段后，crate 能稳定表达 session 状态，两个 host 能提供一致的搜索、恢复入口、链接和安全交互。

### 1. `TerminalSessionSpec` 与增量事件

**Owner：crate**

将 `terminal_create(cols, rows)` 演进为向后兼容的 options API：

- cwd、program + argv、env overlay；
- initial cols/rows、scrollback limit；
- stable session ID 与 lifecycle；
- title、cwd、foreground process、exit status、bell、progress、notification、hyperlink typed events；
- 每类事件带 sequence/generation。

渲染 snapshot 与 session event 分开：前者服务绘制，后者服务状态与恢复。`profile` 本身由 host 保存，crate 只接收解析后的 session spec；恢复状态如需关联 profile，应保存稳定的 profile ID，而不是让 crate 读取设置。

### 2. OSC 7 + OSC 133 shell integration

**Owner：crate + host**

crate：

- 提供 zsh/bash/fish/PowerShell 可审计 integration 脚本或资源；
- 在 spawn 时按 spec 启用，不修改用户 rc 文件；
- 解析 OSC 7、OSC 133 A/B/C/D；
- 输出 cwd、prompt/input/output range、exit code。

host：

- 提供启用开关；
- 新 tab/split 使用 focused session 的 cwd 创建新 spec；
- 呈现 command navigation、成功/失败 gutter 等 UI。

不支持 integration 的 shell 必须继续作为完整普通终端工作。

### 3. Scrollback search

**Owner：crate + host**

crate：

- 搜索完整逻辑 scrollback，而不是当前 viewport cell；
- 支持 plain/case-sensitive/regex；
- 返回稳定 match ranges、结果数和可滚动目标；
- 大 scrollback 搜索可取消，不长期持有全局 session lock。

host：

- find bar、find next/previous、关闭与焦点快捷键；
- 搜索高亮、当前结果高亮、滚动定位；
- resize 后重新映射可见结果。

### 4. Session recovery

**Owner：crate + host，但 crash-safe persistence 明确属于 host**

crate 只做：

- 定义 versioned `TerminalRestoreState`；
- 导出 cwd、title、profile ID 和有大小上限的 scrollback；不恢复旧 VT modes，fresh shell 从干净状态开始；
- 从 restore state 创建 fresh shell，并把恢复内容与新输出明确分隔；
- 对未知 schema version 返回可识别错误，不自行找文件。

host 负责：

- tabs、pane tree/ratio、active tab/pane、manual title、zoom state；
- 决定 snapshot 时机；
- Application Support/AppData 路径、原子写入、crash-safe persistence、损坏回退；
- 启动时恢复和运行期 reopen closed；
- 默认不自动重跑普通命令。

因此文档中的 “versioned session snapshot、crash-safe persistence” 不能作为一个 crate task：前半是 contract，后半是 host task。

### 5. Hyperlink、URL 与路径

**Owner：crate + host**

crate：

- 从 Alacritty VT cell 保留 OSC 8 hyperlink metadata；
- 对普通输出提供可测试的 URL/path detector；
- 使用 session cwd 解析相对路径；
- 返回 target、visible range、source（OSC 8 或 heuristic）。

host：

- modifier hover/click、underline、pointer cursor；
- 与 selection 的手势冲突处理；
- 右键菜单、复制和系统打开；
- 外部打开前执行权限策略。

### 6. 输入与剪贴板安全

**Owner：crate + host**

crate：

- 暴露 bracketed-paste 状态和 OSC 52 request event；
- 提供纯函数判断多行、控制字符、末尾换行等危险 paste；
- 对 notification payload 做协议级长度上限和控制字符清洗。

host：

- 读取/写入系统剪贴板；
- 显示确认 UI；
- 保存 ask/allow/deny 策略；
- 为远程 session 应用更严格默认值；
- 决定是否执行 open URL/file、notification 和 attention request。

## 第二阶段：Terminal UX 与系统集成

在第一阶段 contract 稳定后，完善状态呈现、个性化配置、键盘操作、文本渲染与系统 accessibility。

### 7. Progress、badge 与通知

**Owner：crate + host**

crate 将 BEL、OSC 9;4、OSC 9/777/99 统一为：

```text
idle | running(indeterminate|percent) | paused | succeeded | failed
```

host 在 tab 上显示 running/completed/error/unread，按前台状态与用户权限发送声音或系统通知，并让通知点击回到准确 pane。host 不得从普通输出文本猜测状态。

### 8. Profiles、主题、字体与快捷键

**Owner：以 host 为主，crate 只接收解析后的配置**

crate：

- `TerminalSessionSpec` 接收 shell/program、argv、cwd、env、scrollback limit；
- theme input 接收 ANSI palette、default fg/bg；
- 不读取用户配置文件，不保存 profile，不定义平台快捷键。

host：

- profile/settings schema 的用户级存储和迁移；
- font、line height、padding、light/dark theme、cursor/selection colors；
- copy-on-select、paste policy；
- tab/split/focus/resize/zoom/find/clear keybindings；
- 配置热更新与作用范围。

### 9. Pane 与 tab 的完整键盘操作

**Owner：host**

- 新建、关闭、切换、重命名 tab；
- 四向 split/focus、调整比例、equalize、pane zoom；
- tab switcher；
- 可重绑快捷键与 TUI 冲突处理；
- 关闭仍有前台进程的 pane 时确认。

crate 只需提供可靠 lifecycle/foreground-process 状态和 session close API。

### 10. 文本渲染正确性

**Owner：crate + host**

crate 必须完整输出：

- strike、underline style/color、faint、inverse、hidden；
- grapheme cluster、combining mark、wide char、variation selector、ZWJ emoji；
- cell span、hyperlink、cursor 与 selection 所需的逻辑位置。

host 必须正确完成：

- font fallback、Nerd Font symbols、CJK、彩色 emoji；
- ligature shaping 与 cell mapping；
- box drawing 的 DPI 对齐；
- selection/cursor/IME candidate rect 与实际 glyph 对齐。

### 11. Accessibility

**Owner：crate + host**

crate 提供逻辑行、cursor、selection、可见范围等只读查询；host 分别实现 NSAccessibility 和 Windows UI Automation tree。accessibility 不应从绘制命令或截图反推文本。

## 第三阶段：高级增强

这些能力有价值，但依赖前两阶段的语义、渲染 contract 和性能基准，不应阻塞基础体验交付。

| 能力 | crate | host |
|---|---|---|
| GPU renderer / smooth scroll | 提供高效增量 render contract 和 benchmark input | Metal、DirectWrite/Direct2D/GPU 绘制、动画与 frame scheduling |
| Inline images | 解析 Kitty/iTerm2 协议，产出受限 image placement/event | 解码、缓存、内存预算、绘制和 DPI 处理 |
| Autocomplete / inline suggest | 提供 OSC 133 prompt/input range；不读取用户历史数据库 | 候选 UI、设置、历史隐私策略、快捷键 |
| Hint / vi mode | 提供 link/search/selection 查询 | mode UI、标签绘制、键盘交互 |
| Command palette / recipes | 提供可调用的 session actions | palette UI、快捷键、profile/layout recipes 持久化 |
| Remote policy | 标记/接收 remote session policy，限制协议事件 | SSH 场景设置、用户提示、clipboard/notification 默认策略 |

## 验收指标

| 指标 | crate 验收 | host 验收 |
|---|---|---|
| 恢复 | restore-state version 兼容、大小限制、未知版本安全失败 | 异常退出后最近布局不丢失；损坏文件安全回退；不自动重跑普通命令 |
| 兼容 | zsh/bash/fish/PowerShell、vim/less/tmux 的 VT/OSC/mouse/resize 单元与集成测试 | 两个平台真实输入、IME、selection、tab/pane 生命周期测试 |
| 搜索 | 大 scrollback 搜索可取消、不阻塞其他 session、range 稳定 | find UI 不阻塞输入，结果可跳转，resize 后高亮正确 |
| 性能 | metadata event 不复制完整 grid；session 间锁隔离；持续输出仍可写入 | 不可见 tab/pane 不绘制；持续输出时低输入延迟；无主线程长任务 |
| 安全 | OSC payload limit、sanitize、dangerous-paste classifier 测试 | 权限默认值、确认 UI、系统 clipboard/notification/open 行为测试 |
| 渲染 | 输出完整 cluster/style/span/hyperlink contract | CJK、emoji、combining、box drawing、SGR、IME 的视觉回归 |
| Accessibility | 逻辑文本/cursor/selection 查询稳定 | 屏幕阅读器识别 focused tab/pane、当前行、selection、cursor |

## 来源

- [Kero 官网](https://kero.sh/)
- [Kero changelog](https://kero.sh/changelog/)
- [Otty 官网](https://otty.sh/)
- [Otty Terminal 文档](https://docs.otty.sh/)
- Otty 具体参考：[session recovery](https://docs.otty.sh/workflows/session-recovery)、[shell integration](https://docs.otty.sh/terminal-features/shell-integration)、[progress](https://docs.otty.sh/terminal-features/progress-state)、[notifications](https://docs.otty.sh/terminal-features/notifications)、[files and links](https://docs.otty.sh/user-interface/files-and-links)、[autocomplete](https://docs.otty.sh/terminal-features/autocomplete)

实现时应依据终端协议原始规范和现有依赖的公开 API 独立完成，不复制竞品实现。
