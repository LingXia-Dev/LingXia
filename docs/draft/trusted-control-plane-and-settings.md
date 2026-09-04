# 可信控制平面与 Host 设置

> 状态：实现中；本文同时记录目标 invariant 与当前主线落地状态
>
> 范围：browser WebUI 授权、host-wide display language 与桌面端 Settings 归属
>
> 兼容性：允许 Breaking changes

## 决策摘要

LingXia 保留每个 browser tab 一个 WebView 的架构。信任绑定到当前执行的
document，而不是 WebView、URL、app id 或 bundle source。

```text
CallerClass     = StandardApp | ControlApp | BrowserControlDocument
RouteAudience   = AppSessionOnly | AuthenticatedReadOnly | ControlAppOnly |
                  BrowserControlOnly | ControlOnly
DocumentSession = WebView instance + NavigationId + document generation +
                  loader attestation + random secret + CallerClass
```

- `CallerClass` 仅由 host bootstrap 的 native code 分配；JavaScript 不能请求、
  声明或推导 caller class。
- `RouteAudience` 是 SDK 内置封闭 enum，不是 app id，也不是调用方传入的值。每条
  registry route 都有一个 effective audience。
- 普通 host-defined/native-extension `#[lingxia::native]` route 的 `audience` 参数可省略；macro 在
  编译期默认 `AppSessionOnly`，所以既有普通 route 无需逐条标注。framework-owned
  control route 与手写 `HostRegistration` 必须显式指定 audience。
- browser session 精确绑定 WebView instance、`NavigationId`、loader
  attestation、document generation 与随机 secret。替换 top-level document 的
  navigation 会撤销旧 session。
- 外部 document 是 `Unauthenticated`，不是 `StandardApp`；它们没有 schema、
  `AuthenticatedCaller` 或 `AuthenticatedReadOnly` 访问权。
- `DisplayLanguageService` 已替换旧 single-slot `OVERRIDE`，统一拥有 Runner websocket lease、
  persisted preference、effective、effective source，以及 state/effective 两种 revisioned 事件。
- 桌面端最多一个启动时固定、纯数据的 `SettingsDestination`。它不持有 runtime、
  session 或权限；点击时才重新解析当前对象。未配置时 shell 不显示入口，显式调用 resolver
  返回 `SettingsDestinationResolveError::NotConfigured`。

本方案不引入第二个 renderer、通用 capability system、新的 `control.*`
JavaScript namespace 或 runtime Settings-provider registration。

## 为什么修改上一版方案

将可信 browser UI 与外部页面放入不同 renderer，会让 LingXia 在 native WebView 之上
重建 browser navigation model。例如：

```text
external A -> external B -> internal Settings -> external C
```

会产生两段独立历史；随后必须组合维护 back/forward、redirect、reload、BFCache、
scroll/form restoration、title、favicon、loading state、tab discard、crash restore 与
memory policy。这不是单纯的安全边界，而是 browser engine integration 项目。

`principal + capability set` 也超出当前问题。LingXia 已将 “capability” 用于 build
permission，bridge protocol 又用 `cap` 表示近似 namespace 的 metadata；再增加一种
含义会提高评审和排障成本。修订后的设计只回答必要问题：当前哪个 document 能认证一条
privileged bridge message。

## 目标与非目标

目标：

- 阻止外部 browser document 与普通 lxapp 调用 framework 或 host control route。
- 保留 authenticated lxapp 的 app-scoped native route，且不将它暴露给任意 browser
  document。
- 维持每 tab 一个 native WebView 与一份 native history。
- 使 schema discovery 与 runtime dispatch 共用一套授权规则。
- 在 navigation、renderer 丢失、detach、discard、teardown 时撤销所有 session-owned
  bridge 工作，并阻止 stale native outbound。
- 让 Rust、Logic、logic-disabled View 共享 display language 实现。
- 支持没有 home lxapp、默认 surface 为 browser、terminal、URL 或 native UI 的 host。

非目标：

- 不要求每个 host 提供 Settings screen。
- 不将 history、privacy、downloads、proxy 等 browser setting 合并到 global service。
- 不提供任意 runtime capability delegation。
- app id 不是 secret 或 authorization credential。
- `AppSessionOnly` admission 不替代 handler 的资源级授权。
- Sidebar action 不是 service discovery 或 authority registration。
- 不以 framework composite history 替代 browser engine history。

## 安全模型

### CallerClass 与 AppScope

`CallerClass` 是 native bootstrap 分配的封闭 enum：

| Caller class | 含义 | 示例 |
| --- | --- | --- |
| `StandardApp` | 已认证、但不是控制应用的 lxapp | ordinary 或 downloaded lxapp |
| `ControlApp` | bootstrap 选定、可使用 app-control routes 的 lxapp | home/control lxapp、可信 `logic: false` control UI |
| `BrowserControlDocument` | 已认证、只管理 browser domain 的当前 document | built-in 或 host 选定的 browser WebUI |

caller class 不在 `lxapp.json` 中声明。manifest 可以描述需求，却不能让 app 自行成为可信主体。
app id 继续用于 bundle lookup、storage、log、WebTag、automation 与 compatibility，
包括稳定的 `app.lingxia.browser`；字符串相等不授予权限。

caller class 表示授权范围，不表示组件包含关系。`ControlApp` 与
`BrowserControlDocument` 是并列 scope：前者不直接获得 browser profile 的 history、
bookmarks、privacy、proxy 或 download 权限，只能发起可信的 browser-settings navigation；
后者只管理 browser domain。少数确实跨两者共享的 host-wide route 必须显式标为
`ControlOnly`。

每个 `LxAppSession` 还带有 native-derived、不可由 caller 覆写的 `AppScope`：

```text
AppScope {
  app_identity,
  storage_namespace,
  native_issued_or_user_approved_resource_grants,
}
```

`AppScope` 来自 native session/用户确认，不来自 payload 的 `appid` 或 resource id。
`AppSessionOnly` 仅决定 route admission；handler 仍必须以 `AppScope` 和资源本身的 policy
执行资源级授权，绝不能信任 payload 声称的 app 或资源归属。

### AuthenticatedCaller

只有 native code 已建立 `AuthenticatedCaller` 后才计算 audience：

```text
AuthenticatedCaller =
  | LxAppSession {
      app_session_id,
      caller_class: StandardApp | ControlApp,
      app_scope: native-derived AppScope,
    }
  | BrowserDocument {
      session: active DocumentSession,
      caller_class: BrowserControlDocument,
    }
```

Native Rust call 不经过此 bridge policy。HTTP/HTTPS、`file:`、error page 与其他外部
browser document 都是 `Unauthenticated`，不是已认证的 `StandardApp`；它们不能调用
`AuthenticatedReadOnly`。

授权顺序固定：

1. 认证当前 app session 或 browser document session。
2. 没有 authenticated caller 时立即拒绝。
3. 用 caller kind、caller class 与 route 的 effective `RouteAudience` 授权。
4. 通过后才 decode route parameter 或调用 handler。
5. `AppSessionOnly` route 的 handler 再以 native-derived `AppScope` 执行资源级授权。

### RouteAudience

`RouteAudience` 是 SDK 内置封闭 enum：

