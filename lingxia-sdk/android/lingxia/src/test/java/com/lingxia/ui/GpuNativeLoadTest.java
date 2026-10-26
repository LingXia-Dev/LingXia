package com.lingxia.ui;

import java.util.ArrayList;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;
import static org.junit.Assert.*;

/** The JNI adapter drives the real shared host: frames, input, semantics and images. */
@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 28)
public class GpuNativeLoadTest {
    @Before public void loadCore() { GpuNative.load(); }

    /** A platform whose "main thread" is the test thread. */
    static final class Fake implements GpuPlatform {
        final LinkedBlockingQueue<Long> tasks = new LinkedBlockingQueue<>();
        final ArrayList<Long> delayed = new ArrayList<>();
        final ArrayList<String> frames = new ArrayList<>(), events = new ArrayList<>();
        // CPU frames are drawn from the raster pool, not the host's thread.
        final java.util.List<int[]> pixels = java.util.Collections.synchronizedList(new ArrayList<>());
        boolean vsync;
        int loaded, presented;
        public void post(long task, long delay) { if (delay == 0) tasks.add(task); else synchronized (delayed) { delayed.add(task); } }
        public void requestVsync() { vsync = true; }
        public String environment() {
            return "{\"width\":200,\"height\":300,\"scale\":2,\"dark\":false,\"system_insets\":[0,0,0,0],\"chrome_insets\":[0,0,0,0],"
                + "\"keyboard\":0,\"smallest_width\":360,\"locale\":\"en\",\"fine_pointer\":false,\"animation_scale\":1,\"reduce_motion\":false,\"runner\":false}";
        }
        public boolean presentPixels(int[] argb, int width, int height) { assertEquals(argb.length, width * height); pixels.add(argb); return true; }
        public void gpuAttached(boolean attached) { }
        public void announce(int event, int id) { events.add("announce " + event); }
        public void announceText(int id, boolean secure, String before, String after) { }
        public void announceSelection(int id, boolean secure, String text, int start, int end) { }
        public void announceClicked() { events.add("announce-clicked"); }
        public void announceLive(int id, String text, boolean assertive) { events.add("live:" + (assertive ? "assertive:" : "polite:") + text); }
        public void frame(String snapshot, float scale) { frames.add(snapshot); }
        public void imeFocus(int node, long commit, boolean touch, float x, float y, boolean word) { }
        public void imeActivation() { }
        public void imeReset() { }
        public void inputReply(long callback, String reply) { events.add("reply:" + reply); }
        public void press(int node) { events.add("press"); }
        public void clearPress() { events.add("clear-press"); }
        public void focusRing(int node) { }
        public void clearFocusRing() { }
        public void claim(boolean exclusive) { }
        public void armLongPress() { }
        public void cancelLongPress() { }
        public void fling(String region, boolean horizontal, float velocity) { }
        public void stopFling() { }
        public void clicked() { events.add("clicked"); }
        public void haptic() { }
        public int editText(int node, String text, int start, int end) { events.add("edit:" + text); return 0; }
        public void loaded() { loaded++; }
        public void presented() { presented++; }
        public void message(long connection, String message) { }
        public boolean restart(long connection) { return false; }
        public void recovery(int message, int action, boolean retained, String error) { if (message >= 0) fail(error); }
        public void bootstrapOverdue(boolean overdue) { }
        final ArrayList<String> copied = new ArrayList<>();
        public void copyText(String text) { copied.add(text); }
        public void revealed(boolean hold) { }
        final ArrayList<String> menus = new ArrayList<>();
        public void textMenu(String menu) { menus.add(menu); }
        final ArrayList<Integer> pickers = new ArrayList<>();
        public void openPicker(int node) { pickers.add(node); }

