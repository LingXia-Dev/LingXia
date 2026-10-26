// Generated from packages/lingxia-ui/contracts/schema.json; do not edit.
package com.lingxia.ui.generated;

import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Typed readers over the frame contract (schema.json `frame`): Every message the core hands a platform adapter.
 * Messages: snapshot = FrameSnapshot, semanticNode = SemanticNode, automationRequest = AutomationRequest, automationElement = AutomationElement, automationQueryAll = AutomationQueryAll, automationTree = AutomationTree, automationFrame = AutomationFrame, inputReply = InputReply, copySelection = CopyReply, semanticRoots = SemanticRoots, textMenu = TextMenu.
 */
public final class GpuFrame {
    private GpuFrame() {}

    public static final String SCROLL_AXIS_HORIZONTAL = "horizontal";
    public static final String SCROLL_AXIS_VERTICAL = "vertical";
    public static final String KEYBOARD_TEXT = "text";
    public static final String KEYBOARD_NUMBER = "number";
    public static final String KEYBOARD_EMAIL = "email";
    public static final String KEYBOARD_PHONE = "phone";
    public static final String KEYBOARD_SEARCH = "search";
    public static final String CAPITALIZE_NONE = "none";
    public static final String CAPITALIZE_SENTENCES = "sentences";
    public static final String CAPITALIZE_WORDS = "words";
    public static final String CAPITALIZE_CHARACTERS = "characters";
    public static final String RETURN_KEY_NEWLINE = "newline";
    public static final String RETURN_KEY_DONE = "done";
    public static final String RETURN_KEY_GO = "go";
    public static final String RETURN_KEY_NEXT = "next";
    public static final String RETURN_KEY_PREVIOUS = "previous";
    public static final String RETURN_KEY_SEARCH = "search";
    public static final String RETURN_KEY_SEND = "send";
    public static final String PICKER_KIND_DATE = "date";
    public static final String PICKER_KIND_TIME = "time";
    public static final String SEMANTIC_ACTION_CLICK = "click";
    public static final String SEMANTIC_ACTION_SCROLL_FORWARD = "scroll-forward";
    public static final String SEMANTIC_ACTION_SCROLL_BACKWARD = "scroll-backward";
    public static final String SEMANTIC_ACTION_SET_TEXT = "set-text";
    public static final String SEMANTIC_ACTION_SET_SELECTION = "set-selection";
    public static final String SEMANTIC_ACTION_SET_PROGRESS = "set-progress";
    public static final String SEMANTIC_ACTION_CLEAR_SELECTION = "clear-selection";
    public static final String SEMANTIC_ACTION_SELECT_ITEM = "select-item";
    public static final String SEMANTIC_ACTION_SHOW_ON_SCREEN = "show-on-screen";
    public static final String SEMANTIC_ACTION_COPY = "copy";
    public static final String AUTOMATION_REQUEST_QUERY = "query";
    public static final String AUTOMATION_REQUEST_ACT = "act";
    public static final String AUTOMATION_REQUEST_TREE = "tree";
    public static final String AUTOMATION_REQUEST_FRAME = "frame";
    public static final String AUTOMATION_ACTION_CLICK = "click";
    public static final String AUTOMATION_ACTION_FILL = "fill";
    public static final String AUTOMATION_ACTION_TYPE = "type";
    public static final String AUTOMATION_ACTION_SCROLL = "scroll";
    public static final String AUTOMATION_ACTION_PRESS = "press";
    public static final String AUTOMATION_ACTION_SELECT = "select";
    public static final String AUTOMATION_ACTION_DBLCLICK = "dblclick";
    public static final String TEXT_MENU_TARGET_TEXT = "text";
    public static final String TEXT_MENU_TARGET_EDITOR = "editor";
    public static final String MENU_ACTION_CUT = "cut";
    public static final String MENU_ACTION_COPY = "copy";
    public static final String MENU_ACTION_PASTE = "paste";
    public static final String MENU_ACTION_SELECT_ALL = "select_all";
    public static final String MENU_ACTION_UNDO = "undo";
    public static final String MENU_ACTION_REDO = "redo";

    public static final class FrameSnapshot {
        public final JSONObject json;
        public FrameSnapshot(JSONObject json) { this.json = json; }
        /** `generation`: u64 */
        public long generation() { return json.optLong("generation"); }
        /** `nodes`: Node[] */
        public Node[] nodes() { return (Node[]) GpuFrame.array(json.optJSONArray("nodes"), 1, "Node"); }
        /** `images`: ImageSource[] */
        public ImageSource[] images() { return (ImageSource[]) GpuFrame.array(json.optJSONArray("images"), 1, "ImageSource"); }
        /** `metrics`: FrameMetrics */
        public FrameMetrics metrics() { JSONObject o = json.optJSONObject("metrics"); return o == null ? null : new FrameMetrics(o); }
        /** `feedback`: FeedbackStyle */
        public FeedbackStyle feedback() { JSONObject o = json.optJSONObject("feedback"); return o == null ? null : new FeedbackStyle(o); }
    }

    public static final class ImageSource {
        public final JSONObject json;
        public ImageSource(JSONObject json) { this.json = json; }
        /** `source`: string */
        public String source() { return json.isNull("source") ? null : json.optString("source", null); }
        /** `retry_key`: string */
        public String retryKey() { return json.isNull("retry_key") ? null : json.optString("retry_key", null); }
    }

    public static final class FrameMetrics {
        public final JSONObject json;
        public FrameMetrics(JSONObject json) { this.json = json; }
        /** `generation`: u64 */
        public long generation() { return json.optLong("generation"); }
        /** `totalUs`: u64 */
        public long totalUs() { return json.optLong("totalUs"); }
        /** `viewUs`: u64 */
        public long viewUs() { return json.optLong("viewUs"); }
        /** `viewEvalUs`: u64 */
        public long viewEvalUs() { return json.optLong("viewEvalUs"); }
        /** `viewTransferUs`: u64 */
        public long viewTransferUs() { return json.optLong("viewTransferUs"); }
        /** `viewDecodeUs`: u64 */
        public long viewDecodeUs() { return json.optLong("viewDecodeUs"); }
        /** `wireValues`: usize */
        public long wireValues() { return json.optLong("wireValues"); }
        /** `layoutUs`: u64 */
        public long layoutUs() { return json.optLong("layoutUs"); }
        /** `textUs`: u64 */
        public long textUs() { return json.optLong("textUs"); }
        /** `composeUs`: u64 */
        public long composeUs() { return json.optLong("composeUs"); }
        /** `buildUs`: u64 */
        public long buildUs() { return json.optLong("buildUs"); }
        /** `viewPasses`: u32 */
        public long viewPasses() { return json.optLong("viewPasses"); }
        /** `layoutPasses`: u32 */
        public long layoutPasses() { return json.optLong("layoutPasses"); }
        /** `nodes`: usize */
        public long nodes() { return json.optLong("nodes"); }
        /** `paints`: usize */
        public long paints() { return json.optLong("paints"); }
        /** `animationFrames`: usize */
        public long animationFrames() { return json.optLong("animationFrames"); }
        /** `cost`: FrameCost */
        public FrameCost cost() { JSONObject o = json.optJSONObject("cost"); return o == null ? null : new FrameCost(o); }
        /** `imageCacheBytes`: u64? */
        public Long imageCacheBytes() { if (json.isNull("imageCacheBytes")) return null; return json.optLong("imageCacheBytes"); }
        /** `presenterSingleTarget`: bool? */
        public Boolean presenterSingleTarget() { if (json.isNull("presenterSingleTarget")) return null; return json.optBoolean("presenterSingleTarget"); }
    }

