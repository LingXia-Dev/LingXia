// Generated from contracts/view/schema.json; do not edit.
// The frame contract the Harmony SDK decodes: snapshot = FrameSnapshot, semanticNode = SemanticNode, automationRequest = AutomationRequest, automationElement = AutomationElement, automationQueryAll = AutomationQueryAll, automationTree = AutomationTree, automationFrame = AutomationFrame, automationCall = AutomationCall, automationResult = AutomationResult, inputRequest = InputRequest, inputReply = InputReply, copySelection = CopyReply, semanticRoots = SemanticRoots, textMenu = TextMenu.

/** `ScrollAxis` values. */
export class GpuFrameScrollAxisValues {
  static readonly HORIZONTAL: string = "horizontal";
  static readonly VERTICAL: string = "vertical";
}

/** `Keyboard` values. */
export class GpuFrameKeyboardValues {
  static readonly TEXT: string = "text";
  static readonly NUMBER: string = "number";
  static readonly EMAIL: string = "email";
  static readonly PHONE: string = "phone";
  static readonly SEARCH: string = "search";
}

/** `Capitalize` values. */
export class GpuFrameCapitalizeValues {
  static readonly NONE: string = "none";
  static readonly SENTENCES: string = "sentences";
  static readonly WORDS: string = "words";
  static readonly CHARACTERS: string = "characters";
}

/** `ReturnKey` values. */
export class GpuFrameReturnKeyValues {
  static readonly NEWLINE: string = "newline";
  static readonly DONE: string = "done";
  static readonly GO: string = "go";
  static readonly NEXT: string = "next";
  static readonly PREVIOUS: string = "previous";
  static readonly SEARCH: string = "search";
  static readonly SEND: string = "send";
}

/** `PickerKind` values. */
export class GpuFramePickerKindValues {
  static readonly DATE: string = "date";
  static readonly TIME: string = "time";
}

/** `MenuAction` values. */
export class GpuFrameMenuActionValues {
  static readonly CUT: string = "cut";
  static readonly COPY: string = "copy";
  static readonly PASTE: string = "paste";
  static readonly SELECT_ALL: string = "select_all";
  static readonly UNDO: string = "undo";
  static readonly REDO: string = "redo";
}

/** `SemanticAction` variants, by its `kind` key. */
export class GpuFrameSemanticActionKinds {
  static readonly CLICK: string = "click";
  static readonly SCROLL_FORWARD: string = "scroll-forward";
  static readonly SCROLL_BACKWARD: string = "scroll-backward";
  static readonly SET_TEXT: string = "set-text";
  static readonly SET_SELECTION: string = "set-selection";
  static readonly SET_PROGRESS: string = "set-progress";
  static readonly CLEAR_SELECTION: string = "clear-selection";
  static readonly SELECT_ITEM: string = "select-item";
  static readonly SHOW_ON_SCREEN: string = "show-on-screen";
  static readonly COPY: string = "copy";
  static readonly CUT: string = "cut";
  static readonly PASTE: string = "paste";
  static readonly CUSTOM: string = "custom";
  static readonly DISMISS: string = "dismiss";
}

/** `AutomationRequest` variants, by its `op` key. */
export class GpuFrameAutomationRequestKinds {
  static readonly QUERY: string = "query";
  static readonly ACT: string = "act";
  static readonly TREE: string = "tree";
  static readonly FRAME: string = "frame";
  static readonly HUD: string = "hud";
  static readonly ACTION: string = "action";
  static readonly DATA: string = "data";
  static readonly RESULT: string = "result";
}

/** `InputRequest` variants, by its `op` key. */
export class GpuFrameInputRequestKinds {
  static readonly DRAG_HIT_TEST: string = "drag_hit_test";
  static readonly HIT_TEST: string = "hit_test";
  static readonly FOCUS: string = "focus";
  static readonly QUERY: string = "query";
  static readonly BLUR: string = "blur";
  static readonly EDIT: string = "edit";
}

