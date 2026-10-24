package com.lingxia.ui;

import android.graphics.Rect;
import android.view.*;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.widget.PopupMenu;
import org.json.*;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.annotation.Config;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest=Config.NONE,sdk={23,31})
public class GpuTextMenuTest {
    static class Surface extends View {
        Mode latest;
        Surface() { super(RuntimeEnvironment.getApplication()); }
        @Override public ActionMode startActionMode(ActionMode.Callback callback,int type) {
            assertEquals(ActionMode.TYPE_FLOATING,type);
            latest=new Mode(this,(ActionMode.Callback2)callback);
            return callback.onCreateActionMode(latest,latest.menu) ? latest : null;
        }
    }
    static class Mode extends ActionMode {
        final Menu menu; final Callback2 callback; final View view; boolean finished; int invalidations, rectInvalidations;
        Mode(View view,Callback2 callback) { this.view=view; this.callback=callback; menu=new PopupMenu(view.getContext(),view).getMenu(); }
        public void setTitle(CharSequence value) { }
        public void setTitle(int value) { }
        public void setSubtitle(CharSequence value) { }
        public void setSubtitle(int value) { }
        public void setCustomView(View value) { }
        public void invalidate() { invalidations++; callback.onPrepareActionMode(this,menu); }
        public void invalidateContentRect() { rectInvalidations++; }
        public void finish() { finished=true; callback.onDestroyActionMode(this); }
        public Menu getMenu() { return menu; }
        public CharSequence getTitle() { return ""; }
        public CharSequence getSubtitle() { return ""; }
        public View getCustomView() { return null; }
        public MenuInflater getMenuInflater() { return new MenuInflater(view.getContext()); }
        boolean click(int id) { return callback.onActionItemClicked(this,menu.findItem(id)); }
    }
    static class Client implements GpuIme.Host {
        final Surface view=new Surface(); final GpuIme ime=new GpuIme(view,this);
        final Rect bounds=new Rect(10,20,150,70);
        final GpuTextMenu menu=new GpuTextMenu(view,ime,() -> new Rect(bounds));
        int edits;
        public void request(JSONObject value,java.util.function.Consumer<JSONObject> reply) { if ("edit".equals(value.optString("op"))) edits++; }
        public void render(boolean rebuild) { }
        public void showTextMenu() { menu.show(); }
        public void hideTextMenu() { menu.dismiss(); }
        public void editingChanged() { menu.update(); }
        void present(boolean secure) throws Exception {
            present(secure,false,false);
        }
        void present(boolean secure,boolean undo,boolean redo) throws Exception {
            JSONObject state=new JSONObject().put("text","hello").put("selection",new JSONArray(new int[]{0,5}))
                .put("can_undo",undo).put("can_redo",redo);
            JSONObject input=new JSONObject().put("enabled",true).put("secure",secure).put("edit_token",7).put("edit",state);
            JSONObject node=new JSONObject().put("identity","editor").put("tag","input").put("input",input);
            ime.presented(new JSONObject().put("nodes",new JSONArray().put(node)),ime.capture());
        }
    }
    @Test public void menuUsesNativeActionsAndMovesWithInputBounds() throws Exception {
        Client c=new Client(); c.present(false); c.menu.show(); Mode mode=c.view.latest;
        assertTrue(mode.menu.findItem(android.R.id.copy).isVisible());
        assertTrue(mode.menu.findItem(android.R.id.cut).isVisible());
        for(int i=0;i<20;i++) c.menu.update(); assertEquals(0,mode.invalidations); assertEquals(0,mode.rectInvalidations);
        Rect rect=new Rect(); mode.callback.onGetContentRect(mode,c.view,rect); assertEquals(c.bounds,rect);
        c.bounds.offset(20,30); c.menu.update(); assertEquals(1,mode.rectInvalidations);
        mode.callback.onGetContentRect(mode,c.view,rect); assertEquals(c.bounds,rect);
        assertTrue(mode.click(android.R.id.selectAll)); assertFalse(mode.finished);
        assertTrue(mode.click(android.R.id.copy)); assertTrue(mode.finished);
        android.content.ClipboardManager clipboard=(android.content.ClipboardManager)c.view.getContext().getSystemService(android.content.Context.CLIPBOARD_SERVICE);
        assertEquals("hello",clipboard.getPrimaryClip().getItemAt(0).getText().toString());
        c.menu.show(); assertFalse(mode.click(android.R.id.cut)); assertEquals("hello",c.ime.menuText());
        assertTrue(c.view.latest.click(android.R.id.cut));
        assertEquals("",c.ime.menuText()); assertTrue(c.view.latest.finished);
        c.menu.show(); assertFalse(c.view.latest.menu.findItem(android.R.id.copy).isVisible());
        assertTrue(c.view.latest.click(android.R.id.paste)); assertEquals("hello",c.ime.menuText());
    }
    @Test public void passwordChangesAndLifecycleInvalidateMenuCallbacks() throws Exception {
        Client c=new Client(); c.present(false); c.menu.show(); Mode old=c.view.latest;
        c.present(true); assertFalse(old.menu.findItem(android.R.id.copy).isVisible());
        assertFalse(old.menu.findItem(android.R.id.cut).isVisible()); assertFalse(old.click(android.R.id.copy));
        c.ime.setCaretActive(false); assertTrue(old.finished); int edits=c.edits;
        assertFalse(old.click(android.R.id.paste)); assertEquals(edits,c.edits);
        c.ime.setCaretActive(true); c.menu.show(); old=c.view.latest;
        c.ime.clear(true); assertTrue(old.finished); assertFalse(old.click(android.R.id.selectAll));
    }
    @Test public void typingDismissesMenuWithoutDroppingEditor() throws Exception {
        Client c=new Client(); c.present(false); c.menu.show(); Mode mode=c.view.latest;
        InputConnection input=c.ime.create(new EditorInfo()); assertTrue(input.commitText("hello",1));
        assertTrue(mode.finished); assertTrue(c.ime.hasFocus()); assertEquals("hello",c.ime.menuText());
    }
    @Test public void historyMenuTracksAvailabilityAndCannotRepeatPendingCommands() throws Exception {
        Client c=new Client(); c.present(false,true,false); c.menu.show(); Mode mode=c.view.latest;
        assertTrue(mode.menu.findItem(android.R.id.undo).isVisible());
        assertFalse(mode.menu.findItem(android.R.id.redo).isVisible());
        for(int i=0;i<10;i++) c.menu.update(); assertEquals(0,mode.invalidations);
        assertTrue(mode.click(android.R.id.undo)); assertTrue(mode.finished); assertEquals(1,c.edits);
        c.menu.show(); assertFalse(c.view.latest.menu.findItem(android.R.id.undo).isVisible());
        assertFalse(c.view.latest.click(android.R.id.undo)); assertEquals(1,c.edits);
        c.ime.reset(); c.present(false,false,true); c.menu.show();
        assertTrue(c.view.latest.menu.findItem(android.R.id.redo).isVisible());
        assertFalse(mode.click(android.R.id.redo)); assertEquals(1,c.edits);
        assertTrue(c.view.latest.click(android.R.id.redo)); assertEquals(2,c.edits);
    }
}