    public static final class FrameCost {
        public final JSONObject json;
        public FrameCost(JSONObject json) { this.json = json; }
        /** `nodes`: usize */
        public long nodes() { return json.optLong("nodes"); }
        /** `paints`: usize */
        public long paints() { return json.optLong("paints"); }
        /** `glyphs`: usize */
        public long glyphs() { return json.optLong("glyphs"); }
        /** `vectorElements`: usize */
        public long vectorElements() { return json.optLong("vectorElements"); }
        /** `layers`: usize */
        public long layers() { return json.optLong("layers"); }
        /** `backdrops`: usize */
        public long backdrops() { return json.optLong("backdrops"); }
        /** `shadows`: usize */
        public long shadows() { return json.optLong("shadows"); }
        /** `images`: usize */
        public long images() { return json.optLong("images"); }
        /** `imagePixels`: u64 */
        public long imagePixels() { return json.optLong("imagePixels"); }
    }

    public static final class FeedbackStyle {
        public final JSONObject json;
        public FeedbackStyle(JSONObject json) { this.json = json; }
        /** `pressOverlay`: u32 */
        public long pressOverlay() { return json.optLong("pressOverlay"); }
        /** `focusRing`: u32 */
        public long focusRing() { return json.optLong("focusRing"); }
        /** `focusRingWidth`: f32 */
        public float focusRingWidth() { return (float) json.optDouble("focusRingWidth"); }
        /** `selectionHandle`: u32 */
        public long selectionHandle() { return json.optLong("selectionHandle"); }
        /** `noticeBackground`: u32 */
        public long noticeBackground() { return json.optLong("noticeBackground"); }
        /** `noticeText`: u32 */
        public long noticeText() { return json.optLong("noticeText"); }
    }

    public static final class Node {
        public final JSONObject json;
        public Node(JSONObject json) { this.json = json; }
        /** `identity`: string */
        public String identity() { return json.isNull("identity") ? null : json.optString("identity", null); }
        /** `tag`: string */
        public String tag() { return json.isNull("tag") ? null : json.optString("tag", null); }
        /** `test_id`: string */
        public String testId() { return json.isNull("test_id") ? null : json.optString("test_id", null); }
        /** `text`: string */
        public String text() { return json.isNull("text") ? null : json.optString("text", null); }
        /** `bounds`: f32[4] */
        public float[] bounds() { return (float[]) GpuFrame.array(json.optJSONArray("bounds"), 1, "float"); }
        /** `visible_bounds`: f32[4] */
        public float[] visibleBounds() { return (float[]) GpuFrame.array(json.optJSONArray("visible_bounds"), 1, "float"); }
        /** `selectable`: string? */
        public String selectable() { return json.isNull("selectable") ? null : json.optString("selectable", null); }
        /** `text_selection`: InputGeometry? */
        public InputGeometry textSelection() { JSONObject o = json.optJSONObject("text_selection"); return o == null ? null : new InputGeometry(o); }
        /** `selection_range`: u32[2]? */
        public long[] selectionRange() { return (long[]) GpuFrame.array(json.optJSONArray("selection_range"), 1, "long"); }
        /** `hit_rects`: f32[4][]? */
        public float[][] hitRects() { return (float[][]) GpuFrame.array(json.optJSONArray("hit_rects"), 2, "float"); }
        /** `clips`: Clip[]? */
        public Clip[] clips() { return (Clip[]) GpuFrame.array(json.optJSONArray("clips"), 1, "Clip"); }
        /** `scroll`: Scroll? */
        public Scroll scroll() { JSONObject o = json.optJSONObject("scroll"); return o == null ? null : new Scroll(o); }
        /** `on_click`: u64? */
        public Long onClick() { if (json.isNull("on_click")) return null; return json.optLong("on_click"); }
        /** `checked`: bool? */
        public Boolean checked() { if (json.isNull("checked")) return null; return json.optBoolean("checked"); }
        /** `range`: Slider? */
        public Slider range() { JSONObject o = json.optJSONObject("range"); return o == null ? null : new Slider(o); }
        /** `plot`: Plot? */
        public Plot plot() { JSONObject o = json.optJSONObject("plot"); return o == null ? null : new Plot(o); }
        /** `radio_group`: string? */
        public String radioGroup() { return json.isNull("radio_group") ? null : json.optString("radio_group", null); }
        /** `scroll_parent`: string? */
        public String scrollParent() { return json.isNull("scroll_parent") ? null : json.optString("scroll_parent", null); }
        /** `accessibility_label`: string? */
        public String accessibilityLabel() { return json.isNull("accessibility_label") ? null : json.optString("accessibility_label", null); }
        /** `accessibility_hidden`: bool? */
        public Boolean accessibilityHidden() { if (json.isNull("accessibility_hidden")) return null; return json.optBoolean("accessibility_hidden"); }
        /** `role`: string? */
        public String role() { return json.isNull("role") ? null : json.optString("role", null); }
        /** `heading_level`: u8? */
        public Long headingLevel() { if (json.isNull("heading_level")) return null; return json.optLong("heading_level"); }
        /** `live_region`: string? */
        public String liveRegion() { return json.isNull("live_region") ? null : json.optString("live_region", null); }
        /** `select`: Select? */
        public Select select() { JSONObject o = json.optJSONObject("select"); return o == null ? null : new Select(o); }
        /** `button_content`: string? */
        public String buttonContent() { return json.isNull("button_content") ? null : json.optString("button_content", null); }
        /** `radii`: f32[2][4]? */
        public float[][] radii() { return (float[][]) GpuFrame.array(json.optJSONArray("radii"), 2, "float"); }
        /** `radius`: f32 */
        public float radius() { return (float) json.optDouble("radius"); }
        /** `content_clip`: Clip? */
        public Clip contentClip() { JSONObject o = json.optJSONObject("content_clip"); return o == null ? null : new Clip(o); }
        /** `control_bounds`: f32[4]? */
        public float[] controlBounds() { return (float[]) GpuFrame.array(json.optJSONArray("control_bounds"), 1, "float"); }
        /** `input`: Input? */
        public Input input() { JSONObject o = json.optJSONObject("input"); return o == null ? null : new Input(o); }
        /** `clips_children`: bool? */
        public Boolean clipsChildren() { if (json.isNull("clips_children")) return null; return json.optBoolean("clips_children"); }
        /** `pressed_style`: bool? */
        public Boolean pressedStyle() { if (json.isNull("pressed_style")) return null; return json.optBoolean("pressed_style"); }
        /** `focus_style`: bool? */
        public Boolean focusStyle() { if (json.isNull("focus_style")) return null; return json.optBoolean("focus_style"); }
        /** `order`: u32 */
        public long order() { return json.optLong("order"); }
    }