/** `InputEdit` variants, by its `op` key. */
export class GpuFrameInputEditKinds {
  static readonly SELECTION: string = "selection";
  static readonly COMPOSITION: string = "composition";
  static readonly REPLACE: string = "replace";
  static readonly FINISH_COMPOSITION: string = "finish_composition";
  static readonly UNDO: string = "undo";
  static readonly REDO: string = "redo";
  static readonly DELETE_SURROUNDING: string = "delete_surrounding";
}

/** `AutomationAction` variants, by its `kind` key. */
export class GpuFrameAutomationActionKinds {
  static readonly CLICK: string = "click";
  static readonly FILL: string = "fill";
  static readonly TYPE: string = "type";
  static readonly SCROLL: string = "scroll";
  static readonly PRESS: string = "press";
  static readonly SELECT: string = "select";
  static readonly DBLCLICK: string = "dblclick";
  static readonly ACTION: string = "action";
}

/** `TextMenuTarget` variants, by its `kind` key. */
export class GpuFrameTextMenuTargetKinds {
  static readonly TEXT: string = "text";
  static readonly EDITOR: string = "editor";
}

/** `FrameSnapshot` in the frame contract. */
export interface GpuFrameFrameSnapshot {
  generation: number;
  nodes: GpuFrameNode[];
  images: GpuFrameImageSource[];
  metrics: GpuFrameFrameMetrics;
  feedback: GpuFrameFeedbackStyle;
}

/** `ImageSource` in the frame contract. */
export interface GpuFrameImageSource {
  source: string;
  retry_key: string;
}

/** `FrameMetrics` in the frame contract. */
export interface GpuFrameFrameMetrics {
  generation: number;
  totalUs: number;
  viewUs: number;
  viewEvalUs: number;
  viewTransferUs: number;
  viewDecodeUs: number;
  wireValues: number;
  layoutUs: number;
  textUs: number;
  composeUs: number;
  buildUs: number;
  viewPasses: number;
  layoutPasses: number;
  nodes: number;
  paints: number;
  animationFrames: number;
  ghosts?: number | null;
  layoutMotions?: number | null;
  cost: GpuFrameFrameCost;
  imageCacheBytes?: number | null;
  presenterSingleTarget?: boolean | null;
  activations?: number | null;
  droppedActivations?: number | null;
  composite?: GpuFrameCompositeStats | null;
  backdropCache?: GpuFrameBackdropCacheStats | null;
}

/** `FrameCost` in the frame contract. */
export interface GpuFrameFrameCost {
  nodes: number;
  paints: number;
  glyphs: number;
  vectorElements: number;
  layers: number;
  backdrops: number;
  shadows: number;
  images: number;
  imagePixels: number;
  outlineGlyphs: number;
}

/** `FeedbackStyle` in the frame contract. */
export interface GpuFrameFeedbackStyle {
  pressOverlay: number;
  focusRing: number;
  focusRingWidth: number;
  selectionHandle: number;
  noticeBackground: number;
  noticeText: number;
}

/** `Node` in the frame contract. */
export interface GpuFrameNode {
  identity: string;
  tag: string;
  test_id: string;
  text: string;
  bounds: number[];
  visible_bounds: number[];
  selectable?: string | null;
  text_selection?: GpuFrameInputGeometry | null;
  selection_range?: number[] | null;
  hit_rects?: number[][] | null;
  clips?: GpuFrameClip[] | null;
  scroll?: GpuFrameScroll | null;
  on_click?: number | null;
  checked?: boolean | null;
  range?: GpuFrameSlider | null;
  plot?: GpuFramePlot | null;
  radio_group?: string | null;
  scroll_parent?: string | null;
  accessibility_label?: string | null;
  accessibility_hidden?: boolean | null;
  role?: string | null;
  heading_level?: number | null;
  live_region?: string | null;
  select?: GpuFrameSelect | null;
  button_content?: string | null;
  radii?: number[][] | null;
  radius: number;
  content_clip?: GpuFrameClip | null;
  control_bounds?: number[] | null;
  input?: GpuFrameInput | null;
  clips_children?: boolean | null;
  pressed_style?: boolean | null;
  focus_style?: boolean | null;
  order: number;
}

