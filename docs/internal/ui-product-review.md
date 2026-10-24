# GPU UI 产品、语言与架构评审

状态：2026-10-23 独立评审；后续修复与验收记录见“实施进度”。原始问题描述保留为评审依据。

本次从现有源码、独立最小样例和当前 Pixel 3 画面重新判断，不以旧验收清单
或测试数量推导产品完成度。范围包括 TSX/Tailwind 作者体验、共享运行时、
平台适配、页面体验、工具与验证；native-component gallery 仍不属于页面迁移目标。

## 产品判断

当前实现已经具备可复用的渲染基础，但作者契约和模块边界尚不足以作为长期
演进的产品基线。主要矛盾是：作者写的是熟悉的 TSX/Tailwind，实际却需要理解
多套隐含约束，以及 View 生命周期、原生几何和编译器实现细节。

目标应当是“可预测的原生 UI 语言”：普通应用代码清楚、类型可靠、失败可定位，
页面操作自然且可恢复，同一份 UI 的语义由共享层定义。GPU 是实现方式，不应
成为页面作者处理生命周期、图片缓存和输入几何的理由。

产品成功条件：

- React showcase 中约定范围内的任务、信息和导航可等价完成；视觉优化应保持
  任务含义与信息层级，不能用删内容代替对齐。
- TSX 组合、状态和事件有明确语义；已知不支持的能力在开发时说明具体原因。
- Tailwind 支持范围内保留真实 CSS cascade、尺寸、继承和状态含义；范围外
  明确拒绝，不能靠类名看起来相同宣称完整 CSS 支持。
- 切页、输入、滚动、图片加载、后台恢复有一致的表现与错误恢复路径。
- 新平台接入主要实现 OS 适配，不重新定义列表身份、命中、动画与提交规则。

## 确认的问题与风险

P0 表示应在继续扩大作者能力之前解决；P1 表示应进入下一阶段产品基线。
“确认”是当前代码或本次执行证据；“风险”不代表已在所有平台复现。

### 1. P0：同一种作者错误的失败阶段不一致【确认】

位置：[compiler.mjs](../../packages/lingxia-ui/compiler.mjs)，`classAttributes`
只记录直接的 JSX 字符串属性；表达式中的字符串虽进入 candidates，却不接受
同等构建期验证。[classes.js](../../packages/lingxia-ui/src/classes.js) 在渲染时
才抛出对应错误。

本次实际运行：

| 写法 | 构建 | 渲染 |
|---|---|---|
| `className="flex-col"` | 失败，定位到文件与行列 | 未执行 |
| `className={"flex-col"}` | 通过 | `UI_CLASS` 失败 |
| 条件分支包含 `"flex-col"` | 通过 | 选中该分支时失败 |
| `<text {...{typo:true}}>` | 通过 | `UI_PROP: typo` |

后果：一个页面初次打开成功，不代表切换主题或业务状态后还能显示。

决定：编译阶段收集 className 表达式中可静态确定的完整候选和来源，覆盖常量、
条件、对象映射及约定的 class 合成函数。明确区分“未知类”“已知但不支持的声明”
和“无法静态分析”。不把所有普通字符串的错误都升级为构建错误，那会误伤文案。
动态属性仍保留运行时防线，但静态已知非法输入必须尽早失败。

### 2. P0：TSX 组合承诺不完整【确认】

位置：[runtime.js](../../packages/lingxia-ui/src/runtime.js)，`h`、`resolveNode`；
[index.d.ts](../../packages/lingxia-ui/src/index.d.ts) 的 `Props.children`；编译器 JSX 配置。

- `function Card(props) { return <column {...props}/> }` 接收 children 后构建通过，
  渲染报 `UI_PROP: children`。类型允许的转发方式与运行时契约不一致。
- `<><text>Hello</text></>` 构建通过，渲染报 `React is not defined`。
- 编译审计把所有名为 `window` 的 Identifier 都视为 DOM 使用；
  `const metadata = { window: 1 }` 也会误报。

决定：规范 children 优先级、数组/null/boolean、Fragment、组件返回值和 key；
为 Fragment 提供本语言实现或准确的编译诊断，禁止泄露 React 隐式依赖。
DOM 检查必须区分自由变量与属性名/局部绑定；这只是 DX 检查，真正的能力隔离
仍由受限 View 引擎负责，不能把 AST 黑名单当作安全边界。