    public static final class Clip {
        public final JSONObject json;
        public Clip(JSONObject json) { this.json = json; }
        /** `radii`: f32[2][4]? */
        public float[][] radii() { return (float[][]) GpuFrame.array(json.optJSONArray("radii"), 2, "float"); }
        /** `bounds`: f32[4] */
        public float[] bounds() { return (float[]) GpuFrame.array(json.optJSONArray("bounds"), 1, "float"); }
        /** `radius`: f32 */
        public float radius() { return (float) json.optDouble("radius"); }
        /** `transform`: f64[6]? */
        public double[] transform() { return (double[]) GpuFrame.array(json.optJSONArray("transform"), 1, "double"); }
    }

    public static final class Scroll {
        public final JSONObject json;
        public Scroll(JSONObject json) { this.json = json; }
        /** `rtl`: bool? */
        public Boolean rtl() { if (json.isNull("rtl")) return null; return json.optBoolean("rtl"); }
        /** `contain`: bool? */
        public Boolean contain() { if (json.isNull("contain")) return null; return json.optBoolean("contain"); }
        /** `axis`: ScrollAxis */
        public String axis() { return json.isNull("axis") ? null : json.optString("axis", null); }
        /** `on_reach_start`: u64? */
        public Long onReachStart() { if (json.isNull("on_reach_start")) return null; return json.optLong("on_reach_start"); }
        /** `on_reach_end`: u64? */
        public Long onReachEnd() { if (json.isNull("on_reach_end")) return null; return json.optLong("on_reach_end"); }
        /** `id`: string */
        public String id() { return json.isNull("id") ? null : json.optString("id", null); }
        /** `parent`: string? */
        public String parent() { return json.isNull("parent") ? null : json.optString("parent", null); }
        /** `offset`: f32 */
        public float offset() { return (float) json.optDouble("offset"); }
        /** `max_offset`: f32 */
        public float maxOffset() { return (float) json.optDouble("max_offset"); }
    }

    public static final class InputGeometry {
        public final JSONObject json;
        public InputGeometry(JSONObject json) { this.json = json; }
        /** `anchor`: f32[4] */
        public float[] anchor() { return (float[]) GpuFrame.array(json.optJSONArray("anchor"), 1, "float"); }
        /** `caret`: f32[4] */
        public float[] caret() { return (float[]) GpuFrame.array(json.optJSONArray("caret"), 1, "float"); }
        /** `content_bounds`: f32[4] */
        public float[] contentBounds() { return (float[]) GpuFrame.array(json.optJSONArray("content_bounds"), 1, "float"); }
        /** `selection`: f32[4][] */
        public float[][] selection() { return (float[][]) GpuFrame.array(json.optJSONArray("selection"), 2, "float"); }
        /** `composition`: f32[4][] */
        public float[][] composition() { return (float[][]) GpuFrame.array(json.optJSONArray("composition"), 2, "float"); }
    }

    public static final class Slider {
        public final JSONObject json;
        public Slider(JSONObject json) { this.json = json; }
        /** `min`: f64 */
        public double min() { return json.optDouble("min"); }
        /** `max`: f64 */
        public double max() { return json.optDouble("max"); }
        /** `step`: f64 */
        public double step() { return json.optDouble("step"); }
        /** `value`: f64 */
        public double value() { return json.optDouble("value"); }
        /** `inset`: f32 */
        public float inset() { return (float) json.optDouble("inset"); }
        /** `enabled`: bool */
        public boolean enabled() { return json.optBoolean("enabled"); }
        /** `on_change`: u64? */
        public Long onChange() { if (json.isNull("on_change")) return null; return json.optLong("on_change"); }
    }

    public static final class Plot {
        public final JSONObject json;
        public Plot(JSONObject json) { this.json = json; }
        /** `kind`: string */
        public String kind() { return json.isNull("kind") ? null : json.optString("kind", null); }
        /** `values`: f64[] */
        public double[] values() { return (double[]) GpuFrame.array(json.optJSONArray("values"), 1, "double"); }
        /** `domain`: f64[2] */
        public double[] domain() { return (double[]) GpuFrame.array(json.optJSONArray("domain"), 1, "double"); }
        /** `pie_layout`: PieLayout */
        public PieLayout pieLayout() { JSONObject o = json.optJSONObject("pie_layout"); return o == null ? null : new PieLayout(o); }
        /** `selected_index`: i64 */
        public long selectedIndex() { return json.optLong("selected_index"); }
        /** `on_select`: u64? */
        public Long onSelect() { if (json.isNull("on_select")) return null; return json.optLong("on_select"); }
    }

    public static final class PieLayout {
        public final JSONObject json;
        public PieLayout(JSONObject json) { this.json = json; }
        /** `center`: f64[2] */
        public double[] center() { return (double[]) GpuFrame.array(json.optJSONArray("center"), 1, "double"); }
        /** `innerRadius`: f64 */
        public double innerRadius() { return json.optDouble("innerRadius"); }
        /** `outerRadius`: f64 */
        public double outerRadius() { return json.optDouble("outerRadius"); }
        /** `emphasisScale`: f64 */
        public double emphasisScale() { return json.optDouble("emphasisScale"); }
    }

    public static final class Select {
        public final JSONObject json;
        public Select(JSONObject json) { this.json = json; }
        /** `options`: SelectOption[] */
        public SelectOption[] options() { return (SelectOption[]) GpuFrame.array(json.optJSONArray("options"), 1, "SelectOption"); }
        /** `selected`: usize? */
        public Long selected() { if (json.isNull("selected")) return null; return json.optLong("selected"); }
        /** `on_select`: u64? */
        public Long onSelect() { if (json.isNull("on_select")) return null; return json.optLong("on_select"); }
    }