/** `Clip` in the frame contract. */
export interface GpuFrameClip {
  radii?: number[][] | null;
  bounds: number[];
  radius: number;
  transform?: number[] | null;
}

/** `Scroll` in the frame contract. */
export interface GpuFrameScroll {
  rtl?: boolean | null;
  contain?: boolean | null;
  axis: string;
  on_reach_start?: number | null;
  on_reach_end?: number | null;
  id: string;
  parent?: string | null;
  offset: number;
  max_offset: number;
}

/** `InputGeometry` in the frame contract. */
export interface GpuFrameInputGeometry {
  anchor: number[];
  caret: number[];
  content_bounds: number[];
  selection: number[][];
  composition: number[][];
}

/** `Slider` in the frame contract. */
export interface GpuFrameSlider {
  min: number;
  max: number;
  step: number;
  value: number;
  inset: number;
  enabled: boolean;
  on_change?: number | null;
}

/** `Plot` in the frame contract. */
export interface GpuFramePlot {
  kind: string;
  values: number[];
  domain: number[];
  pie_layout: GpuFramePieLayout;
  selected_index: number;
  on_select?: number | null;
}

/** `PieLayout` in the frame contract. */
export interface GpuFramePieLayout {
  center: number[];
  innerRadius: number;
  outerRadius: number;
  emphasisScale: number;
}

/** `Select` in the frame contract. */
export interface GpuFrameSelect {
  options: GpuFrameSelectOption[];
  selected?: number | null;
  on_select?: number | null;
}

/** `SelectOption` in the frame contract. */
export interface GpuFrameSelectOption {
  value: string;
  label: string;
  enabled: boolean;
}

/** `Input` in the frame contract. */
export interface GpuFrameInput {
  value: string;
  placeholder: string;
  secure: boolean;
  keyboard: string;
  auto_focus: boolean;
  submit_on_enter: boolean;
  enabled: boolean;
  on_input?: number | null;
  on_submit?: number | null;
  max_length?: number | null;
  auto_complete?: string | null;
  enter_key_hint?: string | null;
  auto_capitalize?: string | null;
  spell_check?: boolean | null;
  min?: Object | null;
  max?: Object | null;
  step?: number | null;
  picker?: string | null;
  edit?: GpuFrameEdit | null;
  edit_token?: number | null;
  geometry?: GpuFrameInputGeometry | null;
  scroll_offset: number[];
  ime: GpuFrameIme;
  raise_keyboard: boolean;
}

/** `Edit` in the frame contract. */
export interface GpuFrameEdit {
  text: string;
  selection: number[];
  composition?: number[] | null;
  focused: boolean;
  can_undo: boolean;
  can_redo: boolean;
}

/** `Ime` in the frame contract. */
export interface GpuFrameIme {
  keyboard: string;
  secure: boolean;
  multiline: boolean;
  capitalize: string;
  suggestions: boolean;
  return_key: string;
  return_inserts_newline: boolean;
  hide_on_submit: boolean;
  picker?: string | null;
  auto_complete?: string | null;
  max_length?: number | null;
}

/** `SemanticNode` in the frame contract. */
export interface GpuFrameSemanticNode {
  id: number;
  index: number;
  role: string;
  name: string;
  text: string;
  test_id: string;
  bounds: number[];
  visible: boolean;
  enabled: boolean;
  focusable: boolean;
  clickable: boolean;
  focused: boolean;
  checked?: boolean | null;
  scrollable: boolean;
  range?: GpuFrameSemanticRange | null;
  text_field?: GpuFrameTextField | null;
  parent?: number | null;
  children: number[];
  position?: number[] | null;
  actions: GpuFrameSemanticAction[];
  heading_level?: number | null;
  live_region?: string | null;
  text_selection?: number[] | null;
  pressed?: boolean | null;
  selected?: boolean | null;
  expanded?: boolean | null;
  busy?: boolean | null;
  value?: string | null;
  scroll?: GpuFrameScrollPosition | null;
}