| Audience | 允许的 caller |
| --- | --- |
| `AppSessionOnly` | `StandardApp` 或 `ControlApp` 的 authenticated `LxAppSession` |
| `AuthenticatedReadOnly` | 任意 authenticated caller；只读且对所有此类 caller 安全 |
| `ControlAppOnly` | 仅 `ControlApp` 的 `LxAppSession` |
| `BrowserControlOnly` | 仅 `BrowserControlDocument` 的 active `BrowserDocument` |
| `ControlOnly` | `ControlApp` 或 `BrowserControlDocument`；明确拒绝 `StandardApp` |

`AuthenticatedReadOnly` 的含义是 authenticated-public，不是 internet-public。它不得建立持久
订阅、租约或有副作用的资源分配，不得跨 app 泄露数据；外部 document 因未认证而不能
调用。`AppSessionOnly` 明确表示任意 authenticated `StandardApp` 或 `ControlApp` lxapp 可进入，
所以 host-wide mutation 必须显式使用 control audience。

### role、identity、caller class 与 audience 的边界

这些字段处于不同层，不能互相代替：

| 概念 | 谁产生 | 用途 | 不能证明什么 |
| --- | --- | --- | --- |
| bridge `hello.role` | protocol endpoint | 描述消息端角色，例如 `view` | caller identity、control authority |
| `app_identity` / `app_session_id` | native bootstrap | 绑定 app lifecycle、storage 与 resource grants | route audience 是否允许该 caller |
| `CallerClass` | native bootstrap / active `DocumentSession` | 把 authenticated caller 分类为 `StandardApp`、`ControlApp` 或 `BrowserControlDocument` | 具体资源 ownership |
| `RouteAudience` | route registration | 声明哪些 caller class/kind 可发现并 dispatch route | caller 是否已认证 |
| bridge `cap` | wire codec 从 route name 推导 | 检测 namespace/routing 不一致 | `CallerClass`、`RouteAudience`、`AppScope` |

授权必须先得到 authenticated identity/caller class，再比较 fixed `RouteAudience`，最后由 handler
根据 `AppScope` 检查具体资源。任何 payload field、route alias、appid equality 或 `role` 值都不能
改变这条顺序。

每条 unary、stream、channel、macro-generated 与 direct registration paths（`register_host_route`、
`HostRegistration`、`ChannelRegistration` 等）都必须在 registration 时形成
`EffectiveRoutePolicy`，其中含 effective audience。若未来引入其他 registration path，也必须
在 registration 时形成该 policy。schema filtering
与 dispatch 调用同一个 `authorize(caller, audience)`；dispatch 才是安全边界，filtered
schema 仅改善 ergonomics 并减少 information disclosure。

普通 host-defined/native-extension `#[lingxia::native]` route 的 `audience` 参数可选。省略时 macro 在
编译期写入 `AppSessionOnly`：

```rust
#[lingxia::native("editor.loadDocument", audience = "app-session-only")]
#[lingxia::native("system.getVersion", audience = "authenticated-read-only")]
#[lingxia::native("host.setAccount", audience = "control-app-only")]
#[lingxia::native("browser.setPrivacy", audience = "browser-control-only")]
#[lingxia::native("display.setLanguage", audience = "control-only")]
```

macro 的有效字符串仅为 `app-session-only`、`authenticated-read-only`、
`control-app-only`、`browser-control-only` 与 `control-only`；省略时等价于
`app-session-only`。

framework-owned control route 使用独立 registration API，该 API 在编译期强制给出
audience，绝不能落入默认值。底层手写 `HostRegistration` constructor 也必须显式接收
`RouteAudience` enum。macro 字符串仅在编译期映射为内置 enum variant，未知值产生
compile error。direct registration paths、generated registration paths，或未来引入的其他
registration path 缺少 policy metadata 时，必须在 registration 立即 fail，而不能拖延到
dispatch；route inventory test 必须覆盖整个
registry。调用方不能在 payload 中选择或覆盖 audience，`appid` 也不参与其计算。

现有 bridge field `cap` 若仍表示 namespace 或 route family，应按原义重命名，不能与
`RouteAudience` 混淆。

### 第一阶段的 appid/source guard 迁移记录

下表保留迁移前的 equality guards 与已经落地的替代 policy，便于回归审计；这些 guard
不再构成授权边界：

| 已删除/缩窄的 guard | 历史覆盖入口 | 当前 policy |
| --- | --- | --- |
| `ensure_home_lxapp` | `setDisplayLanguage`、`checkUpdate`、`screenshot`、`autostart`、`sidebarActions` | `ControlAppOnly` session policy |
| `require_home_caller` | surface `open`、`reconfigure`、`openApp`、`openBuiltin` 等 | `ControlAppOnly` session policy |
| browser-shell `require_builtin_browser` | browser-shell privileged routes | `BrowserControlOnly` document-session policy |

每一条迁移后的 route 都要在 schema 与 direct dispatch 测试“appid 相同但未获 caller class”时被
拒绝。source、URL、built-in asset 或 appid equality 仅可保留作 presentation/lookup，不能
作 authorization input。

### DocumentSession 与 session registry

browser WebView 是长生命周期 container，会在内部与外部 document 之间切换，因而不能
自身充当可信 principal。授权绑定短生命周期 document session：

```text
PendingNavigation {
  webview_instance_id, // 不可复用的 native WebView identity
  navigation_id,       // native/normalized top-level attempt identity
  expected_loader,     // opaque native loader/source attestation
}

DocumentSession {
  webview_instance_id,
  navigation_id,
  document_generation,
  loader_attestation,
  secret,              // cryptographically random bearer secret
  public_session_id,   // native-to-document frame 的非 secret binding
  caller_class,        // browser WebUI 固定为 BrowserControlDocument
  state,               // Pending | Active | Revoked
}
```

`DocumentSession` 的创建、activation 与撤销均由每个 WebView 的 session registry
linearization point 串行化。完整且不可复用的 binding 是
`webview_instance_id + navigation_id + document_generation + loader_attestation + secret +
public_session_id`。`authorize`、`revoke` 与 outbound
send 都在该线性化点比较完整 binding。

`NavigationId` 是进程级单调、不复用的 accepted top-level attempt identity。只有 normalizer
接受、且每个 `NavigationId` 仅交付一次的 `NavigationEvent::Started` 才是本方案所称的
navigation start；不是每一个 raw backend callback。同一 native navigation id 的 redirect
repeated start 由 normalizer coalesce，不重复 revoke 或 rekey；
`document_generation` 是 per-WebView 数值，仅在证实 current top-level document replacement
commit 后递增。每次由 normalizer 单次交付的 document-replacing
`NavigationEvent::Started` 创建 `PendingNavigation` 并撤销旧 session。
`PendingNavigation` 的 `webview_instance_id + navigation_id + expected_loader` 负责
将 accepted start 与 loader/commit 跨层关联；commit 必须取得该 `NavigationId`，不能用额外
counter 掩盖关联缺失。被更高 generation 取代的 stale commit/event 直接丢弃，绝不能撤销
新 session。若 backend 无法证明 commit 与当前
top-level generation 的关系，必须 fail closed：撤销可疑状态并经可信路径 reload，而不能
签发 authority。

当 commit 的 WebView instance、`NavigationId`、current top-level generation 与 loader
attestation 都匹配最新 attempt 时，native code 才可创建新的
`BrowserControlDocument` session。`secret` 证明 caller 持有可信 bootstrap；native message
metadata 或 document-scoped transport 证明 message 出自对应 top-level generation；两项
都必需，且不得由 URL、appid、DOM field 或 caller payload 推导。secret 不写入 log、URL
或 error，也不在 native-to-document frame 中回传。

