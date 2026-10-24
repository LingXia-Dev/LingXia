package com.lingxia.ui;

import android.view.View;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.annotation.Config;
import java.util.ArrayList;
import java.util.function.Consumer;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest=Config.NONE,sdk={23,31})
public class GpuImeTest {
    static class Host implements GpuIme.Host {
        final ArrayList<JSONObject> requests=new ArrayList<>();
        final ArrayList<Consumer<JSONObject>> replies=new ArrayList<>();
        boolean rebuild;
        int frames;
        int menus;
        public void showTextMenu() { menus++; }
        public void request(JSONObject value,Consumer<JSONObject> reply) { requests.add(value); replies.add(reply); }
        public void render(boolean rebuild) { this.rebuild|=rebuild; frames++; }
    }
    private static JSONObject state(String text) throws Exception {
        return new JSONObject().put("text",text).put("selection",new JSONArray(new int[]{text.length(),text.length()}));
    }
    private static JSONObject node(long token,String text) throws Exception {
        return new JSONObject().put("identity","editor").put("tag","input").put("input",new JSONObject()
            .put("enabled",true).put("keyboard","text").put("edit_token",token).put("edit",state(text)));
    }
    private static JSONObject frame(long token,String text) throws Exception { return new JSONObject().put("nodes",new JSONArray().put(node(token,text))); }
    private GpuIme ime(Host host) { return new GpuIme(new View(RuntimeEnvironment.getApplication()),host); }