/** `SemanticRange` in the frame contract. */
export interface GpuFrameSemanticRange {
  min: number;
  max: number;
  value: number;
}

/** `TextField` in the frame contract. */
export interface GpuFrameTextField {
  text: string;
  hint: string;
  secure: boolean;
  multiline: boolean;
  selection?: number[] | null;
}

/** `ScrollPosition` in the frame contract. */
export interface GpuFrameScrollPosition {
  offset: number;
  max_offset: number;
  horizontal: boolean;
}

/** `SemanticAction` in the frame contract. */
export interface GpuFrameSemanticAction {
  kind: string;
  index?: number | null;
  label?: string | null;
}

/** `AutomationRequest` in the frame contract. */
export interface GpuFrameAutomationRequest {
  op: string;
  css?: string | null;
  index?: number | null;
  all?: boolean | null;
  maxText?: number | null;
  action?: GpuFrameAutomationAction | null;
  windowMs?: number | null;
  on?: boolean | null;
  name?: string | null;
  args?: Object | null;
  call?: number | null;
}

/** `InputRequest` in the frame contract. */
export interface GpuFrameInputRequest {
  op: string;
  identity?: string | null;
  generation?: number | null;
  token?: number | null;
  point?: number[] | null;
  edge?: number[] | null;
  word?: boolean | null;
  edits?: GpuFrameInputEdit[] | null;
  submit?: boolean | null;
}

/** `InputEdit` in the frame contract. */
export interface GpuFrameInputEdit {
  op: string;
  anchor?: number | null;
  focus?: number | null;
  start?: number | null;
  end?: number | null;
  text?: string | null;
  cursor?: number | null;
  composing?: boolean | null;
  before?: number | null;
  after?: number | null;
  code_points?: boolean | null;
}

/** `AutomationCall` in the frame contract. */
export interface GpuFrameAutomationCall {
  call: number;
}

/** `AutomationResult` in the frame contract. */
export interface GpuFrameAutomationResult {
  done: boolean;
  value?: Object | null;
}

/** `AutomationAction` in the frame contract. */
export interface GpuFrameAutomationAction {
  kind: string;
  force?: boolean | null;
  text?: string | null;
  key?: string | null;
  value?: string | null;
  name?: string | null;
}

/** `AutomationElement` in the frame contract. */
export interface GpuFrameAutomationElement {
  exists: boolean;
  renderer: string;
  index: number;
  count: number;
  tag?: string | null;
  type?: string | null;
  id?: string | null;
  name?: string | null;
  role?: string | null;
  aria_label?: string | null;
  placeholder?: string | null;
  visible: boolean;
  inViewport: boolean;
  enabled: boolean;
  editable: boolean;
  focused?: boolean | null;
  checked?: boolean | null;
  text?: string | null;
  text_truncated?: boolean | null;
  value?: string | null;
  value_truncated?: boolean | null;
  rect?: GpuFrameAutomationRect | null;
  attributes?: Object | null;
}

/** `AutomationQueryAll` in the frame contract. */
export interface GpuFrameAutomationQueryAll {
  count: number;
  renderer: string;
  items: GpuFrameAutomationElement[];
}

/** `AutomationFrame` in the frame contract. */
export interface GpuFrameAutomationFrame {
  renderer: string;
  page: string;
  commit: number;
  firstPresentAt?: number | null;
  lastPresentAt?: number | null;
  frames: number;
  workerQueueUs: GpuFrameQueueTimes;
  evalMs: GpuFrameEvalTimes;
  imageCacheBytes: number;
  imageCacheEntries: number;
  animationFrames: number;
  wakeups: number;
  reveal: Object;
  hud: boolean;
  nodes?: number | null;
  pacing: GpuFrameFramePacing;
  composite?: GpuFrameCompositeStats | null;
  backdropCache?: GpuFrameBackdropCacheStats | null;
}

/** `QueueTimes` in the frame contract. */
export interface GpuFrameQueueTimes {
  last: number;
  p50: number;
  max: number;
}