control lxapp 使用同一 `CallerClass`/`RouteAudience` policy 与 `AppScope`。
除非其 WebView 也会在可信与不可信 top-level document 之间 navigation，否则不需要
browser 专用 document binding。

## Browser document 生命周期

每个 browser tab 保留一个 `BrowserRelaxed` WebView 及其 native navigation history。
native message endpoint 可以留在 WebView 中，但 endpoint 本身不授予 authority。

### Loader attestation：当前落地边界

主线已建立 WebView/browser 跨层关联：normalizer 分配不复用的 `NavigationId`，
`PendingNavigation` 将 accepted attempt、expected loader 与 top-level commit 关联；native-owned
load path 再提供不可由 document 伪造的 `NativeKey` 或 platform attestation。URL、`WebTag`、
appid 与当前可见地址都不参与授权。Apple、Windows 与 Android API 23+ 已具有正向证明路径；
HarmonyOS 保持 `Unsupported`/fail closed。history/BFCache 恢复的 internal document 不复用旧
证明，而是先保持 unauthenticated，再由 host 发起 fresh trusted load。
browser tabs 的 generation token 只用于 stale-drop，仍不单独构成 document auth。internal
browser page 的 `frame-ancestors 'none'` / `X-Frame-Options: DENY` 只缩小攻击面，不替代
provenance。

### Backend message provenance

跨平台 WebView callback 必须携带 message enqueue 时由 native 保存的 provenance，而不得
稍后根据当前 URL 重建：

```text
WebMessageContext {
  webview_instance_id,
  document_generation,
  top_level_proof,      // native frame metadata 或 document-scoped port
}
```

每个 backend 必须正面证明 control message 来自已 commit 的 current top-level document
generation。可使用 native source-frame metadata，或专为该 top-level document 创建并仅
交付给它的 message port。process-wide JavaScript interface、source URL、WebView 当前
state 都不是证明。缺少此 provenance 的 backend 必须 fail closed：可 render internal
document，却不能创建或 activate `BrowserControlDocument` session。delegate abstraction 必须保留
该 context，不能将 message 降为裸字符串。

| 平台 | 当前可信路径 | 无法证明时的降级 |
| --- | --- | --- |
| Apple | native load key + accepted `NavigationId`/generation + `WKScriptMessage.frameInfo` top-level proof | 任一 binding 不匹配即拒绝，不从 URL 重建 provenance |
| Android API 23+ | host-issued load token 关联 start/commit；每 document 新建、绑定 generation 的 `MessagePort` | stale navigation/frame/external/旧 port 一律拒绝并撤销旧 session |
| Android API 21/22 | process-wide JavaScript interface，无 provenance | `BrowserControlOnly` bridge fail closed；可渲染 internal UI，但 privileged UI 实际不可用，并记录产品影响与拒绝/降级 metrics |
| HarmonyOS | 尚无可证明 current generation 的完整 backend | browser control transport 当前拒绝；完成 per-document port/generation 与 commit 关联前不得启用 |
| Windows | WebView2 native navigation/top-level context + platform attestation；per-WebView FIFO dispatcher 保留 enqueue-time context | source、navigation 或 generation 失配即拒绝，不按 `WebTag`/当前 URL 补证 |

### 可信内部 document

1. Native code 将 internal navigation 解析到 host bootstrap 选定的确切 browser WebUI
   asset。
2. normalizer 接受且仅交付一次的 top-level document-replacing
   `NavigationEvent::Started` 在线性化点记录 `PendingNavigation`、撤销旧 session，并取消或
   detach 其工作；同一 native navigation id 的 redirect repeated start 已被 coalesce，绝不重复
   revoke 或 rekey。
3. 只有证实为 current top-level replacement 的 commit 才推进 generation。若它已被更高
   generation 取代，直接丢弃；它不得影响新 session。
4. native code 仅接受同时匹配 WebView instance、`NavigationId`、current generation 和
   loader attestation 的 commit。无法关联或失配的 commit 保持
   `Unauthenticated`；无法安全判定时撤销并 trusted reload。
5. 对已接受 commit，native code 创建新的 random secret 与 public session id，以
   `BrowserControlDocument` caller class 绑定当前 document generation。
6. 可信 bootstrap 经 native-owned、top-level-only injection path 接收 secret 与
   public session id；它们不嵌入公开 URL。
7. document 从匹配 `WebMessageContext` 发送
   `hello(secret, publicSessionId, protocolVersion)`。
8. Native code 仅 activate 匹配 pending session，并返回该 `BrowserDocument` 可见的
   schema。
9. 每条 document-to-native frame 携带 secret；每条 native-to-document frame
   携带 public session id。dispatch 与 posting 都拒绝非 active binding。

commit 校验依赖 native loader/source handle、navigation identity、WebView instance、
current generation 与 provenance，不比较 `current_url` 或 `pending_url`。仅已 commit 的
top-level frame 可以 activate 或使用 session；child frame 在 route/parameter parsing 前由
`WebMessageContext` 拒绝。复制 public session id 或与 parent 同源都不能提升 child frame。

### 外部 document

1. normalizer 接受且仅交付一次的 top-level document-replacing
   `NavigationEvent::Started` 立即撤销当前 session，防止旧 document 利用
   external-to-internal race。
2. HTTP/HTTPS 与其他非 control document 不获得 secret、schema 或
   `AuthenticatedCaller`。它们是 `Unauthenticated`，不是 bridge `StandardApp`，不能调用
   `AuthenticatedReadOnly` 或任何 bridge route。
3. native endpoint 在 JSON parsing 前检查固定最大 frame size；界限内也只先解析
   authentication envelope。没有 active binding 和匹配 `WebMessageContext` 的 frame，
   必须在 route lookup、parameter deserialization 或 session-owned allocation 前拒绝。
4. 外部 document 仍可使用 browser engine 的正常能力，但不能使用 LingXia control route。

因此 endpoint 对外部内容无效。backend 支持时，从外部 document 移除 endpoint 是 defense
in depth，不是正确性前提。rejected-frame log 必须限量或限速，避免 hostile content 造成
无界 log 或 allocation。

### 撤销与 outbound gate

撤销发生于最早由 normalizer 接受并单次交付的 top-level document-replacing
`NavigationEvent::Started`，而不是 raw backend callback 或 URL state 变化之后。
current-generation commit 的信任评估前也撤销残留 session；same-document
fragment 与 History API 变化不替换 document，故保留 session。下列事件均调用同一撤销
操作：

- tab close 或 discard；
- WebView destruction、replacement 或 detach/rebind；
- renderer-process termination 或 crash；
- browser/control runtime replacement；
- owning app-session 或 host shutdown；
- protocol failure 的显式 bridge reset；
- 无法证明 current top-level provenance/generation 的 commit 或恢复。

撤销必须原子地：标记 session revoked；detach outbound sink；删除 pending bootstrap、
handshake、schema state；在 route-specific parsing 前拒绝新 frame；取消或 detach
in-flight request、notification、stream、native View call、callback；关闭 channel 并拒绝
后续 `ch.data` / `ch.close`；拒绝 stale binding 的 `cancel`、`stateAck`、callback、reply；
并阻止 queued/late native output 投递至 successor document。

每个 asynchronous task 和 channel 捕获完整 binding 与 cancellation token。transport 在
序列化/排队时比较 active binding，在实际 UI-thread post 前必须再比较一次；不匹配即丢弃
frame/payload。延迟 side effect 执行前必须检查 cancellation token。JavaScript-side filtering
仅为 defense in depth。cancellation 不回滚已在授权期间完成的 side effect，但保证后续
frame 或 payload 不会进入另一 document。