    public static final class SelectOption {
        public final JSONObject json;
        public SelectOption(JSONObject json) { this.json = json; }
        /** `value`: string */
        public String value() { return json.isNull("value") ? null : json.optString("value", null); }
        /** `label`: string */
        public String label() { return json.isNull("label") ? null : json.optString("label", null); }
        /** `enabled`: bool */
        public boolean enabled() { return json.optBoolean("enabled"); }
    }

    public static final class Input {
        public final JSONObject json;
        public Input(JSONObject json) { this.json = json; }
        /** `value`: string */
        public String value() { return json.isNull("value") ? null : json.optString("value", null); }
        /** `placeholder`: string */
        public String placeholder() { return json.isNull("placeholder") ? null : json.optString("placeholder", null); }
        /** `secure`: bool */
        public boolean secure() { return json.optBoolean("secure"); }
        /** `keyboard`: string */
        public String keyboard() { return json.isNull("keyboard") ? null : json.optString("keyboard", null); }
        /** `auto_focus`: bool */
        public boolean autoFocus() { return json.optBoolean("auto_focus"); }
        /** `submit_on_enter`: bool */
        public boolean submitOnEnter() { return json.optBoolean("submit_on_enter"); }
        /** `enabled`: bool */
        public boolean enabled() { return json.optBoolean("enabled"); }
        /** `on_input`: u64? */
        public Long onInput() { if (json.isNull("on_input")) return null; return json.optLong("on_input"); }
        /** `on_submit`: u64? */
        public Long onSubmit() { if (json.isNull("on_submit")) return null; return json.optLong("on_submit"); }
        /** `max_length`: u32? */
        public Long maxLength() { if (json.isNull("max_length")) return null; return json.optLong("max_length"); }
        /** `auto_complete`: string? */
        public String autoComplete() { return json.isNull("auto_complete") ? null : json.optString("auto_complete", null); }
        /** `enter_key_hint`: string? */
        public String enterKeyHint() { return json.isNull("enter_key_hint") ? null : json.optString("enter_key_hint", null); }
        /** `auto_capitalize`: string? */
        public String autoCapitalize() { return json.isNull("auto_capitalize") ? null : json.optString("auto_capitalize", null); }
        /** `spell_check`: bool? */
        public Boolean spellCheck() { if (json.isNull("spell_check")) return null; return json.optBoolean("spell_check"); }
        /** `min`: any? */
        public Object min() { return json.isNull("min") ? null : json.opt("min"); }
        /** `max`: any? */
        public Object max() { return json.isNull("max") ? null : json.opt("max"); }
        /** `step`: f64? */
        public Double step() { if (json.isNull("step")) return null; return json.optDouble("step"); }
        /** `picker`: string? */
        public String picker() { return json.isNull("picker") ? null : json.optString("picker", null); }
        /** `edit`: Edit? */
        public Edit edit() { JSONObject o = json.optJSONObject("edit"); return o == null ? null : new Edit(o); }
        /** `edit_token`: u64? */
        public Long editToken() { if (json.isNull("edit_token")) return null; return json.optLong("edit_token"); }
        /** `geometry`: InputGeometry? */
        public InputGeometry geometry() { JSONObject o = json.optJSONObject("geometry"); return o == null ? null : new InputGeometry(o); }
        /** `scroll_offset`: f32[2] */
        public float[] scrollOffset() { return (float[]) GpuFrame.array(json.optJSONArray("scroll_offset"), 1, "float"); }
        /** `ime`: Ime */
        public Ime ime() { JSONObject o = json.optJSONObject("ime"); return o == null ? null : new Ime(o); }
        /** `raise_keyboard`: bool */
        public boolean raiseKeyboard() { return json.optBoolean("raise_keyboard"); }
    }

    public static final class Edit {
        public final JSONObject json;
        public Edit(JSONObject json) { this.json = json; }
        /** `text`: string */
        public String text() { return json.isNull("text") ? null : json.optString("text", null); }
        /** `selection`: u32[2] */
        public long[] selection() { return (long[]) GpuFrame.array(json.optJSONArray("selection"), 1, "long"); }
        /** `composition`: u32[2]? */
        public long[] composition() { return (long[]) GpuFrame.array(json.optJSONArray("composition"), 1, "long"); }
        /** `focused`: bool */
        public boolean focused() { return json.optBoolean("focused"); }
        /** `can_undo`: bool */
        public boolean canUndo() { return json.optBoolean("can_undo"); }
        /** `can_redo`: bool */
        public boolean canRedo() { return json.optBoolean("can_redo"); }
    }

    public static final class Ime {
        public final JSONObject json;
        public Ime(JSONObject json) { this.json = json; }
        /** `keyboard`: Keyboard */
        public String keyboard() { return json.isNull("keyboard") ? null : json.optString("keyboard", null); }
        /** `secure`: bool */
        public boolean secure() { return json.optBoolean("secure"); }
        /** `multiline`: bool */
        public boolean multiline() { return json.optBoolean("multiline"); }
        /** `capitalize`: Capitalize */
        public String capitalize() { return json.isNull("capitalize") ? null : json.optString("capitalize", null); }
        /** `suggestions`: bool */
        public boolean suggestions() { return json.optBoolean("suggestions"); }
        /** `return_key`: ReturnKey */
        public String returnKey() { return json.isNull("return_key") ? null : json.optString("return_key", null); }
        /** `return_inserts_newline`: bool */
        public boolean returnInsertsNewline() { return json.optBoolean("return_inserts_newline"); }
        /** `hide_on_submit`: bool */
        public boolean hideOnSubmit() { return json.optBoolean("hide_on_submit"); }
        /** `picker`: Picker? */
        public Picker picker() { JSONObject o = json.optJSONObject("picker"); return o == null ? null : new Picker(o); }
        /** `auto_complete`: string? */
        public String autoComplete() { return json.isNull("auto_complete") ? null : json.optString("auto_complete", null); }
        /** `max_length`: u32? */
        public Long maxLength() { if (json.isNull("max_length")) return null; return json.optLong("max_length"); }
    }

    public static final class Picker {
        public final JSONObject json;
        public Picker(JSONObject json) { this.json = json; }
        /** `kind`: PickerKind */
        public String kind() { return json.isNull("kind") ? null : json.optString("kind", null); }
        /** `min`: string? */
        public String min() { return json.isNull("min") ? null : json.optString("min", null); }
        /** `max`: string? */
        public String max() { return json.isNull("max") ? null : json.optString("max", null); }
        /** `seed`: u32[3]? */
        public long[] seed() { return (long[]) GpuFrame.array(json.optJSONArray("seed"), 1, "long"); }
        /** `min_parts`: u32[3]? */
        public long[] minParts() { return (long[]) GpuFrame.array(json.optJSONArray("min_parts"), 1, "long"); }
        /** `max_parts`: u32[3]? */
        public long[] maxParts() { return (long[]) GpuFrame.array(json.optJSONArray("max_parts"), 1, "long"); }
    }