/** `CompositeStats` in the frame contract. */
export interface GpuFrameCompositeStats {
  draws: number;
  drawPixels: number;
  contentSceneUs: number;
  compositeSceneUs: number;
  encodeUs: number;
  submitUs: number;
  submits: number;
  clears: number;
  copies: number;
  copyPixels: number;
  backdropHits: number;
  backdropMisses: number;
  capturePixels: number;
  filterPixels: number;
  flattened: number;
}

/** `BackdropCacheStats` in the frame contract. */
export interface GpuFrameBackdropCacheStats {
  filtered: number;
  cached: number;
  cachedBytes: number;
}

/** `EvalTimes` in the frame contract. */
export interface GpuFrameEvalTimes {
  last: number;
  max: number;
}

/** `FramePacing` in the frame contract. */
export interface GpuFrameFramePacing {
  windowMs: number;
  frames: number;
  samples: number;
  refreshMs?: number | null;
  frameTimeMs?: GpuFramePercentiles | null;
  fps?: number | null;
  dropped: number;
  jank: number;
  late: number;
  cpuMs?: GpuFramePercentiles | null;
  queueMs?: GpuFramePercentiles | null;
  gpuMs?: GpuFramePercentiles | null;
  inputToPresent: GpuFrameInputLatency;
  firstPresentMs?: number | null;
  firstPresentReason?: string | null;
  reveals: number;
  revealMs?: GpuFramePercentiles | null;
}

/** `Percentiles` in the frame contract. */
export interface GpuFramePercentiles {
  p50: number;
  p95: number;
  p99: number;
  max: number;
}

/** `InputLatency` in the frame contract. */
export interface GpuFrameInputLatency {
  count: number;
  lastMs?: number | null;
  statsMs?: GpuFramePercentiles | null;
}

/** `AutomationRect` in the frame contract. */
export interface GpuFrameAutomationRect {
  left: number;
  top: number;
  width: number;
  height: number;
  right: number;
  bottom: number;
  center_x: number;
  center_y: number;
  viewport_width: number;
  viewport_height: number;
}

/** `AutomationTree` in the frame contract. */
export interface GpuFrameAutomationTree {
  renderer: string;
  nodes: GpuFrameAutomationTreeNode[];
}

/** `AutomationTreeNode` in the frame contract. */
export interface GpuFrameAutomationTreeNode {
  role: string;
  name: string;
  testId?: string | null;
  bounds: number[];
  disabled?: boolean | null;
  focused?: boolean | null;
  offscreen?: boolean | null;
  checked?: boolean | null;
  pressed?: boolean | null;
  selected?: boolean | null;
  expanded?: boolean | null;
  busy?: boolean | null;
  level?: number | null;
  value?: Object | null;
  scroll?: number[] | null;
  children?: GpuFrameAutomationTreeNode[] | null;
}

/** `InputReply` in the frame contract. */
export interface GpuFrameInputReply {
  token?: number | null;
  state?: GpuFrameEditorState | null;
  needs_view_render?: boolean | null;
  moved_to?: string | null;
  selection?: number[] | null;
}

/** `EditorState` in the frame contract. */
export interface GpuFrameEditorState {
  text: string;
  selection: number[];
  composition?: number[] | null;
  focused: boolean;
  can_undo: boolean;
  can_redo: boolean;
}

/** `CopyReply` in the frame contract. */
export interface GpuFrameCopyReply {
  text?: string | null;
}

/** `SemanticRoots` in the frame contract. */
export interface GpuFrameSemanticRoots {
  roots: number[];
  inputFocus?: number | null;
  accessibilityFocus?: number | null;
}

/** `TextMenu` in the frame contract. */
export interface GpuFrameTextMenu {
  target: GpuFrameTextMenuTarget;
  anchor: number[];
  actions: string[];
}

/** `TextMenuTarget` in the frame contract. */
export interface GpuFrameTextMenuTarget {
  kind: string;
  identity?: string | null;
}
