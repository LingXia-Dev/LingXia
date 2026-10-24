package com.lingxia.ui;

import android.view.View;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.annotation.Config;
import java.util.ArrayList;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = {23,31})
public class GpuInputConnectionTest {
    @Test public void undoWaitsForAuthoritativeStateAndRejectsEditsAgainstOldOffsets() throws Exception {
        Client c=new Client("hello",5,5);
        c.connection.acceptFrame(0,state("hello",5,5).put("can_undo",true));
        assertTrue(c.connection.canUndo());
        assertTrue(c.connection.performContextMenuAction(android.R.id.undo));
        assertEquals("undo",c.batches.get(0).getJSONObject(0).getString("op"));
        assertEquals("hello",c.text()); assertFalse(c.connection.canUndo());
        assertFalse(c.connection.commitText("stale",1)); assertFalse(c.connection.setSelection(0,0));
        assertFalse(c.connection.finishComposingText()); assertFalse(c.connection.beginBatchEdit());
        assertFalse(c.connection.performEditorAction(0)); assertFalse(c.connection.performContextMenuAction(android.R.id.copy));
        c.connection.acceptFrame(0,state("old frame",0,0)); assertEquals("hello",c.text());
        assertFalse(c.connection.acknowledge(0,state("old reply",0,0)));
        assertTrue(c.connection.acknowledge(1,state("",0,0).put("can_redo",true)));
        assertEquals("",c.text()); assertTrue(c.connection.canRedo());
        assertFalse(c.connection.canUndo()); assertTrue(c.connection.commitText("fresh",1));
        assertEquals("fresh",c.text()); assertFalse(c.connection.canRedo());
    }
    @Test public void historyShortcutsRespectCapabilitiesAndClosedConnections() throws Exception {
        Client c=new Client("hi",2,2);
        android.view.KeyEvent undo=new android.view.KeyEvent(0,0,0,android.view.KeyEvent.KEYCODE_Z,0,android.view.KeyEvent.META_CTRL_ON);
        assertFalse(c.connection.sendKeyEvent(undo));
        c.connection.acceptFrame(0,state("hi",2,2).put("can_undo",true));
        c.connection.beginBatchEdit(); assertFalse(c.connection.sendKeyEvent(undo)); c.connection.endBatchEdit();
        assertTrue(c.connection.sendKeyEvent(undo));
        c.connection.acknowledge(1,state("",0,0).put("can_redo",true));
        assertTrue(c.connection.sendKeyEvent(new android.view.KeyEvent(0,0,0,android.view.KeyEvent.KEYCODE_Z,0,
            android.view.KeyEvent.META_CTRL_ON|android.view.KeyEvent.META_SHIFT_ON)));
        assertEquals("redo",c.batches.get(1).getJSONObject(0).getString("op"));
        c.connection.closeConnection();
        assertFalse(c.connection.acknowledge(2,state("hi",2,2).put("can_undo",true)));
        assertFalse(c.connection.canUndo()); assertFalse(c.connection.canRedo());
    }
    private static android.view.KeyEvent deletion(int code) { return new android.view.KeyEvent(android.view.KeyEvent.ACTION_DOWN,code); }
    @Test @Config(sdk=31) public void hardwareBackspaceDeletesAndroidEmojiSequences() throws Exception {
        for(String emoji:new String[]{"👨‍👩‍👧‍👦","👍🏽","🇨🇳","1️⃣","😀"}) {
            String text="L"+emoji+"R"; Client c=new Client(text,text.length()-1,text.length()-1);
            assertTrue(c.connection.sendKeyEvent(deletion(android.view.KeyEvent.KEYCODE_DEL)));
            assertEquals(emoji,"LR",c.text()); assertEquals(1,c.connection.focus());
            assertEquals(2,c.batches.get(0).length());
        }
    }
    @Test public void deletionUsesSelectionInsteadOfCompositionAndReservesAtomically() throws Exception {
        Client c=new Client("compose keep",12,8); c.connection.setComposingRegion(0,7);
        assertTrue(c.connection.sendKeyEvent(deletion(android.view.KeyEvent.KEYCODE_DEL)));
        assertEquals("compose ",c.text()); assertEquals(-1,c.connection.composeStart());
        JSONArray batch=c.batches.get(c.batches.size()-1);
        assertEquals(3,batch.length()); assertEquals("finish_composition",batch.getJSONObject(0).getString("op"));
        c=new Client("keep",4,4); c.connection.beginBatchEdit();
        for(int i=0;i<63;i++) assertTrue(c.connection.setSelection(4,4));
        assertFalse(c.connection.sendKeyEvent(deletion(android.view.KeyEvent.KEYCODE_DEL)));
        assertEquals("keep",c.text()); assertEquals(4,c.connection.focus());
        c.connection.endBatchEdit(); assertEquals(63,c.batches.get(0).length());
    }
    @Test @Config(sdk=31) @org.robolectric.annotation.GraphicsMode(org.robolectric.annotation.GraphicsMode.Mode.NATIVE)
    public void forwardDeleteUsesPlatformGlyphBoundary() throws Exception {
        Client c=new Client("a😀b",1,1);
        assertTrue(c.connection.sendKeyEvent(deletion(android.view.KeyEvent.KEYCODE_FORWARD_DEL)));
        assertEquals("ab",c.text()); assertEquals(1,c.connection.focus());
    }
    @Test public void edgeNoopsPreserveImeDeletionUnits() throws Exception {
        Client c=new Client("a",0,0); assertTrue(c.connection.sendKeyEvent(deletion(android.view.KeyEvent.KEYCODE_DEL))); assertTrue(c.batches.isEmpty());
        c=new Client("a👍🏽",5,5); assertTrue(c.connection.deleteSurroundingTextInCodePoints(1,0)); assertEquals("a👍",c.text());
    }
    @Test public void touchSelectionReservesCompositionAndSelectionTogether() throws Exception {
        Client c=new Client("a\ud83d\ude00b",4,4); c.connection.beginBatchEdit();
        assertFalse(c.connection.settled());
        assertTrue(c.connection.setComposingRegion(0,4));
        assertFalse(c.connection.selectFromTouch(2,2));
        for(int i=0;i<62;i++) assertTrue(c.connection.setSelection(4,4));
        assertFalse(c.connection.selectFromTouch(0,3));
        assertEquals(4,c.connection.focus()); assertEquals(0,c.connection.composeStart());
        c.connection.endBatchEdit(); assertEquals(63,c.batches.get(0).length());
        assertFalse(c.connection.settled()); c.connection.acknowledge(1,state(c.text(),4,4));
        assertTrue(c.connection.settled()); assertTrue(c.connection.selectFromTouch(3,1));
        assertEquals("\ud83d\ude00",c.connection.getSelectedText(0));
    }
    @org.robolectric.annotation.Implements(android.content.ClipboardManager.class)
    public static class DeniedClipboard {
        @org.robolectric.annotation.Implementation protected void setPrimaryClip(android.content.ClipData data) {
            throw new SecurityException("Clipboard unavailable");
        }
    }
    @Test @Config(shadows=DeniedClipboard.class)
    public void deniedCutRollsBackReservationWithoutLosingEarlierBatchEdits() throws Exception {
        Client c=new Client("abc",0,3); c.connection.beginBatchEdit();
        assertTrue(c.connection.setComposingRegion(0,3));
        assertFalse(c.connection.performContextMenuAction(android.R.id.cut));
        assertEquals("abc",c.text()); assertEquals(0,c.connection.composeStart());
        assertEquals(0,c.connection.anchor()); assertEquals(3,c.connection.focus());
        for(int i=0;i<63;i++) assertTrue(c.connection.setSelection(0,3));
        c.connection.endBatchEdit(); assertEquals(64,c.batches.get(0).length());
        assertEquals("composition",c.batches.get(0).getJSONObject(0).getString("op"));
    }
    private static android.content.ClipboardManager clipboard() {
        return (android.content.ClipboardManager)RuntimeEnvironment.getApplication().getSystemService(android.content.Context.CLIPBOARD_SERVICE);
    }
    private static void clip(String text) { clipboard().setPrimaryClip(android.content.ClipData.newPlainText("",text)); }
    @Test public void clipboardUsesSelectionAndFinishesCompositionAtomically() throws Exception {
        Client c=new Client("a\ud83d\ude00bc",4,1);
        c.connection.setComposingRegion(0,1);
        assertTrue(c.connection.performContextMenuAction(android.R.id.copy));
        assertEquals("\ud83d\ude00b",clipboard().getPrimaryClip().getItemAt(0).getText().toString());
        assertEquals("a\ud83d\ude00bc",c.text()); assertEquals(0,c.connection.composeStart());
        assertTrue(c.connection.performContextMenuAction(android.R.id.cut));
        assertEquals("ac",c.text()); assertEquals(1,c.connection.focus());
        JSONArray batch=c.batches.get(c.batches.size()-1);
        assertEquals(2,batch.length()); assertEquals("finish_composition",batch.getJSONObject(0).getString("op"));
        assertTrue(c.connection.performContextMenuAction(android.R.id.paste));
        assertEquals("a\ud83d\ude00bc",c.text());
        c.connection.setComposingRegion(0,1);
        assertTrue(c.connection.performContextMenuAction(android.R.id.selectAll));
        assertEquals(-1,c.connection.composeStart()); assertEquals(0,c.connection.anchor()); assertEquals(5,c.connection.focus());
        clip("new"); assertTrue(c.connection.performContextMenuAction(android.R.id.pasteAsPlainText));
        assertEquals("new",c.text());
    }
    @Test public void secureAndClosedConnectionsDoNotExportText() throws Exception {
        Client c=new Client("secret",0,6); c.connection.secure=true; clip("safe");
        assertFalse(c.connection.performContextMenuAction(android.R.id.copy));
        assertFalse(c.connection.performContextMenuAction(android.R.id.cut));
        assertEquals("safe",clipboard().getPrimaryClip().getItemAt(0).getText().toString());
        assertEquals("secret",c.text());
        assertTrue(c.connection.performContextMenuAction(android.R.id.paste)); assertEquals("safe",c.text());
        c.connection.closeConnection();
        for(int action:new int[]{android.R.id.selectAll,android.R.id.copy,android.R.id.cut,android.R.id.paste})
            assertFalse(c.connection.performContextMenuAction(action));
    }
    @Test public void clipboardRejectsInvalidPayloadsAndPreservesQueuedEdits() throws Exception {
        Client c=new Client("abc",0,3);
        for(String value:new String[]{"\ud800",new String(new char[22000]).replace('\0','\u4f60')}) {
            clip(value); assertFalse(c.connection.performContextMenuAction(android.R.id.paste)); assertEquals("abc",c.text());
        }
        clipboard().setPrimaryClip(android.content.ClipData.newRawUri("",android.net.Uri.parse("content://unread/item")));
        assertFalse(c.connection.performContextMenuAction(android.R.id.paste));
        clip("unchanged"); c.connection.beginBatchEdit();
        c.connection.setComposingRegion(0,3);
        for(int i=0;i<62;i++) assertTrue(c.connection.setSelection(0,3));
        assertFalse(c.connection.performContextMenuAction(android.R.id.cut));
        assertFalse(c.connection.performContextMenuAction(android.R.id.selectAll));
        assertEquals("unchanged",clipboard().getPrimaryClip().getItemAt(0).getText().toString());
        assertEquals("abc",c.text()); assertEquals(0,c.connection.composeStart());
        c.connection.endBatchEdit(); assertEquals(63,c.batches.get(0).length());
    }
    @Test public void keyboardShortcutsAndMultiplePlainTextItems() throws Exception {
        Client c=new Client("old",3,3);
        assertTrue(c.connection.sendKeyEvent(new android.view.KeyEvent(0,0,0,android.view.KeyEvent.KEYCODE_A,0,android.view.KeyEvent.META_CTRL_ON)));
        android.content.ClipData data=android.content.ClipData.newPlainText("",new android.text.SpannableString("first"));
        data.addItem(new android.content.ClipData.Item("second")); clipboard().setPrimaryClip(data);
        assertTrue(c.connection.sendKeyEvent(new android.view.KeyEvent(0,0,0,android.view.KeyEvent.KEYCODE_V,0,android.view.KeyEvent.META_CTRL_ON)));
        assertEquals("first\nsecond",c.text());
        assertFalse(c.connection.sendKeyEvent(new android.view.KeyEvent(0,0,0,android.view.KeyEvent.KEYCODE_Z,0,android.view.KeyEvent.META_CTRL_ON)));
        assertEquals("first\nsecond",c.text());
    }
    private static JSONObject state(String text,int anchor,int focus) throws Exception {
        return new JSONObject().put("text",text).put("selection",new JSONArray(new int[]{anchor,focus}));
    }
    @Test public void submitOnEnterPreservesShiftNewlineAndPreeditConfirmation() throws Exception {
        Client c=new Client("hello",5,5); c.connection.multiline=true; c.connection.submitOnEnter=true;
        android.view.KeyEvent enter=new android.view.KeyEvent(android.view.KeyEvent.ACTION_DOWN,android.view.KeyEvent.KEYCODE_ENTER);
        assertTrue(c.connection.sendKeyEvent(new android.view.KeyEvent(0,0,0,android.view.KeyEvent.KEYCODE_ENTER,0,android.view.KeyEvent.META_SHIFT_ON)));
        assertEquals("hello\n",c.text()); assertFalse(c.submit);
        c.connection.setComposingRegion(0,5);
        assertTrue(c.connection.sendKeyEvent(enter)); assertFalse(c.submit);
        assertEquals("hello\n",c.text()); assertEquals(-1,c.connection.composeStart());
        assertTrue(c.connection.sendKeyEvent(enter)); assertTrue(c.submit);
        assertEquals("hello\n",c.text());
        c.connection.closeConnection(); assertFalse(c.connection.sendKeyEvent(enter));
    }
    private static class Client implements GpuInputConnection.Sink {
        final ArrayList<JSONArray> batches=new ArrayList<>();
        boolean submit;
        GpuInputConnection connection;
        Client(String text,int anchor,int focus) throws Exception {
            connection=new GpuInputConnection(new View(RuntimeEnvironment.getApplication()),"input",7,state(text,anchor,focus),this);
        }
        public void send(long revision,JSONArray edits,boolean submit) { batches.add(edits); this.submit=submit; }
        public void changed() { }
        String text() { return connection.getExtractedText(null,0).text.toString(); }
    }
    @Test public void physicalTextAndMultilineEnterFinishCompositionBeforeInsertion() throws Exception {
        Client c=new Client("line",4,4);
        c.connection.setComposingRegion(0,4);
        assertTrue(c.connection.sendKeyEvent(new android.view.KeyEvent(android.view.KeyEvent.ACTION_DOWN,android.view.KeyEvent.KEYCODE_A)));
        assertEquals("linea",c.text()); assertEquals(-1,c.connection.composeStart());
        JSONArray batch=c.batches.get(c.batches.size()-1);
        assertEquals(2,batch.length()); assertEquals("finish_composition",batch.getJSONObject(0).getString("op"));
        c.connection.setComposingRegion(0,5); c.connection.multiline=true;
        assertTrue(c.connection.sendKeyEvent(new android.view.KeyEvent(android.view.KeyEvent.ACTION_DOWN,android.view.KeyEvent.KEYCODE_ENTER)));
        assertEquals("linea\n",c.text()); assertFalse(c.submit);
        c.connection.closeConnection();
    }
    @Test public void physicalInsertionReservesBothEditsBeforeChangingComposition() throws Exception {
        Client c=new Client("",0,0); c.connection.beginBatchEdit();
        c.connection.setComposingText("line",1);
        for(int i=0;i<62;i++) assertTrue(c.connection.setSelection(4,4));
        assertFalse(c.connection.sendKeyEvent(new android.view.KeyEvent(android.view.KeyEvent.ACTION_DOWN,android.view.KeyEvent.KEYCODE_A)));
        assertEquals("line",c.text()); assertEquals(0,c.connection.composeStart()); assertEquals(4,c.connection.composeEnd());
        c.connection.endBatchEdit(); assertEquals(63,c.batches.get(0).length()); c.connection.closeConnection();
    }
    @Test public void compositionIsLocallyQueryableAndNestedBatchesSendOnce() throws Exception {
        Client c=new Client("",0,0);
        assertTrue(c.connection.beginBatchEdit());
        assertTrue(c.connection.beginBatchEdit());
        assertTrue(c.connection.setComposingText("ni",1));
        assertEquals("ni",c.connection.getTextBeforeCursor(20,0));
        assertEquals(0,c.connection.composeStart()); assertEquals(2,c.connection.composeEnd());
        assertTrue(c.connection.setComposingText("\u4f60\ud83d\ude00",1));
        assertEquals(3,c.connection.focus());
        assertTrue(c.connection.endBatchEdit()); assertEquals(0,c.batches.size());
        assertFalse(c.connection.endBatchEdit()); assertEquals(1,c.batches.size());
        assertEquals(2,c.batches.get(0).length());
        assertTrue(c.connection.performEditorAction(0)); assertTrue(c.submit);
        assertEquals(-1,c.connection.composeStart());
    }
    @Test public void oldAcknowledgementsAndFramesCannotOverwriteNewerTyping() throws Exception {
        Client c=new Client("",0,0);
        c.connection.commitText("a",1); c.connection.commitText("b",1);
        c.connection.acknowledge(1,state("a",1,1));
        assertEquals("ab",c.text());
        c.connection.acceptFrame(0,state("",0,0)); assertEquals("ab",c.text());
        c.connection.acknowledge(2,state("ab",2,2));
        c.connection.beginBatchEdit(); c.connection.commitText("c",1);
        c.connection.acceptFrame(2,state("old",3,3)); assertEquals("abc",c.text());
        c.connection.endBatchEdit();
        c.connection.acknowledge(3,state("ABC",3,3)); assertEquals("ABC",c.text());
        c.connection.acceptFrame(2,state("old",3,3)); assertEquals("ABC",c.text());
        c.connection.closeConnection();
        assertFalse(c.connection.commitText("late",1));
        assertFalse(c.connection.finishComposingText());
        assertNull(c.connection.getExtractedText(null,0));
    }
    @Test public void unicodeBoundariesAndDeletionMatchNativeEditor() throws Exception {
        Client c=new Client("a\ud83d\ude00bc",3,3);
        assertFalse(c.connection.setSelection(2,2));
        assertFalse(c.connection.deleteSurroundingText(1,0));
        assertEquals("a\ud83d\ude00bc",c.text());
        assertTrue(c.connection.deleteSurroundingTextInCodePoints(1,1));
        assertEquals("ac",c.text()); assertEquals(1,c.connection.focus());
        assertFalse(c.connection.commitText("\ud800",1));
        assertTrue(c.connection.setSelection(2,0));
        assertEquals("ac",c.connection.getSelectedText(0));
        assertTrue(c.connection.commitText("\u4f60",1)); assertEquals("\u4f60",c.text());
        if (android.os.Build.VERSION.SDK_INT >= 31) assertEquals(0,c.connection.getSurroundingText(20,20,0).getOffset());
    }
    @Test public void backpressureAndBatchLimitsRejectWithoutChangingMirror() throws Exception {
        Client c=new Client("",0,0);
        c.connection.beginBatchEdit();
        for(int i=0;i<64;i++) assertTrue(c.connection.commitText("x",1));
        assertFalse(c.connection.commitText("overflow",1)); assertEquals(64,c.text().length());
        assertFalse(c.connection.performEditorAction(0));
        c.connection.endBatchEdit();
        for(int i=1;i<64;i++) assertTrue(c.connection.commitText("x",1));
        assertFalse(c.connection.commitText("overflow",1));
        c.connection.acknowledge(64,state(c.text(),127,127));
        assertTrue(c.connection.commitText("ok",1));
        String large=new String(new char[65537]).replace('\0','x');
        assertFalse(c.connection.commitText(large,1));
    }
    @Test public void compositionDeletionAndRelativeCursorRemainExact() throws Exception {
        Client c=new Client("a\u4f60bc",2,2);
        assertTrue(c.connection.setComposingRegion(1,3));
        assertTrue(c.connection.setComposingText("\ud83d\ude00",0));
        assertEquals("a\ud83d\ude00c",c.text()); assertEquals(1,c.connection.focus());
        assertEquals(1,c.connection.composeStart()); assertEquals(3,c.connection.composeEnd());
        assertTrue(c.connection.setComposingText("",1));
        assertEquals("ac",c.text()); assertEquals(-1,c.connection.composeStart());
        assertTrue(c.connection.setSelection(2,0));
        assertTrue(c.connection.setComposingRegion(0,2));
        assertTrue(c.connection.deleteSurroundingText(Integer.MAX_VALUE,Integer.MAX_VALUE));
        assertEquals("ac",c.text()); assertEquals(2,c.connection.anchor());
        assertNull(c.connection.getEditable());
        String many=new String(new char[22000]).replace('\0','\u4f60');
        assertFalse(c.connection.commitText(many,1));
    }
}