### 3. P0：作者契约与 wire schema 混在一起、校验多处重复【确认】

位置：[schema.rs](../../crates/lingxia-ui/src/schema.rs) 的 `typescript_schema`，
以及 `compiler.mjs`、`runtime.js` 各自的 intrinsic 属性白名单。

Rust 生成 append-only wire 常量是正确的；但同一个 Rust 格式化字符串还生成
`View`、资源 hooks、JSX 组件和输入绑定等高层作者 API。编译器与运行时随后又
分别列举允许的 props。此前 `itemKeys` 的编译器遗漏是这种结构的具体后果。

决定：分开 wire contract 与 authoring contract。wire 的 Rust 校验和 JS 常量
继续同源生成；TSX 元素、children、互斥属性和能力状态来自一个作者契约定义，
供类型生成、编译审计和 JS 校验共同使用。原生层仍独立验证不可信 wire，不能
因为“去重复”而移除必要的安全校验。

### 4. P0：语言/原生产物能力没有形成完整的发布契约【确认＋风险】

编译模块有 `schema 0.25` 注释，`ViewSession` 主要检查入口函数是否存在。
lxapp 层已有 GPU renderer factory 可用性检查，应保留；但它不等于对 field102、
动画、引擎任务队列等具体语义版本的检查。2012 与新版 SDK/native 必须人工配套，
表明这条发布链尚需收敛。

决定：编译产物附带 `uiAbi`、实际所需 features、编译器版本和来源映射；host
在创建页面之前检查兼容性，向 devtools 返回结构化诊断。通过 ABI 与能力检查，
而非单纯比较包版本字符串。未知能力不得先打开页面、再在首帧变成通用错误。
具体接入位置还需与现有安装校验及 bridge 协议共同设计，不能另建授权入口。

### 5. P1：Tailwind 布局与元素结构各自控制一半语义【确认】

`row/column` 决定布局轴，而 `flex-col` 等常用方向工具当前不支持。
页面因此通过 viewport 计算、切换组件标签与 inline style 实现响应式。
`grow`、basis、intrinsic size、默认控件尺寸的历史修补也说明需要统一基础模型。

决定：增加通用布局原语（建议 `<view>`），由 `flex`、`flex-col`、响应式方向等
受支持的 Tailwind 声明控制布局。现有 row/column 可作为可解释的默认布局语法糖；
默认值、class 与 inline 的优先级必须写明。不要把所有标签机械替换为 HTML div，
也不要在每个示例里写一套布局换算。

优先完成常见能力的语义闭环：布局轴/尺寸、文本继承、overflow、disabled、
pressed/focus-visible、动画与 reduced-motion。每项必须覆盖编译、类型、渲染、
交互与诊断，而不只增加一个 utility 映射。完整 CSS 浏览器不是当前产品承诺。

### 6. P1：类型没有贯通页面契约和资源调用【确认】

`View<A>` 只能参数化 actions，data 固定为 `Record<string, unknown>`；示例频繁
断言和 `String/Boolean` 转换。`useRequest/useStream` 的 method 是任意字符串，
params 是 unknown；作者需要手动维持 action 名称、参数和返回值对应。
输入 `bind/value/onInput` 在类型上可同时出现，运行时才拒绝。

决定：以一个共享 Page Contract 推导 View data、actions、资源参数和结果，
不复制业务类型到 Rust schema 生成器。绑定模式与受控模式使用互斥联合类型；
状态模型有清晰的 idle/pending/success/error/canceled 分支。保留显式具名 local
状态的身份规则，并说明它与 React hooks 的区别，不能让函数名代替生命周期文档。

### 7. P1：页面承担过多宿主几何与设计系统职责【确认】

Home、ToDo、Stream 等重复计算 viewport、insets、keyboard、spacer 和尺寸。
多数页面各自定义 Header、Panel、按钮 class 与空/错误状态；旧的 Screen/Details
辅助组件也未形成统一的作者入口。当前 Device 真机画面可用，但按钮、标题、
图标和空白的组合仍需要产品级统一评审，不能从“节点可点击”推导视觉成熟。