        /** Run posted work and vsyncs until {@code done} holds. */
        void pump(long host, java.util.function.BooleanSupplier done) throws InterruptedException {
            long deadline = System.currentTimeMillis() + 20000;
            while (!done.getAsBoolean()) {
                assertTrue("host deadline", System.currentTimeMillis() < deadline);
                Long task = tasks.poll(20, TimeUnit.MILLISECONDS);
                if (task != null) GpuView.nativeRunTask(task);
                else if (vsync) { vsync = false; GpuView.nativeVsync(host); }
            }
        }
        void release() {
            // Every posted task runs exactly once so its native box is freed.
            for (Long task; (task = tasks.poll()) != null;) GpuView.nativeRunTask(task);
            // A task may post another (a retry timer), so drain until none is left.
            while (true) {
                ArrayList<Long> due;
                synchronized (delayed) { if (delayed.isEmpty()) break; due = new ArrayList<>(delayed); delayed.clear(); }
                for (long task : due) GpuView.nativeRunTask(task);
                for (Long task; (task = tasks.poll()) != null;) GpuView.nativeRunTask(task);
            }
        }
    }

    private static JSONObject last(Fake fake) throws Exception { return new JSONObject(fake.frames.get(fake.frames.size() - 1)); }
    private static String text(JSONObject frame, String id) {
        JSONArray nodes = frame.optJSONArray("nodes");
        for (int i = 0; i < nodes.length(); i++) if (id.equals(nodes.optJSONObject(i).optString("test_id"))) return nodes.optJSONObject(i).optString("text");
        return null;
    }

    /** Images load in Rust; without an image owner the host settles them as failed, with no Java call. */
    @Test public void hostPushesReachTheViewEnvironment() throws Exception {
        Fake fake = new Fake();
        long host = GpuView.nativeHostCreate(fake, "test:jvm", false, 0);
        try {
            GpuView.nativeLoad(host, GpuNative.module(
                "function render(){const e=useView().env;return h('text',{testId:'env'},e.canDock+' '+e.sizeClass+' '+e.displayLanguage)}"));
            GpuView.nativeSetVisible(host, true);
            fake.pump(host, () -> fake.presented >= 1);
            assertEquals("false compact en", text(last(fake), "env"));
            GpuView.nativeUpdateEnvironment(host, "{\"kind\":\"surface\",\"revision\":2,\"context\":{\"sizeClass\":\"regular\",\"canDock\":true,\"width\":800,\"height\":600}}");
            GpuView.nativeUpdateEnvironment(host, "{\"kind\":\"language\",\"revision\":3,\"language\":\"zh-Hans-CN\"}");
            GpuView.nativeUpdateEnvironment(host, "{\"kind\":\"surface\",\"revision\":1,\"context\":{\"sizeClass\":\"compact\",\"canDock\":false,\"width\":1,\"height\":1}}");
            fake.pump(host, () -> { try { return "true regular zh-Hans-CN".equals(text(last(fake), "env")); } catch (Exception e) { throw new AssertionError(e); } });
        } finally { GpuView.nativeHostRelease(host); }
    }

    @Test public void presentedFramesDriveTapsAndSemanticsThroughTheHost() throws Exception {
        Fake fake = new Fake();
        long host = GpuView.nativeHostCreate(fake, "test:jvm", false, 0);
        GpuView.nativeLoad(host, GpuNative.module(
            "function render(){const n=useLocal('n',0);return h('column',{},h('button',{testId:'add',onClick:()=>n.set(n.value+1),style:{height:60}},'Count '+n.value),h('image',{src:'lx://pic.png',style:{width:20,height:20}}))}"));
        fake.pump(host, () -> fake.loaded == 1);
        GpuView.nativeSetVisible(host, true);
        fake.pump(host, () -> fake.presented == 1 && !fake.pixels.isEmpty());
        assertEquals("Count 0", text(last(fake), "add"));
        assertEquals("device pixels", 200 * 300, fake.pixels.get(0).length);
        assertTrue(fake.events.contains("announce 6"));
        JSONObject roots = new JSONObject(GpuView.nativeSemanticRoots(host));
        int button = roots.getJSONArray("roots").getInt(0);
        JSONObject node = new JSONObject(GpuView.nativeSemanticNode(host, button));
        assertEquals("button", node.getString("role"));
        assertEquals("Count 0", node.getString("name"));
        assertTrue(GpuView.nativePointer(host, 0, 40, 40, 8, 0, 0, 50));
        GpuView.nativePointer(host, 2, 40, 40, 8, 0, 0, 50);
        assertTrue(fake.events.contains("clicked"));
        fake.pump(host, () -> { try { return "Count 1".equals(text(last(fake), "add")); } catch (Exception e) { throw new AssertionError(e); } });
        assertTrue(fake.events.contains("announce-clicked"));
        assertEquals("Count 1", new JSONObject(GpuView.nativeSemanticNode(host, button)).getString("name"));
        GpuView.nativeHostRelease(host);
        fake.release();
    }