“所有 frame family session binding”已经作为 bridge protocol major upgrade 落地。
`LegacyV2` 仅用于普通 app document；它的 hello 携带 `bridge_nonce`，其后的 frame 不具备
control document 所需的完整 binding。`RequiredV3` 用于 BrowserControlDocument：
hello/ready、request/reply、notification、stream event、channel、cancel、state
acknowledgement、native View call、callback 的双向 frame 都携带相应方向所需的 binding。
Rust codec、`packages/lingxia-bridge`、injected/bootstrap 与 generated client 必须作为同一协议
版本发布；native bootstrap 固定模式，不允许 document 协商降级。

V3 抗降级状态机由 native bootstrap 固定：BrowserControlDocument bootstrap 的 native-owned config
写死 `required_protocol = 3`（authoring/manifest 字段为 `controlProtocolVersion: 3`），不接受
payload 选择或降低版本。只有 protocolVersion 为 V3 的
hello，且在 native route/schema lookup 前完整匹配 current `WebMessageContext`、current
`DocumentSession` binding、document-to-native 的 secret 与对应 public session id，才建立
session 并返回 schema。V2、旧 JS、混合版本、payload 自称较低版本或任一 binding 不匹配，
一律 fail closed：不建 session、不返回 schema。secret 只走 bootstrap/document-to-native
认证，绝不在 native-to-document frame 回传。session 建立后所有 inbound frame 都必须在
route lookup 前验证 V3 与 active 完整 binding；native-to-document frame 只携带其方向所需的
public session id/binding 并在 post 前复核 active session。

cancellation 仅在其 `NavigationId` 是最新 pending attempt、且无更新 attempt 时可重新
bootstrap 当前 document。较早 attempt 的 `Superseded` terminal event 永不重新 activate。
re-bootstrap 必须重新取得当前 top-level provenance、document generation、loader attestation
与 `NavigationId` 跨层关联，再签发新的 secret、public session id；不得仅相信存储的旧
attestation。当前 history/BFCache restoration 路径将没有 fresh trusted start 的 internal
commit 保持为 unauthenticated，detach 旧 page lifecycle，并调度新的 host-issued trusted load；
新 load 重新取得 `NavigationId`、generation、secret 与 public session id。backend 无法安全
attest/re-bootstrap 时必须继续 fail closed。

新 window/tab 永不继承 opener 的 session、secret、outbound sink 或 pending navigation；
它拥有独立 WebView instance identity、registry、bootstrap 和 lifecycle。
`pending_url` 与 `current_url` 只属于 presentation/navigation state，绝不得作为
authorization input。

## browser WebUI 如何取得信任

可信 browser WebUI 由 host bootstrap 选定：

- built-in distribution 选择 LingXia embedded browser assets；或
- `browser.webui.path` / `browser.webui.package` 在 build/startup 时选择 host-owned
  replacement。

解析后的 asset source 由 native browser controller 持有。只有经该确切 loader commit 的
document 才能获得 `BrowserControlDocument` session。另行安装、即使 manifest、appid、file name
或 internal-looking URL 相同的 lxapp，仍只是 `StandardApp`。

custom WebUI 属于 host trusted computing base，必须在 `browser.webui` source config 与构建后
`lxapp.json` 同时声明 `controlProtocolVersion: 3`；缺失、V2 与未知未来版本均在
config/build/startup 链路 fail fast。SDK built-in catalog 由 native 固定为 V3，不要求用户重复
声明。artifact pinning/digest verification 属于
build/distribution，可提供 supply-chain defense，却不能替代 document-session
authentication。

## DisplayLanguageService

### 当前实现与最终 lease 合同

旧的 single static `OVERRIDE: Mutex<Option<String>>` 已删除。当前
`crates/lingxia-lxapp/src/lxapp/display_language.rs` 实现单一、revisioned
`DisplayLanguageService`：持久化 preference 与内存 publication 在同一线性化顺序内执行；
订阅注册与 initial snapshot 原子；并发 transition 按 revision 发布；state 与 effective
分别 exact-dedup。

Runner override 由 `DisplayLanguageSessionOwner` 标识。当前最终语义是 lease，而不是启动参数
seed：`crates/lingxia-control-runtime/src/bridge.rs` 仅在具体 dev websocket 完成 hello 后取得
`RunnerDisplayLanguageLease`，connection loop 返回、正常断开、peer crash/stale timeout、setup
失败或重连前都会 Drop lease。新 connection 安装新 owner 即 takeover；旧 owner 的迟到 clear
只做 no-op。host teardown 另有 `clear_active_display_language_session_override` 兜底。永久重连
循环的断线间隔内没有 override；override 不会跨 connection 保留。

public 合同已经从封闭 `DisplayLanguage`/`DisplayLanguageSetting` 与旧 setter 迁到
`DisplayLanguagePreference = 'auto' | LanguageTag`、`DisplayLanguageState` 与新
get/set/watch API。`LanguageTag` 会 parse、validate、canonicalize 任意合法 BCP-47 tag；
`auto` 只允许作为 preference sentinel，显式构造 `LanguageTag("auto")` 会失败。持久化仍以
`None` 表示 `Auto`、canonical tag 表示 explicit preference，旧 `auto|en-US|zh-CN` 输入只是
新输入集合的子集。

显示语言是宿主范围的语义服务：

```text
DisplayLanguagePreference = Auto | LanguageTag
DisplayLanguageEffectiveSource = System | Preference | SessionOverride

DisplayLanguageState {
  preference: DisplayLanguagePreference,
  effective: LanguageTag,
  effective_source: DisplayLanguageEffectiveSource,
}

DisplayLanguageService
  |-- getState() -> DisplayLanguageState
  |-- setPreference(DisplayLanguagePreference)
  |-- observeState() -> DisplayLanguageState changes
  `-- observeEffective() -> effective LanguageTag changes