决定：提供遵守 host geometry contract 的 Page/页面内容布局组件，明确哪一层
消费安全区与键盘避让，背景延伸与内容避让分别定义，避免双重扣减。
示例自己的设计系统持有品牌 tokens、Button、Field、Section、Result、EmptyState、
ErrorState；这些不是 renderer 特例。动态数据的 geometry 可以使用 style，
不追求把每个计算值强行拼成 Tailwind 类。

### 8. P1：错误体验和可访问性仍未达到产品基线【确认＋待验证】

`GpuView.showError` 有保留旧帧的正确选择，但更新失败主要显示 `UI update failed`
Toast，首次失败是固定 TextView；缺少明确的可恢复状态和用户重试路径。
`ImageProps` 没有通用 alt/label/decorative 作者语义；基础 Props 也未提供完整
的语义角色契约。当前环境与渲染链只有 density，没有显式 fontScale、文本方向、
reduced-motion 契约。这里的跨平台一致性未被证明。

决定：错误按可恢复资源失败、页面候选失败、会话失效分类；保留最近成功画面，
用户获得可操作的恢复方式，devtools 获得页面/节点/阶段/源码位置。
文本缩放、语义树、键盘焦点与运动偏好成为基础验收，而非截图通过后的附加项。

### 9. P1：平台共享边界尚未完成【确认】

`lingxia-ui` 已无 lxapp/page-stack 权限依赖；动画、图片请求身份、编辑状态和部分
图表命中在 Rust，共享方向正确。图片读取仍经 host facade 接到 managed storage，
不应再做一套 renderer 下载缓存。

但 Android `GpuView` 仍管理帧队列、多个 generation、surface、bridge、输入、
手势、键盘导航、accessibility 和错误 UI；`GpuClip` 计算命中几何，`GpuScroll`
计算滚动与 reveal 策略。这些策略在新平台有重复实现的风险。
`module.rs` 在 QuickJS 使用专用 ESM/任务队列实现，其他引擎的 drain/discard
是空实现；`budget.rs` 也只对 QuickJS 开启 watchdog。选中了 JSC/ArkJS 依赖
不等于 iOS/Harmony 已拥有同等生命周期与执行预算。

决定：共享层定义 identity、committed geometry、语义命中、滚动/reveal、帧票据
与生命周期转换；平台保留 OS 手势识别、vsync、Surface、IME、系统剪贴板和
accessibility 投影。主线程低延迟输入可以使用共享层输出的不可变命中快照，
不能为了“共享”而给每个 move 增加阻塞 JNI 往返。引擎适配单独列能力和验收。

## 目标结构与依赖边界

先按职责整理现有 package/crate 内部，再由真实依赖或构建成本决定是否拆包。
文件变短不是成功指标；事务边界、数据归属和可测试性才是。

```text
packages/lingxia-ui/
  src/authoring/       公开 TS 类型、JSX、hooks 门面
  src/runtime/         tree、identity、state、resources、candidate commit
  src/style/           已编译规则的匹配/继承；无 CSS parser
  src/generated/       wire constants 和生成的契约产物
  compiler/            TSX 分析、诊断、Tailwind/CSS lowering、产物清单
  contracts/           作者元素/属性/children/能力定义
  tests/               类型、作者用例、编译/运行一致性

crates/lingxia-ui/src/
  protocol/            wire decode、bounds、compatibility
  engine/              ESM、jobs、interrupt/budget 适配
  session/             View 生命周期、candidate/commit/presentation
  layout/              geometry、text shaping、virtual list
  interaction/         命中、编辑语义、selection、scroll policy
  scene/               不可变 display/semantic snapshots
  animation/           时间、identity、效果采样
  resources/           解码与请求生命周期；无网络/磁盘缓存权限
  renderer/{cpu,gpu}/  paint/backend/surface resources
  platform/android/    JNI 编解码与平台资源桥接

lingxia facade + lxapp  route authority、页面栈、资源权限、managed cache
platform SDK           Surface/vsync/OS input/IME/accessibility adapter
showcase               品牌 tokens、业务流程、示例组件
platform tests         设备交互与录屏；不进入小程序 bundle
```

依赖方向：作者代码 → 作者 API；编译器 → 作者契约与 wire 生成数据；运行时 →
契约与已编译样式；共享会话 → 布局/语义/scene；renderer 消费 scene；平台适配
消费共享接口；host facade 注入有权限的资源服务。布局和 scene 不应反向依赖
Android、页面 route、业务组件或 Tailwind 类名。