    public static final class SemanticNode {
        public final JSONObject json;
        public SemanticNode(JSONObject json) { this.json = json; }
        /** `id`: u32 */
        public long id() { return json.optLong("id"); }
        /** `index`: usize */
        public long index() { return json.optLong("index"); }
        /** `role`: string */
        public String role() { return json.isNull("role") ? null : json.optString("role", null); }
        /** `name`: string */
        public String name() { return json.isNull("name") ? null : json.optString("name", null); }
        /** `text`: string */
        public String text() { return json.isNull("text") ? null : json.optString("text", null); }
        /** `test_id`: string */
        public String testId() { return json.isNull("test_id") ? null : json.optString("test_id", null); }
        /** `bounds`: f32[4] */
        public float[] bounds() { return (float[]) GpuFrame.array(json.optJSONArray("bounds"), 1, "float"); }
        /** `visible`: bool */
        public boolean visible() { return json.optBoolean("visible"); }
        /** `enabled`: bool */
        public boolean enabled() { return json.optBoolean("enabled"); }
        /** `focusable`: bool */
        public boolean focusable() { return json.optBoolean("focusable"); }
        /** `clickable`: bool */
        public boolean clickable() { return json.optBoolean("clickable"); }
        /** `focused`: bool */
        public boolean focused() { return json.optBoolean("focused"); }
        /** `checked`: bool? */
        public Boolean checked() { if (json.isNull("checked")) return null; return json.optBoolean("checked"); }
        /** `scrollable`: bool */
        public boolean scrollable() { return json.optBoolean("scrollable"); }
        /** `range`: SemanticRange? */
        public SemanticRange range() { JSONObject o = json.optJSONObject("range"); return o == null ? null : new SemanticRange(o); }
        /** `text_field`: TextField? */
        public TextField textField() { JSONObject o = json.optJSONObject("text_field"); return o == null ? null : new TextField(o); }
        /** `parent`: u32? */
        public Long parent() { if (json.isNull("parent")) return null; return json.optLong("parent"); }
        /** `children`: u32[] */
        public long[] children() { return (long[]) GpuFrame.array(json.optJSONArray("children"), 1, "long"); }
        /** `position`: usize[2]? */
        public long[] position() { return (long[]) GpuFrame.array(json.optJSONArray("position"), 1, "long"); }
        /** `actions`: SemanticAction[] */
        public SemanticAction[] actions() { return (SemanticAction[]) GpuFrame.array(json.optJSONArray("actions"), 1, "SemanticAction"); }
        /** `heading_level`: u8? */
        public Long headingLevel() { if (json.isNull("heading_level")) return null; return json.optLong("heading_level"); }
        /** `live_region`: string? */
        public String liveRegion() { return json.isNull("live_region") ? null : json.optString("live_region", null); }
        /** `text_selection`: u32[2]? */
        public long[] textSelection() { return (long[]) GpuFrame.array(json.optJSONArray("text_selection"), 1, "long"); }
        /** `pressed`: bool? */
        public Boolean pressed() { if (json.isNull("pressed")) return null; return json.optBoolean("pressed"); }
        /** `selected`: bool? */
        public Boolean selected() { if (json.isNull("selected")) return null; return json.optBoolean("selected"); }
        /** `expanded`: bool? */
        public Boolean expanded() { if (json.isNull("expanded")) return null; return json.optBoolean("expanded"); }
        /** `busy`: bool? */
        public Boolean busy() { if (json.isNull("busy")) return null; return json.optBoolean("busy"); }
        /** `scroll`: ScrollPosition? */
        public ScrollPosition scroll() { JSONObject o = json.optJSONObject("scroll"); return o == null ? null : new ScrollPosition(o); }
    }

    public static final class SemanticRange {
        public final JSONObject json;
        public SemanticRange(JSONObject json) { this.json = json; }
        /** `min`: f64 */
        public double min() { return json.optDouble("min"); }
        /** `max`: f64 */
        public double max() { return json.optDouble("max"); }
        /** `value`: f64 */
        public double value() { return json.optDouble("value"); }
    }

    public static final class TextField {
        public final JSONObject json;
        public TextField(JSONObject json) { this.json = json; }
        /** `text`: string */
        public String text() { return json.isNull("text") ? null : json.optString("text", null); }
        /** `hint`: string */
        public String hint() { return json.isNull("hint") ? null : json.optString("hint", null); }
        /** `secure`: bool */
        public boolean secure() { return json.optBoolean("secure"); }
        /** `multiline`: bool */
        public boolean multiline() { return json.optBoolean("multiline"); }
        /** `selection`: u32[2]? */
        public long[] selection() { return (long[]) GpuFrame.array(json.optJSONArray("selection"), 1, "long"); }
    }

    public static final class ScrollPosition {
        public final JSONObject json;
        public ScrollPosition(JSONObject json) { this.json = json; }
        /** `offset`: f32 */
        public float offset() { return (float) json.optDouble("offset"); }
        /** `max_offset`: f32 */
        public float maxOffset() { return (float) json.optDouble("max_offset"); }
        /** `horizontal`: bool */
        public boolean horizontal() { return json.optBoolean("horizontal"); }
    }

    public static final class SemanticAction {
        public final JSONObject json;
        public SemanticAction(JSONObject json) { this.json = json; }
        /** `kind`: string */
        public String kind() { return json.isNull("kind") ? null : json.optString("kind", null); }
        /** `index`: usize? */
        public Long index() { if (json.isNull("index")) return null; return json.optLong("index"); }
        /** `label`: string? */
        public String label() { return json.isNull("label") ? null : json.optString("label", null); }
    }

    public static final class AutomationRequest {
        public final JSONObject json;
        public AutomationRequest(JSONObject json) { this.json = json; }
        /** `op`: string */
        public String op() { return json.isNull("op") ? null : json.optString("op", null); }
        /** `css`: string? */
        public String css() { return json.isNull("css") ? null : json.optString("css", null); }
        /** `index`: usize? */
        public Long index() { if (json.isNull("index")) return null; return json.optLong("index"); }
        /** `all`: bool? */
        public Boolean all() { if (json.isNull("all")) return null; return json.optBoolean("all"); }
        /** `maxText`: usize? */
        public Long maxText() { if (json.isNull("maxText")) return null; return json.optLong("maxText"); }
        /** `action`: AutomationAction? */
        public AutomationAction action() { JSONObject o = json.optJSONObject("action"); return o == null ? null : new AutomationAction(o); }
    }

