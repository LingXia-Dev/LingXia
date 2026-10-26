package com.lingxia.ui.generated;

import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;
import static org.junit.Assert.*;

/** Replays the golden frame vectors the Rust host records through the generated decoder. */
@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 28)
public class GpuFrameVectorsTest {
    /** Every object the decoder touched, so unread wire keys show up as decoder gaps. */
    static final List<Recording> seen = new ArrayList<>();

    static final class Recording extends JSONObject {
        final Set<String> read = new LinkedHashSet<>();
        Recording(JSONObject source) throws JSONException { super(source.toString()); seen.add(this); }
        @Override public boolean isNull(String key) { read.add(key); return super.isNull(key); }
        @Override public Object opt(String key) { read.add(key); return super.opt(key); }
        @Override public JSONObject optJSONObject(String key) { read.add(key); return record(super.optJSONObject(key)); }
        @Override public JSONArray optJSONArray(String key) { read.add(key); return record(super.optJSONArray(key)); }
    }

    static final class RecordingArray extends JSONArray {
        RecordingArray(JSONArray source) throws JSONException { super(source.toString()); }
        @Override public JSONObject optJSONObject(int index) { return record(super.optJSONObject(index)); }
        @Override public JSONArray optJSONArray(int index) { return record(super.optJSONArray(index)); }
    }

    static Recording record(JSONObject o) {
        try { return o == null ? null : new Recording(o); } catch (JSONException e) { throw new AssertionError(e); }
    }

    static RecordingArray record(JSONArray a) {
        try { return a == null ? null : new RecordingArray(a); } catch (JSONException e) { throw new AssertionError(e); }
    }

    static JSONObject vectors() throws Exception {
        byte[] bytes = Files.readAllBytes(Paths.get("../../../packages/lingxia-ui/contracts/vectors/frame.json"));
        return new JSONObject(new String(bytes, StandardCharsets.UTF_8));
    }

    /** Calls every getter, recursing into wrappers and arrays of wrappers. */
    static void walk(Object value) throws Exception {
        if (value == null) return;
        if (value.getClass().isArray()) {
            if (!value.getClass().getComponentType().isPrimitive())
                for (Object item : (Object[]) value) walk(item);
            return;
        }
        if (value.getClass().getEnclosingClass() != GpuFrame.class) return;
        for (Method method : value.getClass().getDeclaredMethods())
            if (Modifier.isPublic(method.getModifiers()) && method.getParameterCount() == 0) walk(method.invoke(value));
    }

    static void assertAllRead() {
        for (Recording object : seen) {
            for (Iterator<String> keys = object.keys(); keys.hasNext(); ) {
                String key = keys.next();
                assertTrue("decoder never reads `" + key + "` in " + object, object.read.contains(key));
            }
        }
        seen.clear();
    }

    @org.junit.Before public void forgetEarlierReads() { seen.clear(); }

    @Test public void snapshotDecodesEveryField() throws Exception {
        GpuFrame.FrameSnapshot snapshot = new GpuFrame.FrameSnapshot(record(vectors().getJSONObject("snapshot")));
        walk(snapshot);
        assertAllRead();
        GpuFrame.Node name = null;
        for (GpuFrame.Node node : snapshot.nodes()) if ("name".equals(node.testId())) name = node;
        assertNotNull(name);
        assertEquals(GpuFrame.KEYBOARD_EMAIL, name.input().keyboard());
        assertEquals(4, name.bounds().length);
        seen.clear();
    }

    @Test public void semanticNodesDecodeEveryField() throws Exception {
        JSONArray nodes = vectors().getJSONArray("semanticNodes");
        assertTrue(nodes.length() > 8);
        for (int i = 0; i < nodes.length(); i++) walk(new GpuFrame.SemanticNode(record(nodes.getJSONObject(i))));
        assertAllRead();
    }

    @Test public void automationExchangesDecodeEveryField() throws Exception {
        JSONArray exchanges = vectors().getJSONArray("automation");
        assertTrue(exchanges.length() > 0);
        for (int i = 0; i < exchanges.length(); i++) {
            JSONObject exchange = exchanges.getJSONObject(i);
            walk(new GpuFrame.AutomationRequest(record(exchange.getJSONObject("request"))));
            String reply = exchange.optString("reply", null);
            if (reply == null) continue;
            JSONObject value = record(exchange.getJSONObject("value"));
            switch (reply) {
                case "automationElement": walk(new GpuFrame.AutomationElement(value)); break;
                case "automationQueryAll": walk(new GpuFrame.AutomationQueryAll(value)); break;
                case "automationTree": walk(new GpuFrame.AutomationTree(value)); break;
                case "automationFrame": walk(new GpuFrame.AutomationFrame(value)); break;
                default: fail("unknown reply " + reply);
            }
        }
        assertAllRead();
    }

    @Test public void inputRepliesCopyRootsAndTextMenuDecodeEveryField() throws Exception {
        JSONObject vectors = vectors();
        JSONArray replies = vectors.getJSONArray("inputReplies");
        boolean token = false, selection = false;
        for (int i = 0; i < replies.length(); i++) {
            walk(new GpuFrame.InputReply(record(replies.getJSONObject(i))));
            GpuFrame.InputReply reply = new GpuFrame.InputReply(replies.getJSONObject(i));
            token |= reply.token() != null && reply.state() != null && reply.state().focused();
            selection |= reply.selection() != null && reply.selection().length == 2;
        }
        assertTrue("a focus or edit reply carries its token and state", token);
        assertTrue("a hit test reply carries its selection", selection);
        walk(new GpuFrame.CopyReply(record(vectors.getJSONObject("copySelection"))));
        walk(new GpuFrame.SemanticRoots(record(vectors.getJSONObject("semanticRoots"))));
        walk(new GpuFrame.TextMenu(record(vectors.getJSONObject("textMenu"))));
        assertAllRead();
        assertTrue(new GpuFrame.SemanticRoots(vectors.getJSONObject("semanticRoots")).roots().length > 0);
        GpuFrame.TextMenu menu = new GpuFrame.TextMenu(vectors.getJSONObject("textMenu"));
        assertEquals(GpuFrame.TEXT_MENU_TARGET_EDITOR, menu.target().kind());
        assertEquals(GpuFrame.MENU_ACTION_SELECT_ALL, menu.actions()[0]);
        assertEquals(4, menu.anchor().length);
    }
}