    @Test public void multilineSubmitOptionUpdatesKeyboardActionAndPreservesMultiline() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host);
        JSONObject node=node(7,"hello").put("tag","textarea");
        JSONObject frame=new JSONObject().put("nodes",new JSONArray().put(node));
        ime.presented(frame,ime.capture()); EditorInfo info=new EditorInfo(); ime.create(info);
        assertEquals(EditorInfo.IME_ACTION_NONE,info.imeOptions & EditorInfo.IME_MASK_ACTION);
        node.getJSONObject("input").put("submit_on_enter",true);
        ime.presented(frame,ime.capture()); info=new EditorInfo(); InputConnection connection=ime.create(info);
        assertEquals(EditorInfo.IME_ACTION_SEND,info.imeOptions & EditorInfo.IME_MASK_ACTION);
        assertNotEquals(0,info.inputType & android.text.InputType.TYPE_TEXT_FLAG_MULTI_LINE);
        assertTrue(connection.performEditorAction(EditorInfo.IME_ACTION_SEND));
        assertTrue(host.requests.get(0).getBoolean("submit"));
        host.replies.get(0).accept(new JSONObject().put("token",7).put("state",state("hello")));
        assertTrue(ime.hasFocus());
    }

    @Test public void searchUsesSearchActionAndKeepsTheEditingConnectionAfterSubmit() throws Exception {
        Host host=new Host();GpuIme ime=ime(host);
        JSONObject node=node(7,"coffee").put("tag","search");node.getJSONObject("input").put("keyboard","search");
        ime.presented(new JSONObject().put("nodes",new JSONArray().put(node)),ime.capture());
        EditorInfo info=new EditorInfo();InputConnection connection=ime.create(info);
        assertEquals(EditorInfo.IME_ACTION_SEARCH,info.imeOptions & EditorInfo.IME_MASK_ACTION);
        assertEquals(0,info.inputType & android.text.InputType.TYPE_TEXT_FLAG_MULTI_LINE);
        assertTrue(connection.performEditorAction(EditorInfo.IME_ACTION_SEARCH));
        assertTrue(host.requests.get(0).getBoolean("submit"));
        host.replies.get(0).accept(new JSONObject().put("token",7).put("state",state("coffee")));
        assertTrue(ime.hasFocus());assertTrue(connection.commitText(" shop",1));
        assertEquals("coffee shop",ime.menuText());
    }

    @Test public void undoReplyInvalidatesOldImeWrapperAndCancelledReplyCannotEditReplacement() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host); JSONObject initial=frame(7,"hello");
        initial.getJSONArray("nodes").getJSONObject(0).getJSONObject("input").getJSONObject("edit").put("can_undo",true);
        ime.presented(initial,ime.capture()); InputConnection old=ime.create(new EditorInfo());
        assertTrue(old.performContextMenuAction(android.R.id.undo));
        host.replies.get(0).accept(new JSONObject().put("token",7).put("state",state("").put("can_redo",true)));
        assertFalse(old.commitText("late composition",1)); assertEquals("",ime.menuText());
        InputConnection fresh=ime.create(new EditorInfo()); assertTrue(fresh.performContextMenuAction(android.R.id.redo));
        ime.reset(); ime.presented(frame(8,"other"),ime.capture());
        host.replies.get(1).accept(new JSONObject().put("token",7).put("state",state("hello")));
        assertEquals("other",ime.menuText()); assertFalse(fresh.commitText("late",1));
        assertTrue(ime.create(new EditorInfo()).commitText("!",1)); assertEquals("other!",ime.menuText());
    }

    @Test public void accessibilityReplacementIsAtomicAndSelectionRejectsSplitSurrogates() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host); JSONObject node=node(7,"a😀b");
        ime.presented(frame(7,"a😀b"),ime.capture());
        assertFalse(ime.accessibilityEdit(node,12,null,2,3));
        assertFalse(ime.accessibilityEdit(node,12,"\ud800",0,0));
        assertTrue(host.requests.isEmpty());
        assertTrue(ime.accessibilityEdit(node,12,"你好",0,0));
        JSONArray edits=host.requests.get(0).getJSONArray("edits");
        assertEquals(3,edits.length()); assertEquals("finish_composition",edits.getJSONObject(0).getString("op"));
        assertEquals(4,edits.getJSONObject(1).getInt("focus"));
        assertEquals("你好",edits.getJSONObject(2).getString("text"));
        assertEquals("你好",ime.menuText());
        assertFalse(ime.accessibilityEdit(node,12,null,0,1)); // An unacknowledged mirror cannot be retargeted.
        assertFalse(ime.accessibilityEdit(node(0,"").put("identity","other"),12,"other",0,0));
        host.replies.get(0).accept(new JSONObject().put("token",7).put("state",state("你好")));
        assertTrue(ime.accessibilityEdit(node,12,null,0,1));
        assertEquals("你",ime.create(new EditorInfo()).getSelectedText(0));
    }
    @Test public void accessibilityReplacementInvalidatesKeyboardComposition() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host);
        ime.presented(frame(7,"old"),ime.capture());
        InputConnection old=ime.create(new EditorInfo());
        assertTrue(ime.accessibilityEdit(node(7,"old"),12,"100",0,0));
        assertFalse(old.commitText("100",1));
        assertFalse(old.setComposingText("",1));
        assertEquals("100",ime.menuText());
        host.replies.get(0).accept(new JSONObject().put("token",7).put("state",state("100")));
        InputConnection fresh=ime.create(new EditorInfo());
        assertEquals("100",fresh.getTextBeforeCursor(10,0));
        assertTrue(fresh.commitText("!",1));
        assertEquals("100!",ime.menuText());
    }
    @Test public void accessibilityReplacementCannotPartiallyReserveABatch() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host);
        ime.presented(frame(7,"keep"),ime.capture());
        GpuInputConnection connection=ime.capture().connection;
        connection.beginBatchEdit();
        for (int i=0;i<62;i++) assertTrue(connection.setSelection(0,0));
        assertFalse(connection.replaceAll("discard"));
        assertEquals("keep",ime.menuText()); assertEquals(0,connection.anchor());
        connection.endBatchEdit(); assertEquals(62,host.requests.get(0).getJSONArray("edits").length());
    }
    @Test public void accessibilityFocusInitializesTextBeforeAttachingKeyboard() throws Exception {
        Host host=new Host(); GpuIme[] owner=new GpuIme[1];
        ArrayList<String> initialText=new ArrayList<>();
        View view=new View(RuntimeEnvironment.getApplication()) {
            @Override public boolean requestFocus(int direction,android.graphics.Rect bounds) {
                initialText.add(owner[0].menuText());return super.requestFocus(direction,bounds);
            }
        };
        GpuIme ime=owner[0]=new GpuIme(view,host);
        assertTrue(ime.accessibilityEdit(node(0,""),12,"100",0,0));
        host.replies.get(0).accept(new JSONObject().put("token",7).put("state",state("")));
        assertEquals(java.util.Collections.singletonList("100"),initialText);
        assertEquals("100",ime.create(new EditorInfo()).getTextBeforeCursor(10,0));
    }
    @Test public void accessibilityReplacementCanFocusAndCancelledFocusCannotEdit() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host); JSONObject node=node(0,"");
        assertTrue(ime.accessibilityEdit(node,12,"hello",0,0));
        assertEquals("focus",host.requests.get(0).getString("op"));
        host.replies.get(0).accept(new JSONObject().put("token",7).put("state",state("")));
        assertEquals("hello",ime.menuText()); assertEquals("edit",host.requests.get(1).getString("op"));
        ime.clear(false);
        assertTrue(ime.accessibilityEdit(node,13,"discard",0,0));
        int pending=host.requests.size()-1; ime.clear(false);
        host.replies.get(pending).accept(new JSONObject().put("token",8).put("state",state("")));
        assertFalse(ime.hasFocus()); assertEquals("blur",host.requests.get(host.requests.size()-1).getString("op"));
        ime.setCaretActive(false); assertFalse(ime.accessibilityEdit(node,13,"hidden",0,0));
    }
    @Test public void touchHitFinishesPreeditAndSelectsInOneBatch() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host);
        JSONObject edited=node(7,"hello world");
        edited.getJSONObject("input").getJSONObject("edit").put("composition",new JSONArray(new int[]{0,5}));
        ime.presented(new JSONObject().put("nodes",new JSONArray().put(edited)),ime.capture());
        InputConnection input=ime.create(new EditorInfo());
        ime.touch(edited,12,20,30,true);
        assertEquals("hit_test",host.requests.get(0).optString("op"));
        assertEquals(12,host.requests.get(0).optLong("generation"));
        assertEquals(20,host.requests.get(0).getJSONArray("point").getInt(0));
        host.replies.get(0).accept(new JSONObject().put("selection",new JSONArray(new int[]{0,5})));
        assertEquals("hello",input.getSelectedText(0));
        assertEquals(1,host.menus);
        JSONArray edits=host.requests.get(1).getJSONArray("edits");
        assertEquals(2,edits.length()); assertEquals("finish_composition",edits.getJSONObject(0).getString("op"));
        assertEquals("selection",edits.getJSONObject(1).getString("op"));
    }
    @Test public void touchQueriesCannotOverwriteTypingOrNewerTouchOrBlur() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host); JSONObject node=node(7,"hello");
        ime.presented(frame(7,"hello"),ime.capture()); InputConnection input=ime.create(new EditorInfo());
        JSONObject selection=new JSONObject().put("selection",new JSONArray(new int[]{0,2}));
        ime.touch(node,12,10,10,false); input.beginBatchEdit(); input.commitText("!",1);
        host.replies.get(0).accept(selection); assertNull(input.getSelectedText(0));
        input.endBatchEdit();
        int before=host.requests.size(); ime.touch(node,12,10,10,false); assertEquals(before,host.requests.size());
        host.replies.get(1).accept(new JSONObject().put("token",7).put("state",state("hello!")));
        ime.touch(node,13,10,10,false); ime.touch(node,13,30,10,true);
        host.replies.get(2).accept(selection); assertNull(input.getSelectedText(0));
        ime.clear(true); before=host.requests.size(); host.replies.get(3).accept(selection);
        assertEquals(before,host.requests.size()); assertFalse(ime.hasFocus());
    }
    @Test public void touchFocusUsesHitSelectionAndCancelledHitCannotFocus() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host); JSONObject node=node(0,"hello");
        JSONObject selection=new JSONObject().put("selection",new JSONArray(new int[]{2,2}));
        ime.touch(node,12,10,10,false); host.replies.get(0).accept(selection);
        assertEquals("focus",host.requests.get(1).getString("op"));
        host.replies.get(1).accept(new JSONObject().put("token",7).put("state",state("hello")));
        InputConnection input=ime.create(new EditorInfo()); assertEquals("he",input.getTextBeforeCursor(10,0));
        assertEquals("selection",host.requests.get(2).getJSONArray("edits").getJSONObject(0).getString("op"));
        ime.clear(true); int before=host.requests.size();
        ime.touch(node,13,10,10,false); ime.reset(); host.replies.get(before).accept(selection);
        assertFalse(ime.hasFocus());
    }
    @Test public void rejectedHitAndInvalidSelectionLeaveEditorUntouched() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host); JSONObject node=node(7,"a\ud83d\ude00b");
        ime.presented(frame(7,"a\ud83d\ude00b"),ime.capture()); InputConnection input=ime.create(new EditorInfo());
        ime.touch(node,12,10,10,false); host.replies.get(0).accept(null);
        assertEquals(1,host.requests.size()); assertEquals("a\ud83d\ude00b",input.getTextBeforeCursor(10,0));
        ime.touch(node,12,10,10,false);
        host.replies.get(1).accept(new JSONObject().put("selection",new JSONArray(new int[]{2,2})));
        assertEquals(2,host.requests.size()); assertEquals("a\ud83d\ude00b",input.getTextBeforeCursor(10,0));
        ime.touch(node,12,Float.NaN,10,false); assertEquals(2,host.requests.size());
        ime.touch(node,12,10,10,false); ime.setCaretActive(false); ime.setCaretActive(true);
        host.replies.get(2).accept(new JSONObject().put("selection",new JSONArray(new int[]{0,1})));
        assertEquals(3,host.requests.size()); assertNull(input.getSelectedText(0));
    }

    @Test @org.robolectric.annotation.LooperMode(org.robolectric.annotation.LooperMode.Mode.PAUSED)
    public void presentationsPreserveBlinkPhaseAndLifecycleStopsItsTimer() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host);
        ime.presented(frame(7,"hello"),ime.capture());
        org.robolectric.shadows.ShadowLooper looper=org.robolectric.Shadows.shadowOf(android.os.Looper.getMainLooper());
        looper.idleFor(java.time.Duration.ofMillis(500));
        assertFalse(ime.caretVisible()); assertEquals(1,host.frames);
        ime.presented(frame(7,"hello"),ime.capture());
        assertFalse(ime.caretVisible());
        looper.idleFor(java.time.Duration.ofMillis(500)); assertTrue(ime.caretVisible());
        ime.setCaretActive(false); int stopped=host.frames;
        looper.idleFor(java.time.Duration.ofSeconds(3)); assertEquals(stopped,host.frames);
        ime.setCaretActive(true); looper.idleFor(java.time.Duration.ofMillis(500));
        assertFalse(ime.caretVisible()); stopped=host.frames;
        ime.clear(true); assertEquals(stopped,host.frames);
        looper.idleFor(java.time.Duration.ofSeconds(3)); assertEquals(stopped,host.frames);
    }

    @Test public void replacingPlatformConnectionDoesNotCloseTheLiveEditor() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host);
        ime.presented(frame(7,""),ime.capture());
        assertTrue(ime.hasFocus());
        InputConnection first=ime.create(new EditorInfo()),second=ime.create(new EditorInfo());
        if (android.os.Build.VERSION.SDK_INT >= 24) first.closeConnection();
        else ((android.view.inputmethod.InputConnectionWrapper)first).getClass().getMethod("closeConnection").invoke(first);
        assertTrue(second.commitText("a",1));
        assertEquals(1,host.requests.size());
        first.commitText("stale",1); assertEquals(1,host.requests.size());
        host.replies.get(0).accept(new JSONObject().put("token",7).put("state",state("a")).put("needs_view_render",true));
        assertEquals("a",second.getTextBeforeCursor(10,0)); assertTrue(host.rebuild);
        ime.clear(true);
        assertEquals("blur",host.requests.get(1).optString("op"));
        assertFalse(second.commitText("late",1)); assertFalse(ime.hasFocus());
    }
    @Test public void staleFramesAndCancelledFocusCannotReopenKeyboard() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host);
        GpuIme.Ticket old=ime.capture();
        ime.focus(node(0,""),12);
        assertEquals(12,host.requests.get(0).optLong("generation"));
        ime.clear(true);
        host.replies.get(0).accept(new JSONObject().put("token",8).put("state",state("")));
        assertEquals("blur",host.requests.get(1).optString("op"));
        ime.presented(frame(8,""),old);
        ime.presented(frame(8,""),ime.capture());
        assertFalse(ime.hasFocus());
        ime.reset(); ime.presented(frame(9,""),ime.capture()); assertTrue(ime.hasFocus());
    }
    @Test public void frameRevisionProtectsTypingAndPasswordConfigIsExposed() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host);
        JSONObject node=node(7,""); node.optJSONObject("input").put("secure",true);
        ime.presented(new JSONObject().put("nodes",new JSONArray().put(node)),ime.capture());
        EditorInfo info=new EditorInfo(); InputConnection input=ime.create(info);
        assertEquals(android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD,info.inputType & android.text.InputType.TYPE_MASK_VARIATION);
        assertTrue(input.commitText("password",1));
        assertTrue(input.performContextMenuAction(android.R.id.selectAll));
        assertFalse(input.performContextMenuAction(android.R.id.copy));
        assertFalse(input.performContextMenuAction(android.R.id.cut));
        input.commitText("",1);
        GpuIme.Ticket before=ime.capture(); input.commitText("a",1);
        host.replies.get(host.replies.size()-1).accept(new JSONObject().put("token",7).put("state",state("a")));
        ime.presented(frame(7,""),before);
        assertEquals("a",input.getTextBeforeCursor(10,0));
        ime.presented(new JSONObject().put("nodes",new JSONArray()),ime.capture());
        assertFalse(ime.hasFocus()); assertFalse(input.commitText("late",1));
    }
}