```

`preference`、`effective` 与 `effective_source` 各自必要：`effective: en-US` 无法区分
“跟随 system，当前为 en-US”与“始终 en-US”。`effective_source` 表示最高优先级的胜出输入；
即使 `SessionOverride(Auto)` 的结果来自 system，其 source 仍是 `SessionOverride`。

service 按以下顺序解析：

1. native Runner 安装的 optional session-only override；
2. persisted `DisplayLanguagePreference`；
3. current system language。

```text
Some(SessionOverride(LanguageTag)) -> tag，source 为 SessionOverride
Some(SessionOverride(Auto))        -> system tag，source 为 SessionOverride
None + preference LanguageTag      -> tag，source 为 Preference
None + preference Auto             -> system tag，source 为 System
```

`lingxia dev --display-language` 仅建立 session-only Runner override，从不修改已持久化的
preference。仅 native Runner websocket lease 可创建、移除它；没有 JavaScript override
mutation API。安装或移除 override 即使使 effective tag 保持不变，只要
`DisplayLanguageState` 改变也只发出一次 state event，且不发出 effective event。

Runner override 的清理与 browser `DocumentSession` 完全无关：它既不等同于、也不触发
browser document 的撤销。override active 时更新 preference 会产生 state event，却可不改变
effective，直到 session 结束。system language 变化仅在当前解析路径自动取值时改变
effective。

`setPreference` 必须先 validation、canonicalize 并成功 persistence，之后才原子更新内存并
发事件；任一步失败则 memory、persistence、事件均不改变。service 还负责 native
shell/chrome refresh、向 live lxapp 传播、language-tag normalization。各 UI surface 仍可将
canonical host tag 映射到自己的 supported catalog/fallback language。

`observeState` 在 `DisplayLanguageState` 任意字段实际变化时恰发一次；
`observeEffective` 仅在 canonical `effective` tag 实际变化时恰发一次。由此 native chrome
和 ordinary lxapp 不会从同一次 effective update 获得重复通知。

semantic ownership 调整不要求立即迁移 file format。`lingxia-settings::Settings` 可继续
在一个 JSON file 中物理保存 display language、download directory 与 per-lxapp appearance，
但各 domain 分别由 service 拥有；后续 storage migration 独立于此 API boundary。browser
history、bookmarks、downloads、privacy、proxy 仍属于 browser service；Settings UI 可呈现
多个 service，但不拥有它们。

## API 适配层

系统只有一个 service 与一套 authorization policy，通过 adapter 覆盖既有 host shape；不
创建 `control.app.*` 或其他 universal JavaScript namespace。

### Rust

trusted native host code 使用同一 facade：

```rust
lingxia::app::display_language(); // 既有 effective-language getter
lingxia::app::display_language_state();
lingxia::app::set_display_language_preference(...);
lingxia::app::on_display_language_change(...); // 仅 effective tag
lingxia::app::on_display_language_state_change(...);
```

effective observer 仅在 `effective` 变化时运行；state observer 在
`DisplayLanguageState` 任一字段变化时运行。

### 启用 Logic 的 ControlApp lxapp

ordinary lxapp 继续从 base info 与
`onDisplayLanguageChange((effective: string) => void)` 获取 effective language，且
signature/可用性不随 caller class 改变。由 launch plan/native bootstrap 赋予
`ControlApp` 的 Logic context 额外获得：

```ts
lx.app.getDisplayLanguageState()
lx.app.setDisplayLanguagePreference(preference)
lx.app.onDisplayLanguageStateChange(listener)
```

新的 observer 发出 `DisplayLanguageState`，而非 string。get/write/state-observer 都检查
native-assigned caller class，不能比较 caller appid 与 `homeAppId`。

### 禁用 Logic 的 ControlApp View 与 browser WebUI

可信 `logic: false` View 通过既有 native-client/bridge transport 适配同一 service。其
route 标记为 `ControlAppOnly` 或 `ControlOnly`，在 `StandardApp` schema 中隐藏，并在 dispatch
再次检查。其 effective-language bootstrap/event 独立于 control route 保持可用。

browser WebUI 在 `BrowserControlDocument` `DocumentSession` 下使用同一 transport。host 若从
browser Settings 暴露少数共享的 host-wide language state/mutation，应显式使用 `ControlOnly`；
effective-language unary read `app.getDisplayLanguage` 使用 `AuthenticatedReadOnly`；
持久 stream `app.watchDisplayLanguage`、完整 state read/write/watch 都使用 `ControlOnly`。
ordinary authenticated lxapp 的 reactivity 走 native 注入的
`DisplayLanguageChange`/`__lingxiaApplyDisplayLanguage`，不通过 control stream。browser-only setting 使用
`BrowserControlOnly`。`ControlApp` 不因此直接取得 browser profile 权限，只能发起可信的
browser-settings navigation。没有 API 会因 caller 声称 appid 是 `app.lingxia.browser` 而放行。

framework-owned WebUI control route 从 public `@lingxia/native` JavaScript client 移除。
host-defined/native-extension `#[lingxia::native]` route 仍独立存在，且每条都有自己的
`RouteAudience`。

legacy `settings.getLanguage`、`settings.setLanguage`、`settings.watchLanguage` 已在逐项迁移
consumer 后删除；没有以“看似无 consumer”为依据直接删除。旧 routes 返回 persisted value，
可能不同于 Runner effective value，因此历史迁移按 route 核对了全部调用点：

| 旧入口/consumer | 已落地迁移语义 | 删除 gate |
| --- | --- | --- |
| `settings.getLanguage` / terminal i18n、browser WebUI i18n、browser settings selector 初始化 | 展示语言改为 effective read；编辑偏好读 preference state | 三处调用点均迁移并覆盖 Runner 遮蔽 |
| `settings.watchLanguage` / terminal i18n、browser WebUI i18n | effective observe | 两处调用点均迁移且有 reactivity test |
| `settings.setLanguage` / browser settings selector | `setDisplayLanguagePreference(Auto | canonical LanguageTag)` | selector 与 generated types 采用新合同 |

旧 literals 是新合同可接受输入的子集；除非 release plan 明示短期 adapter，不保留旧 API/type。

当前 registry 没有 route alias 机制，也未为旧 `settings.*`、旧 setter 或旧 type name 注册
compatibility alias。route 名是 exact match；迁移后的调用方直接使用 `app.getDisplayLanguage`、
`app.watchDisplayLanguage`、`app.getDisplayLanguageState`、
`app.setDisplayLanguagePreference` 与 `app.watchDisplayLanguageState`。因此这是明确的 breaking
migration，不能把“旧名字恰好无人调用”当作兼容层。

## 桌面端 Settings 归属

platform shell 是 Settings menu/sidebar entry 的唯一 writer，因此 routing 在 bootstrap
固定，不能由 page runtime 注册。

```text
HostBootstrap {
  settings_destination: Option<SettingsDestination>,
}

SettingsDestination =
  | ControlAppPage { app_id, page, query? }
  | BrowserControlPage { route, query? }
  | NativeAction { action_id }
```

`SettingsDestination` 是纯静态 descriptor，不含 runtime、`DocumentSession`、app session、
endpoint、callback、closure、secret 或 caller class。它可来自 `lingxia.yaml`、generated host
bootstrap 或 runtime startup 前的 native embedding API；多处输入必须收敛到一个值，冲突
即 configuration error。

startup 只利用 launch plan、static route registry、预声明 native action 验证 descriptor
的存在性、唯一性与 audience compatibility，不要求创建 runtime/session：

- `ControlAppPage` 的 app/page/可选 query 必须是 launch plan 中静态存在的 control
  page；其实际 authority 仍由 bootstrap 后的 `ControlApp` app session 决定。
- `BrowserControlPage` 的 route/可选 query 必须是 host 选定 WebUI 静态支持的 page/asset；
  bridge route 若参与该目标，只能是 `BrowserControlOnly` 或 `ControlOnly`。
- host target 所引用的 bridge route 只能是 `ControlAppOnly` 或 `ControlOnly`。
- `NativeAction` 必须是预声明 static action。

descriptor 静态有效时 shell 即显示 Settings entry。点击时，shell 仅依据 descriptor 与
launch plan fresh-resolve 当前目标：

- 对 `ControlAppPage`，若目标 runtime 尚未创建，则按 launch plan 创建；若已存在，则
  聚焦它并导航到 descriptor 指定 page。只有当前 runtime 获得 bootstrap 分配的
  `ControlApp` 后，页面中的 control route 才可用。
- 对 `BrowserControlPage`，shell 创建或呈现 browser surface，并发起 trusted internal
  navigation。只有该次 navigation 的新 commit 建立新的 `BrowserControlDocument`
  `DocumentSession` 后，WebUI 才可用。
- 对 `NativeAction`，shell 按 `action_id` 重新解析当前注册的 action 后执行。

