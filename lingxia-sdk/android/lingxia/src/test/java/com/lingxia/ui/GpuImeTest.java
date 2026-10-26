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
        final ArrayList<String> carets=new ArrayList<>();
        public void caret(boolean eligible,String text,int anchor,int focus,int start,int end) {
            carets.add(eligible+":"+text+":"+anchor+":"+focus+":"+start+":"+end);
        }
    }
    private static JSONObject state(String text) throws Exception {
        return new JSONObject().put("text",text).put("selection",new JSONArray(new int[]{text.length(),text.length()}));
    }
    /** The core's keyboard configuration for a plain single-line editor. */
    private static JSONObject ime() throws Exception {
        return new JSONObject().put("keyboard","text").put("secure",false).put("multiline",false).put("capitalize","none")
            .put("suggestions",true).put("return_key","done").put("return_inserts_newline",false).put("hide_on_submit",true)
            .put("picker",JSONObject.NULL).put("auto_complete",JSONObject.NULL).put("max_length",JSONObject.NULL);
    }
    private static JSONObject node(long token,String text) throws Exception {
        return new JSONObject().put("identity","editor").put("tag","input").put("input",new JSONObject()
            .put("enabled",true).put("keyboard","text").put("edit_token",token).put("edit",state(text)).put("ime",ime()));
    }
    private static JSONObject ime(JSONObject node) { return node.optJSONObject("input").optJSONObject("ime"); }
    private static JSONObject frame(long token,String text) throws Exception { return new JSONObject().put("nodes",new JSONArray().put(node(token,text))); }
    private GpuIme ime(Host host) { return new GpuIme(new View(RuntimeEnvironment.getApplication()),host); }

    @Test public void multilineSubmitOptionUpdatesKeyboardActionAndPreservesMultiline() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host);
        JSONObject node=node(7,"hello").put("tag","textarea");
        ime(node).put("multiline",true).put("return_key","newline").put("return_inserts_newline",true).put("hide_on_submit",false);
        JSONObject frame=new JSONObject().put("nodes",new JSONArray().put(node));
        ime.presented(frame,ime.capture()); EditorInfo info=new EditorInfo(); ime.create(info);
        assertEquals(EditorInfo.IME_ACTION_NONE,info.imeOptions & EditorInfo.IME_MASK_ACTION);
        ime(node).put("return_key","send").put("return_inserts_newline",false);
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
        JSONObject node=node(7,"coffee").put("tag","search");ime(node).put("keyboard","search").put("return_key","search").put("hide_on_submit",false);
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

    @Test public void editorStateReachesTheHostCaretAndTeardownClearsIt() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host);
        ime.presented(frame(7,"hello"),ime.capture());
        assertEquals("true:hello:5:5:-1:-1",host.carets.get(host.carets.size()-1));
        ime.setCaretActive(false);
        assertEquals("false:hello:5:5:-1:-1",host.carets.get(host.carets.size()-1));
        ime.setCaretActive(true); ime.clear(true);
        assertEquals("true:null:0:0:-1:-1",host.carets.get(host.carets.size()-1));
        assertEquals("blinking is host-timed; the input method never renders for it",0,host.frames);
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
        JSONObject node=node(7,""); ime(node).put("secure",true);
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

    @Test public void editorAttributesShapeTheKeyboardAndAutofillHint() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host);
        JSONObject node=node(7,""); ime(node).put("return_key","next").put("capitalize","words")
            .put("suggestions",false).put("auto_complete","one-time-code").put("max_length",3);
        ime.presented(new JSONObject().put("nodes",new JSONArray().put(node)),ime.capture());
        EditorInfo info=new EditorInfo(); InputConnection input=ime.create(info);
        assertEquals(EditorInfo.IME_ACTION_NEXT,info.imeOptions & EditorInfo.IME_MASK_ACTION);
        assertNotEquals(0,info.inputType & android.text.InputType.TYPE_TEXT_FLAG_CAP_WORDS);
        assertNotEquals(0,info.inputType & android.text.InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS);
        assertEquals("smsOTPCode",com.lingxia.ui.generated.GpuInput.autofillHint("one-time-code"));
        assertTrue(input.commitText("abcd",1));
        assertEquals("abc",input.getTextBeforeCursor(10,0));
        assertTrue(input.commitText("e",1));
        assertEquals("abc",input.getTextBeforeCursor(10,0));
        assertEquals(android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD,
            GpuIme.inputType(ime().put("secure",true)) & android.text.InputType.TYPE_MASK_VARIATION);
        for (String key : new String[]{"done","go","previous","search","send"}) assertTrue(key,GpuIme.action(key) != EditorInfo.IME_ACTION_NONE);
        assertEquals(EditorInfo.IME_ACTION_NONE,GpuIme.action("newline"));
    }

    @Test public void nextActionRetargetsTheKeyboardToTheEditorFocusMovedTo() throws Exception {
        Host host=new Host(); GpuIme ime=ime(host);
        JSONObject first=node(7,"a"), second=node(0,"").put("identity","second");
        second.getJSONObject("input").remove("edit_token"); second.getJSONObject("input").remove("edit");
        ime(first).put("return_key","next");
        ime.presented(new JSONObject().put("nodes",new JSONArray().put(first).put(second)),ime.capture());
        InputConnection input=ime.create(new EditorInfo());
        assertTrue(input.performEditorAction(EditorInfo.IME_ACTION_NEXT));
        assertTrue(host.requests.get(0).getBoolean("submit"));
        host.replies.get(0).accept(new JSONObject().put("moved_to","second").put("token",9).put("state",state("")));
        assertTrue(ime.hasFocus());
        InputConnection moved=ime.create(new EditorInfo());
        assertTrue(moved.commitText("b",1));
        JSONObject edit=host.requests.get(host.requests.size()-1);
        assertEquals("second",edit.getString("identity"));
        assertEquals(9,edit.getLong("token"));
    }

    @Test public void datePickerConfirmsAnIsoValueThenBlurs() throws Exception {
        Host host=new Host();
        android.app.Activity activity=org.robolectric.Robolectric.buildActivity(android.app.Activity.class).setup().get();
        GpuIme ime=new GpuIme(new View(activity),host);
        JSONObject node=node(7,"2024-05-06"); ime(node).put("picker",new JSONObject().put("kind","date").put("min","2024-01-01").put("max",JSONObject.NULL));
        ime.presented(new JSONObject().put("nodes",new JSONArray().put(node)),ime.capture());
        assertNull("no keyboard connection",ime.create(new EditorInfo()));
        android.app.DatePickerDialog dialog=(android.app.DatePickerDialog)org.robolectric.shadows.ShadowDialog.getLatestDialog();
        assertNotNull(dialog);
        assertEquals(2024,dialog.getDatePicker().getYear());
        assertEquals(4,dialog.getDatePicker().getMonth());
        dialog.updateDate(2024,6,9);
        dialog.getButton(android.content.DialogInterface.BUTTON_POSITIVE).performClick();
        org.robolectric.shadows.ShadowLooper.idleMainLooper();
        JSONObject submit=host.requests.get(0);
        assertTrue(submit.getBoolean("submit"));
        assertTrue(submit.getJSONArray("edits").toString(),submit.getJSONArray("edits").toString().contains("2024-07-09"));
        assertEquals("blur",host.requests.get(host.requests.size()-1).getString("op"));
        assertFalse(ime.hasFocus());
    }
}
