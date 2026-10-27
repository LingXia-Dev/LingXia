package com.lingxia.ui.tests;

import android.app.UiAutomation;
import android.graphics.Rect;
import android.os.Bundle;
import android.view.accessibility.AccessibilityNodeInfo;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;
import android.app.Activity;
import android.view.View;
import android.view.ViewGroup;
import androidx.test.runner.lifecycle.ActivityLifecycleMonitorRegistry;
import androidx.test.runner.lifecycle.Stage;
import com.lingxia.ui.GpuView;
import java.io.FileWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.concurrent.atomic.AtomicReference;
import org.json.JSONArray;
import org.json.JSONObject;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.junit.Test;
import org.junit.runner.RunWith;
import static org.junit.Assert.*;

/**
 * Pages through view-lab: a screenshot per scroll page and one line per accessibility node as a
 * service reads it. With `-e talkback 1` each node also takes TalkBack focus and its speech caption
 * is saved; `-e rotation 1` runs in landscape; `-e tab <label>` and `-e group <label>` walk another
 * tab or view-lab group. Output: the host's external files, `walk-<tag>*`.
 */
@RunWith(AndroidJUnit4.class)
public class GpuViewLabWalk {
    private final A11yDriver driver = new A11yDriver(InstrumentationRegistry.getInstrumentation());

    private static void visible(AccessibilityNodeInfo node, String pkg, List<AccessibilityNodeInfo> out) {
        if (node == null) return;
        if (node.isVisibleToUser() && pkg.contentEquals(node.getPackageName())
                && (node.getText() != null && node.getText().length() > 0 || node.getContentDescription() != null
                    || node.getViewIdResourceName() != null || node.isClickable() || node.isScrollable())) out.add(node);
        for (int i = 0; i < node.getChildCount(); i++) visible(node.getChild(i), pkg, out);
    }

    private static String describe(AccessibilityNodeInfo node) {
        Rect bounds = new Rect();
        node.getBoundsInScreen(bounds);
        StringBuilder line = new StringBuilder();
        line.append(node.getClassName()).append(" \"").append(node.getText()).append('"');
        if (node.getContentDescription() != null) line.append(" desc=\"").append(node.getContentDescription()).append('"');
        if (node.getViewIdResourceName() != null) line.append(" id=").append(node.getViewIdResourceName().replaceAll(".*:id/", ""));
        if (node.isHeading()) line.append(" heading");
        CharSequence role = node.getExtras().getCharSequence("AccessibilityNodeInfo.roleDescription");
        if (role != null) line.append(" roleDescription=\"").append(role).append('"');
        if (node.getCollectionInfo() != null) line.append(" rows=").append(node.getCollectionInfo().getRowCount());
        if (node.getCollectionItemInfo() != null) line.append(" row=").append(node.getCollectionItemInfo().getRowIndex());
        if (node.getLiveRegion() != 0) line.append(" live=").append(node.getLiveRegion());
        if (node.getPaneTitle() != null) line.append(" pane=\"").append(node.getPaneTitle()).append('"');
        if (node.isClickable()) line.append(" clickable");
        if (node.isScrollable()) line.append(" scrollable");
        if (node.isEditable()) line.append(" editable");
        if (node.isCheckable()) line.append(" checked=").append(node.isChecked());
        if (!node.isEnabled()) line.append(" disabled");
        if (node.getRangeInfo() != null) line.append(" range=").append(node.getRangeInfo().getCurrent());
        if (node.getHintText() != null) line.append(" hint=\"").append(node.getHintText()).append('"');
        line.append(' ').append(bounds.toShortString());
        return line.toString();
    }