无法建立当前合法目标时必须报告错误，不复用旧 runtime/document/action，不发送任何 bridge
frame。destination 只选择导航/动作目标，绝不授予 caller class 或其他 authority。page 不得在运行时
替换它；Sidebar action 可触发已解析 destination，但不是 discovery/authentication 依据。

```text
destination 存在且静态有效 -> 显示 Settings entry
destination 缺失           -> 不显示 Settings entry
destination 无效或冲突     -> startup 以 configuration error 失败
```

这也是当前 API 的精确 `None` 语义：`static_settings_destination()` 返回 `None`，macOS/Windows
shell 不生成 Settings menu/sidebar item；若 native embedding code 仍显式调用
`resolve_settings_destination()`，返回 `SettingsDestinationResolveError::NotConfigured`，不会
聚焦旧 runtime、打开默认 URL 或执行 fallback action。runtime sidebar 即使伪造 settings id/label
也不能生成 bootstrap-owned static entry。

browser host 可以指向可信 browser WebUI；browser 专属 menu 可另行提供 browser
setting，但不会因此成为 host-wide Settings destination。

browser-local `lingxia://settings` 可保留，但必须重命名/归类为 browser-local navigation，
仅由相应 browser control/menu 调用，绝不能充当 host Settings destination。

### 旧 Settings 入口的第四阶段迁移

| 现状入口 | 目标 | 删除/替换条件 |
| --- | --- | --- |
| macOS/Windows shell hardcoded Settings handler | 改走 `resolve_settings_destination` | 无 destination 不显示也不调用；有 destination fresh-resolve |
| generic runtime sidebar action | 不注册、不自带 host Settings target；只触发已解析 descriptor | shell 为唯一 writer，runtime 无 discovery caller class |
| Logic/FFI `BuiltinShellPage`、`BuiltinBrowserPage::Settings`、`open_builtin_browser_page` 的 host-wide Settings 分支 | 删除 host-wide 分支 | 仅保留 browser-local navigation，且限制为 browser control/menu |

上述 host-wide 入口目前已删除/收窄。browser-local clear-site-data 与 settings menu 仍可导航
browser 自己的内部页面，但不经过 host Settings resolver，也不能替代
`SettingsDestination::BrowserControlPage` 的 trusted reload。

## Host 形态行为

| Host 形态 | Caller class | Settings 行为 |
| --- | --- | --- |
| 启用 Logic 的 home lxapp | bootstrap 分配 `ControlApp` | 可选 `ControlAppPage` |
| 禁用 Logic 的 control lxapp | bootstrap 分配 `ControlApp` | 可选，经既有 View bridge 打开页面 |
| Browser main、无 home lxapp | browser WebUI 每 document 获得 `BrowserControlDocument` | 可选 `BrowserControlPage`；外部 document 为 `Unauthenticated` |
| Terminal main、无 home lxapp | 除非另有配置，否则仅 native control | 可选 `NativeAction`，否则无 entry |
| URL/web main | 外部 document 为 `Unauthenticated` | 除非另有静态 native/control destination，否则无 Settings |
| Pure native host | native code 可信 | 可选 `NativeAction` |
| Ordinary/runtime lxapp | `StandardApp` | 仅可 read/observe effective language |

default surface type 与 app id 均不蕴含 authority。

## 必须保持的 invariant

1. native bootstrap 是 `CallerClass` 的唯一来源；`AppScope` 仅由 native 或用户确认
   产生且不可由 payload 覆写。
2. app id、URL、scheme、bundle-source variant、manifest、payload 都不授予 caller class 或资源权。
3. 每条 registry route 都有 `EffectiveRoutePolicy`；schema/dispatch 共用 policy，缺少
   metadata 在 registration 即失败。
4. `AppSessionOnly` 只允许进入 route；handler 必须对资源执行 `AppScope` 授权。
5. `AuthenticatedReadOnly` 仅对 authenticated caller 开放，且无副作用、无持久订阅/租约/资源分配、
   无跨 app 泄露。
6. browser frame 绑定不可复用的完整 `DocumentSession` binding，其中含 WebView instance、
   `NavigationId`、loader attestation、document generation 与 secret。
7. session registry linearization point 串行化 create/authorize/revoke/outbound；实际
   UI-thread post 前再比较完整 binding。
8. normalizer 单次交付的 top-level document-replacing `NavigationEvent::Started` 撤销旧
   document；被更高 generation 取代的 stale event 直接丢弃，绝不撤销新 session。
9. 外部 document 不获得 secret、`AuthenticatedCaller`、schema 或 `AuthenticatedReadOnly`。
10. child frame、opener、stored attestation 均不能 activate/re-bootstrap session；新 window/tab
    不继承任何 session state。
11. 所有撤销触发点均清理 inbound/outbound state；延迟 side effect 前检查 cancellation token。
12. BFCache/history restoration 永不复用已撤销 session，且必须重新取得 current provenance、
    generation、loader attestation。
13. 所有 display-language write 经 `DisplayLanguageService` 原子持久化；state/effective
    事件分别按其语义去重。
14. shell 有零或一个静态 `SettingsDestination`；它不持有 live object，也不授予 authority。

## 破坏性变更

实现可以移除或改变：

- public browser WebUI `settings.getLanguage`、`settings.setLanguage`、
  `settings.watchLanguage` route，但必须先完成列出的 consumer migration；
- `DisplayLanguage`/`DisplayLanguageSetting` 封闭 catalog、旧 setter、Rust facade 与
  generated Logic client 的旧 API，替换为 `Auto | canonical BCP-47 LanguageTag` preference/state contract；
- 仅依赖 appid 的 `ensure_home_lxapp`、`require_home_caller` 与 browser-shell
  `require_builtin_browser` authorization check，且仅在逐 route policy 迁移后；
- 从 `BuiltinAssets`、`Synthetic`、`DevPath` 或 internal URL string 推导的信任；
- `pending_url` / `current_url` authorization check；
- public generated native client 中的 browser-private handler；
- 未携带完整 document-session binding 的 bridge frame；
- runtime Settings-provider registration；
- 缺少有效静态 bootstrap destination 的 desktop Settings entry；
- 缺少 `EffectiveRoutePolicy` metadata 的 direct registration paths、generated registration
  paths，或未来引入的其他 registration path；
- 持有 runtime、session、endpoint、callback 或 secret 的 Settings destination；以及
- 旧的 host-wide Settings 分支。

custom browser WebUI 必须采用协商后的 V3 control protocol；除非 release plan 明确提供
短暂 transition window，否则不要求 compatibility adapter。

DisplayLanguage 的已落地 migration 如下，旧名没有 alias：

| 旧合同 | 新合同 |
| --- | --- |
| `DisplayLanguage` / `DisplayLanguageSetting` 封闭 enum | `DisplayLanguagePreference` (`'auto' | LanguageTag`) 与 `DisplayLanguageState` |
| Logic 旧 setter | `lx.app.setDisplayLanguagePreference(preference)` |
| 只读取 persisted setting | `lx.app.getDisplayLanguageState()` 区分 `preference`、`effective`、`effectiveSource` |
| 旧 state/string observer 混用 | `onDisplayLanguageChange` 仅 effective；`onDisplayLanguageStateChange` 仅完整 state |
| Rust 旧 setter/facade | `lingxia::app::{display_language_state,set_display_language_preference,...}` |
| browser `settings.*Language` routes | `app.get/watchDisplayLanguage*` 与 `app.setDisplayLanguagePreference`，按 fixed audience 授权 |