    /** Core routes Select and Copy on static text; Java only writes the clipboard. */
    @Test public void accessibilityCopiesStaticTextThroughTheCore() throws Exception {
        Fake fake = new Fake();
        long host = GpuView.nativeHostCreate(fake, "test:jvm", false, 0);
        try {
            GpuView.nativeLoad(host, GpuNative.module(
                "function render(){return h('text',{testId:'p',style:{userSelect:'text'}},'hello brave world')}"));
            GpuView.nativeSetVisible(host, true);
            fake.pump(host, () -> fake.presented >= 1);
            int id = new JSONObject(GpuView.nativeSemanticRoots(host)).getJSONArray("roots").getInt(0);
            assertFalse("nothing selected", GpuView.nativeAccessibility(host, id, GpuAccessibility.COPY, 0, ""));
            assertTrue(GpuView.nativeAccessibility(host, id, GpuAccessibility.SET_SELECTION, 6, "11"));
            fake.pump(host, () -> { try { return !new JSONObject(GpuView.nativeSemanticNode(host, id)).isNull("text_selection"); } catch (Exception e) { throw new AssertionError(e); } });
            String menu = fake.menus.get(fake.menus.size() - 1);
            assertTrue("the core shows the static-text menu: " + menu, menu.contains("\"copy\"") && menu.contains("\"select_all\""));
            assertTrue(GpuView.nativeAccessibility(host, id, GpuAccessibility.COPY, 0, ""));
            fake.pump(host, () -> !fake.copied.isEmpty());
            assertEquals("brave", fake.copied.get(0));
            assertEquals(GpuAccessibility.COPY, GpuAccessibility.action(android.view.accessibility.AccessibilityNodeInfo.ACTION_COPY));
        } finally { GpuView.nativeHostRelease(host); fake.release(); }
    }

    /** A tapped `<select>` asks Java for its picker; the choice answers through the input protocol. */
    @Test public void aSelectOpensThePlatformPickerAndTakesTheChoice() throws Exception {
        Fake fake = new Fake();
        long host = GpuView.nativeHostCreate(fake, "test:jvm", false, 0);
        try {
            GpuView.nativeLoad(host, GpuNative.module(
                "function render(){const plan=useLocal('plan','m');return h('column',{},h('select',{testId:'s',bind:plan,accessibilityLabel:'Plan',options:[{value:'m',label:'Monthly'},{value:'y',label:'Yearly'}]}),h('text',{testId:'t'},plan.value))}"));
            GpuView.nativeSetVisible(host, true);
            fake.pump(host, () -> fake.presented >= 1);
            int box = new JSONObject(GpuView.nativeSemanticRoots(host)).getJSONArray("roots").getInt(0);
            assertEquals("combo-box", new JSONObject(GpuView.nativeSemanticNode(host, box)).getString("role"));
            assertTrue(GpuView.nativeAccessibility(host, box, GpuAccessibility.CLICK, 0, ""));
            fake.pump(host, () -> !fake.pickers.isEmpty());
            JSONObject frame = last(fake);
            JSONObject node = frame.getJSONArray("nodes").getJSONObject(fake.pickers.get(0));
            String request = new JSONObject().put("op", "select").put("identity", node.getString("identity"))
                .put("generation", frame.getLong("generation")).put("index", 1).toString();
            GpuView.nativeInput(host, request, 9);
            fake.pump(host, () -> fake.events.stream().anyMatch(e -> e.startsWith("reply:")));
            GpuView.nativeRequestFrame(host);
            fake.pump(host, () -> { try { return "y".equals(text(last(fake), "t")); } catch (Exception e) { throw new AssertionError(e); } });
        } finally { GpuView.nativeHostRelease(host); fake.release(); }
    }
}