    public static final class AutomationAction {
        public final JSONObject json;
        public AutomationAction(JSONObject json) { this.json = json; }
        /** `kind`: string */
        public String kind() { return json.isNull("kind") ? null : json.optString("kind", null); }
        /** `force`: bool? */
        public Boolean force() { if (json.isNull("force")) return null; return json.optBoolean("force"); }
        /** `text`: string? */
        public String text() { return json.isNull("text") ? null : json.optString("text", null); }
        /** `key`: string? */
        public String key() { return json.isNull("key") ? null : json.optString("key", null); }
        /** `value`: string? */
        public String value() { return json.isNull("value") ? null : json.optString("value", null); }
    }

    public static final class AutomationElement {
        public final JSONObject json;
        public AutomationElement(JSONObject json) { this.json = json; }
        /** `exists`: bool */
        public boolean exists() { return json.optBoolean("exists"); }
        /** `renderer`: string */
        public String renderer() { return json.isNull("renderer") ? null : json.optString("renderer", null); }
        /** `index`: usize */
        public long index() { return json.optLong("index"); }
        /** `count`: usize */
        public long count() { return json.optLong("count"); }
        /** `tag`: string? */
        public String tag() { return json.isNull("tag") ? null : json.optString("tag", null); }
        /** `type`: string? */
        public String type() { return json.isNull("type") ? null : json.optString("type", null); }
        /** `id`: string? */
        public String id() { return json.isNull("id") ? null : json.optString("id", null); }
        /** `name`: string? */
        public String name() { return json.isNull("name") ? null : json.optString("name", null); }
        /** `role`: string? */
        public String role() { return json.isNull("role") ? null : json.optString("role", null); }
        /** `aria_label`: string? */
        public String ariaLabel() { return json.isNull("aria_label") ? null : json.optString("aria_label", null); }
        /** `placeholder`: string? */
        public String placeholder() { return json.isNull("placeholder") ? null : json.optString("placeholder", null); }
        /** `visible`: bool */
        public boolean visible() { return json.optBoolean("visible"); }
        /** `inViewport`: bool */
        public boolean inViewport() { return json.optBoolean("inViewport"); }
        /** `enabled`: bool */
        public boolean enabled() { return json.optBoolean("enabled"); }
        /** `editable`: bool */
        public boolean editable() { return json.optBoolean("editable"); }
        /** `checked`: bool? */
        public Boolean checked() { if (json.isNull("checked")) return null; return json.optBoolean("checked"); }
        /** `text`: string? */
        public String text() { return json.isNull("text") ? null : json.optString("text", null); }
        /** `text_truncated`: bool? */
        public Boolean textTruncated() { if (json.isNull("text_truncated")) return null; return json.optBoolean("text_truncated"); }
        /** `value`: string? */
        public String value() { return json.isNull("value") ? null : json.optString("value", null); }
        /** `value_truncated`: bool? */
        public Boolean valueTruncated() { if (json.isNull("value_truncated")) return null; return json.optBoolean("value_truncated"); }
        /** `rect`: AutomationRect? */
        public AutomationRect rect() { JSONObject o = json.optJSONObject("rect"); return o == null ? null : new AutomationRect(o); }
    }

    public static final class AutomationQueryAll {
        public final JSONObject json;
        public AutomationQueryAll(JSONObject json) { this.json = json; }
        /** `count`: usize */
        public long count() { return json.optLong("count"); }
        /** `renderer`: string */
        public String renderer() { return json.isNull("renderer") ? null : json.optString("renderer", null); }
        /** `items`: AutomationElement[] */
        public AutomationElement[] items() { return (AutomationElement[]) GpuFrame.array(json.optJSONArray("items"), 1, "AutomationElement"); }
    }

    public static final class AutomationFrame {
        public final JSONObject json;
        public AutomationFrame(JSONObject json) { this.json = json; }
        /** `renderer`: string */
        public String renderer() { return json.isNull("renderer") ? null : json.optString("renderer", null); }
        /** `page`: string */
        public String page() { return json.isNull("page") ? null : json.optString("page", null); }
        /** `commit`: u64 */
        public long commit() { return json.optLong("commit"); }
        /** `firstPresentAt`: u64? */
        public Long firstPresentAt() { if (json.isNull("firstPresentAt")) return null; return json.optLong("firstPresentAt"); }
        /** `lastPresentAt`: u64? */
        public Long lastPresentAt() { if (json.isNull("lastPresentAt")) return null; return json.optLong("lastPresentAt"); }
        /** `frames`: u64 */
        public long frames() { return json.optLong("frames"); }
        /** `workerQueueUs`: QueueTimes */
        public QueueTimes workerQueueUs() { JSONObject o = json.optJSONObject("workerQueueUs"); return o == null ? null : new QueueTimes(o); }
        /** `evalMs`: EvalTimes */
        public EvalTimes evalMs() { JSONObject o = json.optJSONObject("evalMs"); return o == null ? null : new EvalTimes(o); }
        /** `imageCacheBytes`: u64 */
        public long imageCacheBytes() { return json.optLong("imageCacheBytes"); }
        /** `imageCacheEntries`: usize */
        public long imageCacheEntries() { return json.optLong("imageCacheEntries"); }
        /** `animationFrames`: usize */
        public long animationFrames() { return json.optLong("animationFrames"); }
        /** `wakeups`: u64 */
        public long wakeups() { return json.optLong("wakeups"); }
        /** `reveal`: any */
        public Object reveal() { return json.isNull("reveal") ? null : json.opt("reveal"); }
    }

    public static final class QueueTimes {
        public final JSONObject json;
        public QueueTimes(JSONObject json) { this.json = json; }
        /** `last`: u64 */
        public long last() { return json.optLong("last"); }
        /** `p50`: u64 */
        public long p50() { return json.optLong("p50"); }
        /** `max`: u64 */
        public long max() { return json.optLong("max"); }
    }

    public static final class EvalTimes {
        public final JSONObject json;
        public EvalTimes(JSONObject json) { this.json = json; }
        /** `last`: f64 */
        public double last() { return json.optDouble("last"); }
        /** `max`: f64 */
        public double max() { return json.optDouble("max"); }
    }