持久化文件格式没有被强制拆分：`Auto` 仍写作缺省/`None`，explicit preference 写 canonical
BCP-47 tag；这是 storage continuity，不是 public API alias。

## 实施阶段

### 当前主线落地模块清单

以下是当前实现 owner，不是早期设计中的建议文件名：

| 领域 | 已落地模块 |
| --- | --- |
| caller/audience 与 route inventory | `crates/lingxia-lxapp/src/host.rs`、`bridge.rs`、`crates/lingxia-native-macros`、`crates/lingxia-logic/src/authorization.rs` |
| app/resource authority | `crates/lingxia-lxapp/src/host.rs`、`page.rs`、`terminal_automation.rs`，以及 Logic fs/media/process per-call gates |
| document identity 与平台 provenance | `crates/lingxia-webview/src/events/normalizer.rs`、`webview.rs`、`apple/webview.rs`、`android/{ffi.rs,webview.rs,java/*}`、`windows/document.rs` |
| BrowserControl session/ingress | `crates/lingxia-browser/src/document_session.rs`、`inbound.rs`、`webview.rs`、`internal_pages.rs` |
| RequiredV3 codec/bootstrap | `crates/lingxia-lxapp/src/bridge/{protocol.rs}`、`bridge.rs`、`control_document_bootstrap.rs`、`lxapp/content.rs`、`packages/lingxia-bridge/src/protocol-v3.ts`、`bridge.ts` |
| DisplayLanguage service/lease | `crates/lingxia-lxapp/src/lxapp/display_language.rs`、`crates/lingxia-control-runtime/src/bridge.rs`、`crates/lingxia/src/{app.rs,display_language_host.rs}`、`crates/lingxia-logic/src/app.rs`、`packages/lingxia-types` |
| Settings static destination | `crates/lingxia-app-context/src/lib.rs`、`tools/lingxia-cli/src/config.rs`、`crates/lingxia/src/{settings_target.rs,settings_destination.rs,bootstrap.rs}`、Apple `LxAppStaticSettingsSource.swift`、Windows `static_settings.rs`/shell runtime |

当前仍需后续实现并更新本文状态的是 HarmonyOS RequiredV3 provenance/outbound；完成前保持
fail closed。BFCache/history restoration 已通过 fresh trusted reload 收口，Apple renderer
termination 也会在上层 callback 前清除 committed generation。

### 第一阶段：显式 route policy 与资源边界

落地状态：主线已完成；下列步骤保留为审计清单。

1. 将 `CallerClass`、native-derived `AppScope` 加入 host bridge context，并将
   `RouteAudience` 加入 registration。
2. 集中实现 `authorize(caller, audience)`，供 schema/dispatch 共用；handler 增加
   `AppScope` 资源授权。
3. 普通 macro route 编译期默认 `AppSessionOnly`；framework control 专用 API 与手写
   `HostRegistration` 强制显式 audience。
4. 为 unary、stream、channel、macro-generated 与 direct registration paths
   （`register_host_route`、`HostRegistration`、`ChannelRegistration` 等）形成
   `EffectiveRoutePolicy`；若未来引入其他 registration path，同样要求 policy；缺 metadata
   registration fail，并加入全量 route inventory test。
5. 依 guard inventory 逐 route 迁到 ControlApp/BrowserControlDocument session policy，才移除
   appid/source/URL equality authorization check。

主要代码落点/owner：`lingxia-lxapp` host/bridge/protocol、`lingxia-native-macros`、Logic
app/surface/browser-shell guards。退出条件：全 registry inventory 通过；schema/direct dispatch
对“同 appid、未获 caller class”均拒绝；无 control route 可由 appid/source/URL 授权。

### 第二阶段：将 browser authority 绑定到 document

落地状态：core registry/RequiredV3、restoration trusted reload 与 Apple、Windows、Android
API 23+ 已完成；HarmonyOS 未完成前 fail closed。

1. 用现有 `NavigationId`、`NavigationProgress`、`Superseded` normalizer 建立
   `PendingNavigation`，新增 loader-to-commit 跨层关联、random secret、loader attestation、
   document generation 与每 WebView session registry linearization point。
2. 在 normalizer 接受且单次交付的 top-level document-replacing
   `NavigationEvent::Started` 撤销并取消所有 session-owned work；redirect repeated start
   coalesce，不重复 revoke/rekey；准确区分 stale event 与 current-generation commit。
3. 只有 WebView instance、`NavigationId`、current generation、attestation 与
   top-level provenance 全部验证后才签发 `BrowserControlDocument`。
4. 绑定所有 inbound/outbound frame family；排队与 UI-thread post 前均检查完整 binding，
   延迟 side effect 前检查 cancellation token。
5. provenance 无法正面证明时 fail closed 并 trusted reload；new window/tab 建立独立
   identity/bootstrap。
6. 从全部 authorization path 移除 `pending_url` 与 `current_url`。

主要代码落点/owner：`lingxia-webview` events/normalizer/platform adapters、`lingxia-browser`
webview/tabs、`lingxia-lxapp` page/bridge、`packages/lingxia-bridge`。退出条件：每个启用
backend 证明 top-level/generation/loader association；Android 21/22 等无法证明的平台持续
fail closed 且有 metrics；Rust 与 JS V3 互操作测试全部通过。

### 第三阶段：重写/替换 display language single-slot

落地状态：主线已完成。下列步骤描述现有实现，不再表示仍存在 `OVERRIDE` 待迁移。

1. 引入 `DisplayLanguagePreference`、`DisplayLanguageEffectiveSource`、
   `DisplayLanguageState`。
2. 用 service 替换 `display_language.rs` 的 `OVERRIDE` single slot，而不是抽取现成行为；
   实现 persistence、native Runner session owner/持续遮蔽/cleanup、effective resolution、refresh、broadcast。
3. 使 `setPreference` 验证、canonicalize、持久化成功后才原子发布 state；实现
   state/effective 两种去重事件。
4. 让 Rust、bootstrap-assigned Logic、logic-disabled View 使用同一 service，并移除
   browser-owned implementation/public framework-native export；迁移表中全部调用点后才删 route。

主要代码落点/owner：`display_language.rs`、`lingxia` service settings、bootstrap、browser-shell
settings、terminal/browser consumers/generated types。退出条件：当前/目标合同迁移测试通过；
旧 consumer 全部用正确语义；没有未计划的旧 API/type；Runner lifecycle 无 stale override。

### 第四阶段：固定 Settings destination

落地状态：config/bootstrap/resolver 与 macOS、Windows static shell source 已完成；`None` 不
生成入口，显式 resolver 返回 `NotConfigured`。

1. 在 host bootstrap 添加一个 optional、纯数据 `SettingsDestination`。
2. 使用 launch plan、static registry、predeclared action 验证唯一性、存在性、页面/asset
   存在性与 audience compatibility，而不创建 runtime/session。
3. 点击 shell entry 时按 descriptor 与 launch plan fresh-resolve：按需创建或聚焦 ControlApp
   runtime，或发起新的 trusted browser navigation，或按 key 重新解析 native action。
4. 替换 macOS/Windows hardcoded handler，限制 generic runtime sidebar action，删除
   Logic/FFI host-wide Settings 分支；无 destination 时所有旧入口不可显示/调用。

主要代码落点/owner：CLI config → app-context → bootstrap resolver → Windows/macOS adapters
与旧入口删除。`lingxia-shell` 不负责 browser/host behavior。退出条件：三类旧入口均不存在
host-wide bypass；无 destination E2E 无显示/调用；每种 descriptor 的 fresh-resolve E2E 通过。

