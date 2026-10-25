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
        final ArrayList<String> frames = new ArrayList<>(), fetched = new ArrayList<>(), events = new ArrayList<>();
        final ArrayList<int[]> pixels = new ArrayList<>();
        boolean vsync;
        int loaded, presented;
        public void post(long task, long delay) { if (delay == 0) tasks.add(task); else synchronized (delayed) { delayed.add(task); } }
        public void requestVsync() { vsync = true; }
        public String environment() {
            return "{\"width\":200,\"height\":300,\"scale\":2,\"env\":{\"theme\":\"light\",\"insets\":{\"top\":0,\"right\":0,\"bottom\":0,\"left\":0},"
                + "\"keyboard\":{\"height\":0},\"os\":\"android\",\"pointer\":\"coarse\",\"formFactor\":\"phone\",\"displayLanguage\":\"en\",\"aside\":false,\"runner\":false}}";
        }
        public boolean presentPixels(int[] argb, int width, int height) { assertEquals(argb.length, width * height); pixels.add(argb); return true; }
        public void gpuAttached(boolean attached) { }
        public void fetchImage(long id, String source) { fetched.add(id + " " + source); }
        public void cancelImage(long id) { events.add("cancel " + id); }
        public void announce(int event, int id) { events.add("announce " + event); }
        public void announceText(int id, boolean secure, String before, String after) { }
        public void announceSelection(int id, boolean secure, String text, int start, int end) { }
        public void announceClicked() { events.add("announce-clicked"); }
        public void frame(String snapshot, float scale) { frames.add(snapshot); }
        public void imeFocus(int node, long commit, boolean touch, float x, float y, boolean word) { }
        public void imeActivation() { }
        public void imeReset() { }
        public void inputReply(long callback, String reply) { }
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
        public void editText(int node, String text, int start, int end) { }
        public void loaded() { loaded++; }
        public void presented() { presented++; }
        public void message(long connection, String message) { }
        public boolean restart(long connection) { return false; }
        public void recovery(int message, int action, boolean retained, String error) { if (message >= 0) fail(error); }
        public void bootstrapOverdue(boolean overdue) { }

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
            synchronized (delayed) { for (long task : delayed) GpuView.nativeRunTask(task); delayed.clear(); }
        }
    }

    private static JSONObject last(Fake fake) throws Exception { return new JSONObject(fake.frames.get(fake.frames.size() - 1)); }
    private static String text(JSONObject frame, String id) {
        JSONArray nodes = frame.optJSONArray("nodes");
        for (int i = 0; i < nodes.length(); i++) if (id.equals(nodes.optJSONObject(i).optString("test_id"))) return nodes.optJSONObject(i).optString("text");
        return null;
    }

    @Test public void presentedFramesDriveTapsSemanticsAndImagesThroughTheHost() throws Exception {
        Fake fake = new Fake();
        long host = GpuView.nativeHostCreate(fake, "test:jvm", false);
        GpuView.nativeLoad(host, GpuNative.module(
            "function render(){const n=useLocal('n',0);return h('column',{},h('button',{testId:'add',onClick:()=>n.set(n.value+1),style:{height:60}},'Count '+n.value),h('image',{src:'lx://pic.png',style:{width:20,height:20}}))}"));
        fake.pump(host, () -> fake.loaded == 1);
        GpuView.nativeSetVisible(host, true);
        fake.pump(host, () -> fake.presented == 1);
        assertEquals("Count 0", text(last(fake), "add"));
        assertEquals("device pixels", 200 * 300, fake.pixels.get(0).length);
        assertTrue(fake.events.contains("announce 6"));
        JSONObject roots = new JSONObject(GpuView.nativeSemanticRoots(host));
        int button = roots.getJSONArray("roots").getInt(0);
        JSONObject node = new JSONObject(GpuView.nativeSemanticNode(host, button));
        assertEquals("button", node.getString("role"));
        assertEquals("Count 0", node.getString("name"));
        assertEquals(1, fake.fetched.size());
        long image = Long.parseLong(fake.fetched.get(0).split(" ")[0]);
        assertTrue(fake.fetched.get(0).endsWith("lx://pic.png"));
        GpuView.nativeImageLoaded(host, image, null);
        assertTrue(GpuView.nativePointer(host, 0, 40, 40, 8, 0, 0, 50));
        GpuView.nativePointer(host, 2, 40, 40, 8, 0, 0, 50);
        assertTrue(fake.events.contains("clicked"));
        fake.pump(host, () -> { try { return "Count 1".equals(text(last(fake), "add")); } catch (Exception e) { throw new AssertionError(e); } });
        assertTrue(fake.events.contains("announce-clicked"));
        assertEquals("Count 1", new JSONObject(GpuView.nativeSemanticNode(host, button)).getString("name"));
        GpuView.nativeHostRelease(host);
        fake.release();
    }
}