    public static final class AutomationRect {
        public final JSONObject json;
        public AutomationRect(JSONObject json) { this.json = json; }
        /** `left`: f32 */
        public float left() { return (float) json.optDouble("left"); }
        /** `top`: f32 */
        public float top() { return (float) json.optDouble("top"); }
        /** `width`: f32 */
        public float width() { return (float) json.optDouble("width"); }
        /** `height`: f32 */
        public float height() { return (float) json.optDouble("height"); }
        /** `right`: f32 */
        public float right() { return (float) json.optDouble("right"); }
        /** `bottom`: f32 */
        public float bottom() { return (float) json.optDouble("bottom"); }
        /** `center_x`: f32 */
        public float centerX() { return (float) json.optDouble("center_x"); }
        /** `center_y`: f32 */
        public float centerY() { return (float) json.optDouble("center_y"); }
        /** `viewport_width`: f32 */
        public float viewportWidth() { return (float) json.optDouble("viewport_width"); }
        /** `viewport_height`: f32 */
        public float viewportHeight() { return (float) json.optDouble("viewport_height"); }
    }

    public static final class AutomationTree {
        public final JSONObject json;
        public AutomationTree(JSONObject json) { this.json = json; }
        /** `renderer`: string */
        public String renderer() { return json.isNull("renderer") ? null : json.optString("renderer", null); }
        /** `nodes`: AutomationTreeNode[] */
        public AutomationTreeNode[] nodes() { return (AutomationTreeNode[]) GpuFrame.array(json.optJSONArray("nodes"), 1, "AutomationTreeNode"); }
    }

    public static final class AutomationTreeNode {
        public final JSONObject json;
        public AutomationTreeNode(JSONObject json) { this.json = json; }
        /** `role`: string */
        public String role() { return json.isNull("role") ? null : json.optString("role", null); }
        /** `name`: string */
        public String name() { return json.isNull("name") ? null : json.optString("name", null); }
        /** `testId`: string? */
        public String testId() { return json.isNull("testId") ? null : json.optString("testId", null); }
        /** `bounds`: f32[4] */
        public float[] bounds() { return (float[]) GpuFrame.array(json.optJSONArray("bounds"), 1, "float"); }
        /** `disabled`: bool? */
        public Boolean disabled() { if (json.isNull("disabled")) return null; return json.optBoolean("disabled"); }
        /** `focused`: bool? */
        public Boolean focused() { if (json.isNull("focused")) return null; return json.optBoolean("focused"); }
        /** `offscreen`: bool? */
        public Boolean offscreen() { if (json.isNull("offscreen")) return null; return json.optBoolean("offscreen"); }
        /** `checked`: bool? */
        public Boolean checked() { if (json.isNull("checked")) return null; return json.optBoolean("checked"); }
        /** `pressed`: bool? */
        public Boolean pressed() { if (json.isNull("pressed")) return null; return json.optBoolean("pressed"); }
        /** `selected`: bool? */
        public Boolean selected() { if (json.isNull("selected")) return null; return json.optBoolean("selected"); }
        /** `expanded`: bool? */
        public Boolean expanded() { if (json.isNull("expanded")) return null; return json.optBoolean("expanded"); }
        /** `busy`: bool? */
        public Boolean busy() { if (json.isNull("busy")) return null; return json.optBoolean("busy"); }
        /** `level`: u8? */
        public Long level() { if (json.isNull("level")) return null; return json.optLong("level"); }
        /** `value`: any? */
        public Object value() { return json.isNull("value") ? null : json.opt("value"); }
        /** `scroll`: f32[2]? */
        public float[] scroll() { return (float[]) GpuFrame.array(json.optJSONArray("scroll"), 1, "float"); }
        /** `children`: AutomationTreeNode[]? */
        public AutomationTreeNode[] children() { return (AutomationTreeNode[]) GpuFrame.array(json.optJSONArray("children"), 1, "AutomationTreeNode"); }
    }

    public static final class InputReply {
        public final JSONObject json;
        public InputReply(JSONObject json) { this.json = json; }
        /** `token`: u64? */
        public Long token() { if (json.isNull("token")) return null; return json.optLong("token"); }
        /** `state`: EditorState? */
        public EditorState state() { JSONObject o = json.optJSONObject("state"); return o == null ? null : new EditorState(o); }
        /** `needs_view_render`: bool? */
        public Boolean needsViewRender() { if (json.isNull("needs_view_render")) return null; return json.optBoolean("needs_view_render"); }
        /** `moved_to`: string? */
        public String movedTo() { return json.isNull("moved_to") ? null : json.optString("moved_to", null); }
        /** `selection`: u32[2]? */
        public long[] selection() { return (long[]) GpuFrame.array(json.optJSONArray("selection"), 1, "long"); }
        /** `value`: string? */
        public String value() { return json.isNull("value") ? null : json.optString("value", null); }
    }

    public static final class EditorState {
        public final JSONObject json;
        public EditorState(JSONObject json) { this.json = json; }
        /** `text`: string */
        public String text() { return json.isNull("text") ? null : json.optString("text", null); }
        /** `selection`: u32[2] */
        public long[] selection() { return (long[]) GpuFrame.array(json.optJSONArray("selection"), 1, "long"); }
        /** `composition`: u32[2]? */
        public long[] composition() { return (long[]) GpuFrame.array(json.optJSONArray("composition"), 1, "long"); }
        /** `focused`: bool */
        public boolean focused() { return json.optBoolean("focused"); }
        /** `can_undo`: bool */
        public boolean canUndo() { return json.optBoolean("can_undo"); }
        /** `can_redo`: bool */
        public boolean canRedo() { return json.optBoolean("can_redo"); }
    }

    public static final class CopyReply {
        public final JSONObject json;
        public CopyReply(JSONObject json) { this.json = json; }
        /** `text`: string? */
        public String text() { return json.isNull("text") ? null : json.optString("text", null); }
    }

    public static final class SemanticRoots {
        public final JSONObject json;
        public SemanticRoots(JSONObject json) { this.json = json; }
        /** `roots`: u32[] */
        public long[] roots() { return (long[]) GpuFrame.array(json.optJSONArray("roots"), 1, "long"); }
        /** `inputFocus`: u32? */
        public Long inputFocus() { if (json.isNull("inputFocus")) return null; return json.optLong("inputFocus"); }
        /** `accessibilityFocus`: u32? */
        public Long accessibilityFocus() { if (json.isNull("accessibilityFocus")) return null; return json.optLong("accessibilityFocus"); }
    }

    public static final class TextMenu {
        public final JSONObject json;
        public TextMenu(JSONObject json) { this.json = json; }
        /** `target`: TextMenuTarget */
        public TextMenuTarget target() { JSONObject o = json.optJSONObject("target"); return o == null ? null : new TextMenuTarget(o); }
        /** `anchor`: f32[4] */
        public float[] anchor() { return (float[]) GpuFrame.array(json.optJSONArray("anchor"), 1, "float"); }
        /** `actions`: MenuAction[] */
        public String[] actions() { return (String[]) GpuFrame.array(json.optJSONArray("actions"), 1, "String"); }
    }