现有 `runtime.js` 约1009行、`GpuView.java` 约1358行，且职责交叉；`render.rs`
约6952行、`session.rs` 约2658行，后两者包含大量测试，不能直接当成生产复杂度。
优先拆事务阶段和职责，再迁移关联测试；不要一次搬动所有文件制造大 diff。
`schema.rs` 保留 wire 知识，但不再承载手写高层 TypeScript API 字符串。

## 实施顺序与验收

1. **先建立语言基线。** 修正本次可复现的 TSX/diagnostics 缺陷，统一作者契约；
   类型与 runtime 对绑定、children、key 的判定一致。提交包含用户写法的回归用例，
   包括被隐藏的条件分支，不以 implementation-mirroring 测试替代。
2. **打通发布能力契约。** 新 bundle/旧 native、缺失 feature、引擎能力不足在
   页面创建之前准确拒绝；普通升级不依赖人工记住 .so/SDK 组合。
3. **做三条完整产品纵切。** Home 覆盖输入/图片/主题/语言；ToDo 覆盖状态、持久化、
   错误恢复与键盘；Stream 覆盖虚拟列表、异步、取消、动画和图表。先抽取这些页面
   的共同设计/布局能力，再迁移其余页面，避免提前设计通用组件大集合。
4. **收敛共享会话与平台边界。** 通过假的 surface/clock/input adapter 测试
   prepare、present、hide、destroy、late callback；随后在第二个平台做同一纵切。
   Android 测试通过不能替代 iOS/Harmony 证明。
5. **建立可重复的发布验收。** 干净环境单命令构建/安装/测试；保存版本、源码身份、
   设备、日志、逐帧录屏与结果。类型/编译、共享核心、平台、产品四层各有门槛。

视觉验收包括真实文本与长文本、中文/英文、字体缩放、横竖屏、键盘、浅深色、
无图/加载/失败状态；互动验收包括快速切页、后台恢复、拖动中失焦、任务取消、
列表重排与回到离屏内容。无整块黑屏检测只能证明该检测覆盖的现象；还要检查
首个有效内容、部分闪烁、布局跳动、输入延迟与帧时间分布。

首个实施批次是第1项，不是增加新动画类或再改一轮圆角。现有核心与回归证据
作为迁移保护；旧“完成度”结论不作为新产品基线的验收依据。

## 实施进度：第一批作者语言修复

已落实：局部常量/条件表达式中的已知 Tailwind 类在构建时诊断；静态已知 spread
属性检查；组件 children 透传与显式 JSX children 优先级；DOM 属性名/局部绑定
误报修复。Fragment 改为明确的构建期拒绝，尚未实现 Fragment 渲染。
编译器与运行时共用 `src/authoring/props.js` 属性规则，源码分析放在 `compiler/`。
后续已将高层 TypeScript API 移到 `src/authoring/index.d.ts`，Rust 仅生成样式类型；
输入绑定使用互斥联合类型，children 按文本/叶节点/容器检查；`View<Actions, Data>`
支持页面数据类型。跨模块常量与对象样式映射也已纳入 class 分析。

发布链已接入 ABI/wire 字段/引擎能力 manifest、native 执行前检查与 bundle 旧宿主
保护；CLI release 压缩保留 manifest。兼容声明目前采用 bundled runtime 的保守基线，
尚未实现按页面裁剪所需 features。Home/ToDo/Stream 开始共用 `PageContent` 管理
安全区与键盘避让。2014、2015 已安装到 Pixel 3 真机验证。

验证：185 项 JS 测试、291 项 native 单元测试通过；GPU showcase 类型检查、release
构建与 `cargo check --workspace --all-targets` 通过。
原评审的七个样例重新执行，结果见 `target/gpu-authoring-verification/results.json`；
组件样例构建/渲染成功，其余非法输入提前诊断，普通属性样例通过。
2014 验证 Home 输入/图片/主题/语言、ToDo 键盘、Stream 历史与键盘，以及深浅主题
共 94 次 tab 切换；录像 368/495 帧未命中整块黑屏检测。2015 增加 `<view>`、
`flex`/四种方向与响应式方向（wire 103/104），ToDo 宽屏与全流程、Stream 16 轮
长对话与键盘验证通过。新资源 hooks 支持 action 引用推导参数/结果，控制器移到
`src/runtime/resources.js`。两版安装哈希与日志索引见 showcase `tests/PARITY.md`。
这些结果不构成帧率或 iOS/Harmony 验收；字体缩放与会话恢复的后续结果见下方。
Fragment 与动画作者入口仍为显式拒绝能力，第二平台仍未验收。
具体约束见 [ui-authoring.md](ui-authoring.md)。

