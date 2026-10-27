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
    /** Records what the menu asks of the core and the editor. */
    static class Client implements GpuTextMenu.Host {
        final Surface view=new Surface();
        final GpuTextMenu menu=new GpuTextMenu(view,this);
        final java.util.ArrayList<String> calls=new java.util.ArrayList<>();
        boolean coreRuns, coreCloses;
        public boolean coreAction(int code) {
            calls.add("core:"+GpuTextMenu.ACTIONS[code]);
            if (coreCloses) menu.show(null,1);
            return coreRuns;
        }
        public boolean editorAction(int itemId) { calls.add("editor:"+itemId); return true; }
        public void dismissed() { calls.add("dismissed"); }
        static JSONObject menu(String kind,double x,String... actions) throws Exception {
            return new JSONObject().put("target",new JSONObject().put("kind",kind).put("identity","n"))
                .put("anchor",new JSONArray(new double[]{x,20,50,10})).put("actions",new JSONArray(actions));
        }
    }
    @Test public void drawsTheCoreActionsAndFollowsTheAnchor() throws Exception {
        Client c=new Client();
        c.menu.show(Client.menu("text",10,"copy","select_all"),2); Mode mode=c.view.latest;
        assertNotNull(mode.menu.findItem(android.R.id.copy)); assertNotNull(mode.menu.findItem(android.R.id.selectAll));
        assertNull(mode.menu.findItem(android.R.id.cut));
        Rect rect=new Rect(); mode.callback.onGetContentRect(mode,c.view,rect); assertEquals(new Rect(20,40,120,60),rect);
        c.menu.show(Client.menu("text",10,"select_all"),2);
        assertSame("the same mode updates",mode,c.view.latest); assertEquals(0,mode.rectInvalidations);
        assertNull(mode.menu.findItem(android.R.id.copy));
        c.menu.show(Client.menu("text",30,"select_all"),2); assertEquals(1,mode.rectInvalidations);
        c.coreRuns=true; assertTrue(mode.click(android.R.id.selectAll));
        assertEquals(java.util.Arrays.asList("core:select_all"),c.calls);
        c.menu.show(null,2); assertTrue(mode.finished);
        assertEquals("closing for the core is not a user dismissal",java.util.Arrays.asList("core:select_all"),c.calls);
    }
    @Test public void editorActionRunsAfterTheCoreClosesTheMenu() throws Exception {
        Client c=new Client(); c.coreCloses=true;
        c.menu.show(Client.menu("editor",0,"cut","paste"),1); Mode mode=c.view.latest;
        assertTrue(mode.click(android.R.id.cut));
        assertEquals(java.util.Arrays.asList("core:cut","editor:"+android.R.id.cut),c.calls);
        assertTrue(mode.finished);
    }
    @Test public void editorActionsRunInTheInputConnectionAndUserDismissalIsReported() throws Exception {
        Client c=new Client();
        c.menu.show(Client.menu("editor",0,"cut","copy","paste","undo"),1); Mode mode=c.view.latest;
        assertTrue(mode.click(android.R.id.paste));
        assertEquals(java.util.Arrays.asList("core:paste","editor:"+android.R.id.paste),c.calls);
        mode.finish();
        assertEquals("dismissed",c.calls.get(c.calls.size()-1));
        c.menu.show(Client.menu("text",0,"copy"),1); Mode text=c.view.latest;
        assertFalse("a static action the core refuses never reaches an editor",text.click(android.R.id.copy));
    }
}