    public static final class TextMenuTarget {
        public final JSONObject json;
        public TextMenuTarget(JSONObject json) { this.json = json; }
        /** `kind`: string */
        public String kind() { return json.isNull("kind") ? null : json.optString("kind", null); }
        /** `identity`: string? */
        public String identity() { return json.isNull("identity") ? null : json.optString("identity", null); }
    }

    /** A JSON list as a Java array of `element` (nested `depth` deep); null when absent. */
    static Object array(JSONArray list, int depth, String element) {
        if (list == null) return null;
        int n = list.length();
        if (depth > 1) {
            Object[] out = (Object[]) java.lang.reflect.Array.newInstance(arrayClass(element, depth - 1), n);
            for (int i = 0; i < n; i++) out[i] = array(list.optJSONArray(i), depth - 1, element);
            return out;
        }
        switch (element) {
            case "float": { float[] out = new float[n]; for (int i = 0; i < n; i++) out[i] = (float) list.optDouble(i); return out; }
            case "double": { double[] out = new double[n]; for (int i = 0; i < n; i++) out[i] = list.optDouble(i); return out; }
            case "long": { long[] out = new long[n]; for (int i = 0; i < n; i++) out[i] = list.optLong(i); return out; }
            case "boolean": { boolean[] out = new boolean[n]; for (int i = 0; i < n; i++) out[i] = list.optBoolean(i); return out; }
            case "String": { String[] out = new String[n]; for (int i = 0; i < n; i++) out[i] = list.isNull(i) ? null : list.optString(i); return out; }
            case "Object": { Object[] out = new Object[n]; for (int i = 0; i < n; i++) out[i] = list.opt(i); return out; }
            default: {
                Object[] out = (Object[]) java.lang.reflect.Array.newInstance(arrayClass(element, 0), n);
                for (int i = 0; i < n; i++) out[i] = wrap(element, list.optJSONObject(i));
                return out;
            }
        }
    }

    private static Class<?> arrayClass(String element, int depth) {
        Class<?> base;
        switch (element) {
            case "float": base = float.class; break;
            case "double": base = double.class; break;
            case "long": base = long.class; break;
            case "boolean": base = boolean.class; break;
            case "String": base = String.class; break;
            case "Object": base = Object.class; break;
            default: base = typeClass(element);
        }
        for (int i = 0; i < depth; i++) base = java.lang.reflect.Array.newInstance(base, 0).getClass();
        return base;
    }

    private static Class<?> typeClass(String name) {
        switch (name) {
            case "FrameSnapshot": return FrameSnapshot.class;
            case "ImageSource": return ImageSource.class;
            case "FrameMetrics": return FrameMetrics.class;
            case "FrameCost": return FrameCost.class;
            case "FeedbackStyle": return FeedbackStyle.class;
            case "Node": return Node.class;
            case "Clip": return Clip.class;
            case "Scroll": return Scroll.class;
            case "InputGeometry": return InputGeometry.class;
            case "Slider": return Slider.class;
            case "Plot": return Plot.class;
            case "PieLayout": return PieLayout.class;
            case "Select": return Select.class;
            case "SelectOption": return SelectOption.class;
            case "Input": return Input.class;
            case "Edit": return Edit.class;
            case "Ime": return Ime.class;
            case "Picker": return Picker.class;
            case "SemanticNode": return SemanticNode.class;
            case "SemanticRange": return SemanticRange.class;
            case "TextField": return TextField.class;
            case "ScrollPosition": return ScrollPosition.class;
            case "SemanticAction": return SemanticAction.class;
            case "AutomationRequest": return AutomationRequest.class;
            case "AutomationAction": return AutomationAction.class;
            case "AutomationElement": return AutomationElement.class;
            case "AutomationQueryAll": return AutomationQueryAll.class;
            case "AutomationFrame": return AutomationFrame.class;
            case "QueueTimes": return QueueTimes.class;
            case "EvalTimes": return EvalTimes.class;
            case "AutomationRect": return AutomationRect.class;
            case "AutomationTree": return AutomationTree.class;
            case "AutomationTreeNode": return AutomationTreeNode.class;
            case "InputReply": return InputReply.class;
            case "EditorState": return EditorState.class;
            case "CopyReply": return CopyReply.class;
            case "SemanticRoots": return SemanticRoots.class;
            case "TextMenu": return TextMenu.class;
            case "TextMenuTarget": return TextMenuTarget.class;
            default: throw new IllegalArgumentException(name);
        }
    }

    private static Object wrap(String name, JSONObject json) {
        if (json == null) return null;
        switch (name) {
            case "FrameSnapshot": return new FrameSnapshot(json);
            case "ImageSource": return new ImageSource(json);
            case "FrameMetrics": return new FrameMetrics(json);
            case "FrameCost": return new FrameCost(json);
            case "FeedbackStyle": return new FeedbackStyle(json);
            case "Node": return new Node(json);
            case "Clip": return new Clip(json);
            case "Scroll": return new Scroll(json);
            case "InputGeometry": return new InputGeometry(json);
            case "Slider": return new Slider(json);
            case "Plot": return new Plot(json);
            case "PieLayout": return new PieLayout(json);
            case "Select": return new Select(json);
            case "SelectOption": return new SelectOption(json);
            case "Input": return new Input(json);
            case "Edit": return new Edit(json);
            case "Ime": return new Ime(json);
            case "Picker": return new Picker(json);
            case "SemanticNode": return new SemanticNode(json);
            case "SemanticRange": return new SemanticRange(json);
            case "TextField": return new TextField(json);
            case "ScrollPosition": return new ScrollPosition(json);
            case "SemanticAction": return new SemanticAction(json);
            case "AutomationRequest": return new AutomationRequest(json);
            case "AutomationAction": return new AutomationAction(json);
            case "AutomationElement": return new AutomationElement(json);
            case "AutomationQueryAll": return new AutomationQueryAll(json);
            case "AutomationFrame": return new AutomationFrame(json);
            case "QueueTimes": return new QueueTimes(json);
            case "EvalTimes": return new EvalTimes(json);
            case "AutomationRect": return new AutomationRect(json);
            case "AutomationTree": return new AutomationTree(json);
            case "AutomationTreeNode": return new AutomationTreeNode(json);
            case "InputReply": return new InputReply(json);
            case "EditorState": return new EditorState(json);
            case "CopyReply": return new CopyReply(json);
            case "SemanticRoots": return new SemanticRoots(json);
            case "TextMenu": return new TextMenu(json);
            case "TextMenuTarget": return new TextMenuTarget(json);
            default: throw new IllegalArgumentException(name);
        }
    }
}