Android 候选失败恢复已补上原生重试提示、中英文资源和原生栏避让；重试在当前会话中
重新构建，失败候选回滚，不重置已提交本地状态。真机故障注入发现并修复了反馈布局
触发自动重试的问题；旧加载回调按页面 generation 丢弃，释放后加载失败不会保留
悬空 native 指针。172 项 Android UI 单元测试通过，独立宿主故障注入与前后截图
验收通过（`target/ui-recovery-device-final.log`、`gpu-recovery-{failed,restored}.png`）。
独立宿主改为依赖真实 SDK 模块，避免只编译 Java 源码而遗漏资源。后续已补上超时
会话的显式重载：停止失效会话输入，按当前 endpoint/generation 进入共享页面重载和
Logic 重连；旧点击/图表/滚动回调不能破坏新会话。2018 完整宿主 instrumentation
恢复测试通过，独立宿主真实无限循环中断/重载测试通过，14 项共享 GPU 生命周期测试
与 workspace/all-targets 检查通过。初始化失败恢复仍由页面创建方处理，尚未独立验收。

系统字体缩放已接入共享字号映射、排版/输入度量、离屏列表测量失效和 Android 系统 SP 转换。
297 项共享 UI、188 项 Android UI/chrome 测试及 workspace/all-targets 检查通过；SDK34 测试覆盖
非线性缩放，Windows browser-shell clippy 通过。2020 真机发现默认输入框固定高度阻止增高；
已改为默认最小高度并保留显式高度约束。截图复查发现原生 tabbar 大字体标签裁切，现按实际
字体度量增加栏高度，默认字号仍为 64dp。2022 真机通过 100%→200%→100% 字号切换、Home 草稿
保留与操作、ToDo 输入框增高、Stream 长消息历史、正常字号 Home 回归和 47 次切页；364 帧录像
无整块黑屏命中。Android14 非线性仅有 SDK34 测试，未在 Android14 真机、iOS 或 Harmony 验收。

图片语义已补上 `alt`：非空描述进入共享语义快照，空字符串表示装饰图并退出读屏树，
省略保留旧行为。wire105、类型、runtime、原生解码与 Android 投影一并更新；Home 背景与
重复 Logo 标为装饰图，剪贴板与媒体预览补充描述。2019 真机通过图片语义和 Home 回归，
186 项 JS、294 项共享 UI、176 项 Android UI 测试及 workspace/all-targets 检查通过。
证据见 showcase `tests/PARITY.md`；未验证读屏语音与其他平台。

Home、ToDo 的 View/Logic 现共用数据与动作参数类型，注册的 Logic 方法检查是否满足 View
接口，去掉页面数据的重复断言与字符串兜底转换。`View.data` 使用导出的 `ReadonlyData<T>`
递归保护对象、数组和元组，与运行时冻结一致；Chart 接受只读数据。186 项 JS 测试、两侧
类型检查和 release 构建通过；2023 真机通过 Home 与 ToDo 完整回归。当前安装为 0.2.23，
原生库与 2022 相同；字体和切页录像来自 2022，未把它们写成 2023 新录制的证据。

## 原始评审证据与限制

- 新执行的最小样例：`node target/gpu-product-review.mjs`，结果保存在
  `target/gpu-product-review/results.json`。以上编译/运行失败均来自实际输出。
- 当前设备画面：`target/gpu-product-review/current.png`，仅覆盖 Device 深色页面；
  不代表本轮逐页重新验收。之前的录屏与检查是历史证据，不能替代新设计验收。
- 原始评审时没有修改实现或已安装应用；后续实施以上方进度为准。目标目录与未列为
  已实现的 API 仍是方案，不能当成已交付能力。
- 本报告不是逐行安全审计，也没有重新验证所有权限、设备能力和平台；这些保持
  原有专门文档与验收要求。性能问题需测量归因，不能仅凭文件行数得出结论。