## 验证矩阵

### 授权与注册表

- ordinary lxapp 即使 appid 为 `app.lingxia.browser`，caller class 仍为 `StandardApp`；
- `BuiltinAssets`、`Synthetic`、`DevPath`、internal-looking URL 不授予 authority；
- `StandardApp` handshake 不含 control route，direct guessed dispatch 被拒绝；
- `AuthenticatedReadOnly` 对 authenticated caller 可用，但 external document 为
  `Unauthenticated` 而被拒绝；并测试其无副作用、无租约/订阅/资源分配、无跨 app 数据；
- 覆盖 3 个 `CallerClass` 与 5 个 `RouteAudience` 的 schema/dispatch 组合；
- macro 默认 `AppSessionOnly`、framework control 显式 audience、手写 registration 显式
  audience 均有 compile/registration test；
- route inventory 覆盖 unary、stream、channel、macro-generated、direct registration paths
  （`register_host_route`、`HostRegistration`、`ChannelRegistration` 等）；未来引入的
  registration path 同样覆盖，缺 metadata 在 registration 失败；
- payload 伪造 appid/resource id 不得绕过 `AppScope` 资源授权。

### 导航、session 与撤销

- external -> internal、internal -> external、redirect chain、canceled navigation；
- 只有 normalizer 接受且对每个 `NavigationId` 单次交付的
  `NavigationEvent::Started` 可以 revoke/rekey；同一 native navigation id 的 redirect repeated
  start 必须 coalesce，raw backend callback 不得额外触发 lifecycle；
- request in flight 时快速重复 navigation、reload、tab discard、crash/session restoration、
  WebView reuse；
- back/forward、BFCache restoration 与 re-bootstrap；
- stale hello/request/notify/cancel/state ack/stream/channel/callback/reply frame；
- old document 在 `NavigationEvent::Started` 后的 race、child-frame 激活尝试；
- 每个 backend 的 new-window/target-blank flow，确认不继承 opener session/secret/sink；
- WebView instance、`NavigationId`、attestation、generation、top-level provenance
  任一不匹配时 fail closed；stale event 不得撤销较新 session；
- `NavigationId` 是不复用的进程级 attempt identity，generation 仅在证实 per-WebView
  top-level replacement commit 后递增；commit 不能跨层关联时不得以 counter 伪造通过；
- Apple `frameInfo`、Android 23+ per-document port/generation、Windows per-WebView
  FIFO/context preservation 各有正向 backend test；HarmonyOS 在可信 backend 落地前断言
  `Unsupported`/fail closed；Android 21/22 断言 privileged UI 不可用并记录拒绝/降级 metrics；
- Rust protocol、`bridge.ts`、injected/bootstrap/generated client 的 V3 version negotiation
  与全 frame-family binding 双端测试，拒绝任一端单独启用；`required_protocol = 3` 下 V2、旧 JS、
  混合版本、payload 降级、首帧 secret/public-session/context 不匹配均不建 session、不返 schema；
- 不能证明 current commit/restoration 时撤销并 trusted reload；
- 每个撤销触发点清理 session-owned state，outbound gate 丢弃 stale output，延迟副作用
  尊重 cancellation token。

### 显示语言

- 旧 `auto|en-US|zh-CN` 输入是新 `Auto | LanguageTag` 的子集；任意 canonical BCP-47 tag 的
  validation、type、persistence、Rust facade/generated Logic API 都覆盖；
- `Auto` 与 explicit language 在 effective 相同时仍可区分；
- `SessionOverride(Auto)` 的 `effective_source` 仍是 `SessionOverride`；
- system language change 更新自动路径而不改 explicit preference；
- Runner override 不持久化、无 JS mutation API，且在正常结束/crash/takeover/teardown 清除；
- 安装或移除 effective tag 相同的 Runner override 时，恰发一次 state event，零次
  effective event，且不触发 browser `DocumentSession` revoke；
- preference 被 override 遮蔽时恰有一次 state event，无 effective event；
- canonical effective 改变时恰有一次 effective event，native chrome 与每个 live lxapp
  各观察一次；
- persistence/validation 失败时 memory、持久化、事件原子不变；
- `StandardApp` 只能 read/observe effective language；Rust、Logic、logic-disabled View adapter
  操作同一 state。
- terminal-settings public/i18n.js、browser WebUI i18n、browser settings selector 分别迁移到
  表中 effective/preference 语义并通过测试后，legacy routes 才可删除。

### Settings 目标

- 无 destination 即无 Settings entry，macOS/Windows handler、generic sidebar、Logic/FFI
  旧入口也均不可显示或调用；
- 启动时即使尚无 runtime/session，静态有效 descriptor 仍通过验证并显示 Settings entry；
- 点击 `ControlAppPage` 会创建或聚焦 runtime 并导航；点击 `BrowserControlPage` 发起新的
  trusted navigation，只有新 commit 建立 session 后才可用；
- conflicting、不存在、页面/asset 缺失、audience/type 不兼容的 descriptor 确定性失败；
- startup 验证不需要 runtime/session；点击时 fresh-resolve，失败不复用旧对象、不发 frame；
- descriptor 不含 live runtime/session/endpoint/callback/closure/secret/caller class，且不能提升
  caller authority。
- browser-local navigation 不能充当 host destination；三个旧入口移除/替换与无 destination
  路径均有 E2E。

## 否决的替代方案

### 每个 tab 分离可信与外部 renderer

这会使 LingXia 负责跨两个 WebView 的 composite history/restoration。在 document-session
revocation 尚未正确实现前，其安全收益不足以抵偿复制 browser navigation semantics 的成本。

### 用 app id 加 bundle source 授权

它们识别 app container 与 asset origin，不识别当前在 browser WebView 中执行的 document。
两者可观察，且不能证明持有 live native-issued session 或资源授权。

### 用 URL 或 pending-navigation state 授权

external-to-internal navigation 期间，native state 可能已描述新 target，旧 external
document 却仍在执行。URL state 适合 UI，不适合 authentication。

### 通用 principal 与细粒度 capability set

该方案解决更大的 delegation 问题，且与现有术语冲突。当前边界只需 3 个固定 authority
caller class 与 5 个 route audience，invalid state 更少。

### 动态 Settings-provider 注册

shell 只有一个 Settings affordance，因而需要确定性 writer。runtime registration 会引入
ordering、replacement、teardown、conflict rule，却不改善 host model。

### 强制内置 Settings app

browser、terminal、URL、pure-native main 的 host 可以有意没有 control lxapp。它们应遵循
default 并省略 Settings，而不应为 AppService 付出成本或展示不可用 screen。

### 新增仅限 control 的 JavaScript namespace

Rust、Logic、View 已有 transport adapter。新增 `control.app.*` 会增加公开 surface，而
authorization 仍必须由 native 强制；复用既有 adapter 可保持单一 service contract。

## 后果

bridge protocol 更严格：每条 message path 必须验证完整 document identity，navigation
lifecycle 在各 backend 上必须一致。实现工作集中，但保留 native browser semantics，并使
stale-document failure 可测试。

framework id 保持稳定而不会成为 credential。browser-private framework route 从 universal
lxapp API 消失；普通 host-defined/native-extension route 仍因编译期 `AppSessionOnly` 默认值保持简洁。host
显式选择是否存在 Settings entry 及其目标，点击时重新获得当前 authority。没有 Settings
UI、home lxapp 或 AppService 时，display language 仍可用。