    @Test public void walk() throws Exception {
        Bundle args = InstrumentationRegistry.getArguments();
        boolean talkback = "1".equals(args.getString("talkback"));
        boolean landscape = "1".equals(args.getString("rotation"));
        String tab = args.getString("tab", "Lab"), group = args.getString("group");
        String tag = (landscape ? "land" : "port") + ("Lab".equals(tab) ? "" : "-" + tab) + (group == null ? "" : "-" + group);
        driver.automation.setRotation(landscape ? UiAutomation.ROTATION_FREEZE_90 : UiAutomation.ROTATION_FREEZE_0);
        Thread.sleep(1500);
        driver.openTab(tab);
        if (group != null) driver.openGroup(group);
        String pkg = driver.instrumentation.getTargetContext().getPackageName();
        Set<String> seen = new HashSet<>();
        int caption = 0;
        try (FileWriter log = new FileWriter(driver.out("walk-" + tag + ".txt"))) {
            for (int page = 0; page < 40; page++) {
                driver.capture("walk-" + tag + "-" + page + ".png", 0, 1);
                log.write("# page " + page + "\n");
                List<AccessibilityNodeInfo> nodes = new ArrayList<>();
                visible(driver.root(), pkg, nodes);
                for (AccessibilityNodeInfo node : nodes) {
                    String line = describe(node);
                    String key = line.substring(0, line.lastIndexOf(' '));
                    if (!seen.add(key)) continue;
                    if (talkback && node.performAction(AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS)) {
                        Thread.sleep(900);
                        String name = "walk-" + tag + "-say-" + caption++ + ".png";
                        driver.capture(name, landscape ? 0.86f : 0.9f, 1);
                        line += " say=" + name;
                    }
                    log.write(line + "\n");
                }
                log.flush();
                AccessibilityNodeInfo scroll = A11yDriver.largestScrollable(driver.root());
                assertNotNull("page scroll", scroll);
                if (!scroll.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD)) break;
                Thread.sleep(800);
            }
        } finally {
            driver.automation.setRotation(UiAutomation.ROTATION_UNFREEZE);
        }
    }

    private static void gpus(View view, List<GpuView> out) {
        if (view instanceof GpuView) out.add((GpuView)view);
        if (view instanceof ViewGroup) for (int i = 0; i < ((ViewGroup)view).getChildCount(); i++) gpus(((ViewGroup)view).getChildAt(i), out);
    }

    /** Every projected node of view-lab, as the core serves it, one JSON per line: `walk-semantics.jsonl`. */
    @Test public void semantics() throws Exception {
        driver.openTab("Lab");
        AtomicReference<String> dump = new AtomicReference<>();
        AtomicReference<Exception> failure = new AtomicReference<>();
        driver.instrumentation.runOnMainSync(() -> {
            try {
                java.util.List<GpuView> views = new ArrayList<>();
                for (Activity activity : ActivityLifecycleMonitorRegistry.getInstance().getActivitiesInStage(Stage.RESUMED))
                    gpus(activity.getWindow().getDecorView(), views);
                assertFalse("GPU views", views.isEmpty());
                Field hostField = GpuView.class.getDeclaredField("host"); hostField.setAccessible(true);
                Method roots = GpuView.class.getDeclaredMethod("nativeSemanticRoots", long.class); roots.setAccessible(true);
                Method node = GpuView.class.getDeclaredMethod("nativeSemanticNode", long.class, int.class); node.setAccessible(true);
                StringBuilder out = new StringBuilder();
                for (GpuView view : views) {
                    long host = hostField.getLong(view);
                    String rootJson = (String)roots.invoke(null, host);
                    out.append("{\"view\":").append(views.indexOf(view)).append(",\"shown\":").append(view.isShown()).append("}\n");
                    if (rootJson == null || "null".equals(rootJson)) continue;
                    JSONArray pending = new JSONObject(rootJson).getJSONArray("roots");
                    for (int i = 0; i < pending.length(); i++) {
                        String json = (String)node.invoke(null, host, pending.getInt(i));
                        if ("null".equals(json)) { out.append("{\"missing\":").append(pending.getInt(i)).append("}\n"); continue; }
                        JSONObject found = new JSONObject(json);
                        out.append(found).append('\n');
                        JSONArray children = found.optJSONArray("children");
                        for (int c = 0; children != null && c < children.length(); c++) pending.put(children.getInt(c));
                    }
                }
                dump.set(out.toString());
            } catch (Exception error) { failure.set(error); }
        });
        if (failure.get() != null) throw failure.get();
        try (FileWriter log = new FileWriter(driver.out("walk-semantics.jsonl"))) { log.write(dump.get()); }
    }

    /** A locator click through the page shell's automation entry, as lx.automation sends it. */
    @Test public void automationClick() throws Exception {
        driver.openTab("Lab");
        driver.openGroup("Input");
        driver.reveal("lab-a11y-save");
        driver.instrumentation.runOnMainSync(() -> {
            try {
                Field registry = com.lingxia.lxapp.GpuPage.class.getDeclaredField("PAGES"); registry.setAccessible(true);
                Field content = com.lingxia.lxapp.GpuPage.class.getDeclaredField("content"); content.setAccessible(true);
                for (java.util.Map.Entry<?, ?> entry : ((java.util.Map<?, ?>)registry.get(null)).entrySet())
                    if (((View)content.get(entry.getValue())).isShown())
                        com.lingxia.lxapp.GpuPage.automate((Long)entry.getKey(),
                            "{\"op\":\"act\",\"css\":\"[data-testid=\\\"lab-a11y-save\\\"]\",\"action\":{\"kind\":\"click\"}}", -1);
            } catch (Exception error) { throw new IllegalStateException(error); }
        });
        Thread.sleep(3000);
        AccessibilityNodeInfo status = driver.node("lab-a11y-status");
        assertEquals("Saved 1 times", String.valueOf(status.getText()));
    }

    /** Sticky headers mid-scroll: a screenshot per inner scroll step with the frame's scroll offset and header bounds. */
    @Test public void sticky() throws Exception {
        driver.openTab("Lab");
        driver.openGroup("Layout");
        driver.reveal("lab-sticky-scroll");
        try (FileWriter log = new FileWriter(driver.out("sticky.txt"))) {
            for (int step = 0; step < 8; step++) {
                AtomicReference<String> frame = new AtomicReference<>();
                driver.instrumentation.runOnMainSync(() -> {
                    try {
                        java.util.List<GpuView> views = new ArrayList<>();
                        for (Activity activity : ActivityLifecycleMonitorRegistry.getInstance().getActivitiesInStage(Stage.RESUMED))
                            gpus(activity.getWindow().getDecorView(), views);
                        Field text = GpuView.class.getDeclaredField("snapshotText"); text.setAccessible(true);
                        for (GpuView view : views) if (view.isShown()) frame.set((String)text.get(view));
                    } catch (Exception error) { throw new IllegalStateException(error); }
                });
                JSONArray nodes = new JSONObject(frame.get()).getJSONArray("nodes");
                log.write("# step " + step + "\n");
                for (int i = 0; i < nodes.length(); i++) {
                    JSONObject node = nodes.getJSONObject(i);
                    String text = node.optString("text");
                    if ("lab-sticky-scroll".equals(node.optString("test_id")))
                        log.write("scroll " + node.opt("scroll") + " bounds=" + node.opt("bounds") + "\n");
                    else if (text.equals("Today") || text.equals("Yesterday") || text.equals("Last week"))
                        log.write(text + " bounds=" + node.opt("bounds") + " visible=" + node.opt("visible_bounds") + " clips=" + node.opt("clips") + "\n");
                }
                log.flush();
                driver.capture("sticky-" + step + ".png", 0, 1);
                AccessibilityNodeInfo scroll = driver.node("lab-sticky-scroll");
                scroll.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
                Thread.sleep(900);
            }
        }
    }

    /** Static text: select a range and copy it the way an accessibility service does; the clipboard gets that range. */
    @Test public void copyThroughAccessibility() throws Exception {
        driver.openTab("Lab");
        driver.node("lab-tab-text").performAction(AccessibilityNodeInfo.ACTION_CLICK);
        Thread.sleep(1500);
        AccessibilityNodeInfo text = driver.reveal("lab-select-all");
        Bundle range = new Bundle();
        range.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, 0);
        range.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, 4);
        assertTrue("set selection", text.performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, range));
        Thread.sleep(800);
        text = driver.node("lab-select-all");
        assertEquals(0, text.getTextSelectionStart());
        assertEquals(4, text.getTextSelectionEnd());
        assertTrue("copy offered", text.getActionList().contains(AccessibilityNodeInfo.AccessibilityAction.ACTION_COPY));
        assertTrue("copy", text.performAction(AccessibilityNodeInfo.ACTION_COPY));
        Thread.sleep(1000);
        AtomicReference<String> clip = new AtomicReference<>();
        driver.instrumentation.runOnMainSync(() -> {
            android.content.ClipboardManager clipboard = (android.content.ClipboardManager)driver.instrumentation.getTargetContext()
                .getSystemService(android.content.Context.CLIPBOARD_SERVICE);
            android.content.ClipData data = clipboard.getPrimaryClip();
            clip.set(data == null ? null : String.valueOf(data.getItemAt(0).getText()));
        });
        assertEquals("8F3A", clip.get());
        assertEquals("the selection goes after copy", -1, driver.node("lab-select-all").getTextSelectionStart());
    }

    /** An editor driven the way TalkBack's editing menu does: set text, select a range, copy, cut and paste. Writes the offered actions to `editor-actions.txt`. */
    @Test public void editorThroughAccessibility() throws Exception {
        driver.openTab("Lab");
        driver.openGroup("Input");
        AccessibilityNodeInfo editor = driver.reveal("lab-input-name");
        assertTrue("editable", editor.isEditable());
        Bundle text = new Bundle();
        text.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, "hello world");
        assertTrue("set text", editor.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, text));
        Thread.sleep(1000);
        editor = driver.node("lab-input-name");
        assertEquals("hello world", String.valueOf(editor.getText()));
        Bundle range = new Bundle();
        range.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, 6);
        range.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, 11);
        assertTrue("set selection", editor.performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, range));
        Thread.sleep(800);
        editor = driver.node("lab-input-name");
        assertEquals(6, editor.getTextSelectionStart());
        assertEquals(11, editor.getTextSelectionEnd());
        try (FileWriter log = new FileWriter(driver.out("editor-actions.txt"))) {
            for (AccessibilityNodeInfo.AccessibilityAction action : editor.getActionList()) log.write(action.getId() + " " + action.getLabel() + "\n");
        }
        assertTrue("copy", editor.performAction(AccessibilityNodeInfo.ACTION_COPY));
        Thread.sleep(1000);
        AtomicReference<String> clip = new AtomicReference<>();
        driver.instrumentation.runOnMainSync(() -> {
            android.content.ClipboardManager clipboard = (android.content.ClipboardManager)driver.instrumentation.getTargetContext()
                .getSystemService(android.content.Context.CLIPBOARD_SERVICE);
            android.content.ClipData data = clipboard.getPrimaryClip();
            clip.set(data == null ? null : String.valueOf(data.getItemAt(0).getText()));
        });
        assertEquals("world", clip.get());
        assertTrue("cut", driver.node("lab-input-name").performAction(AccessibilityNodeInfo.ACTION_CUT));
        Thread.sleep(1000);
        assertEquals("hello ", String.valueOf(driver.node("lab-input-name").getText()));
        Bundle start = new Bundle();
        start.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, 0);
        start.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, 0);
        assertTrue("caret", driver.node("lab-input-name").performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, start));
        Thread.sleep(800);
        assertTrue("paste", driver.node("lab-input-name").performAction(AccessibilityNodeInfo.ACTION_PASTE));
        Thread.sleep(1000);
        assertEquals("worldhello ", String.valueOf(driver.node("lab-input-name").getText()));
    }
}
