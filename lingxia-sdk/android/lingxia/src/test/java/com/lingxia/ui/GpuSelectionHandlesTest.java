package com.lingxia.ui;

import android.view.*;
import org.json.*;
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
public class GpuSelectionHandlesTest {
    static class Client implements GpuIme.Host,GpuSelectionHandles.Host {
        final ArrayList<JSONObject> requests=new ArrayList<>();
        final ArrayList<Consumer<JSONObject>> replies=new ArrayList<>();
        final GpuIme ime=new GpuIme(new View(RuntimeEnvironment.getApplication()),this);
        final GpuSelectionHandles handles=new GpuSelectionHandles(ime,this);
        JSONObject frame; boolean busy; int shown; float scale=1;
        Client() throws Exception { present(1,4,20,80); }
        public JSONObject frame() { return frame; }
        public float scale() { return scale; }
        public boolean busy() { return busy; }
        public void hideMenu() { }
        public void showMenu() { shown++; }
        public void request(JSONObject request,Consumer<JSONObject> reply) { requests.add(request); replies.add(reply); }
        public void render(boolean rebuild) { }
        public void editingChanged() { handles.update(); }
        public void hideTextMenu() { handles.update(); }
        JSONObject state(int anchor,int focus) throws Exception { return new JSONObject().put("text","abcdef").put("selection",new JSONArray(new int[]{anchor,focus})); }
        void present(int anchor,int focus,double ax,double fx) throws Exception {
            JSONObject geometry=new JSONObject().put("anchor",new JSONArray(new double[]{ax,15,1.5,20}))
                .put("caret",new JSONArray(new double[]{fx,15,1.5,20})).put("content_bounds",new JSONArray(new int[]{10,10,100,40}));
            JSONObject input=new JSONObject().put("edit_token",7).put("edit",state(anchor,focus)).put("geometry",geometry);
            JSONObject node=new JSONObject().put("identity","editor").put("tag","input").put("input",input)
                .put("bounds",new JSONArray(new int[]{0,0,120,70})).put("visible_bounds",new JSONArray(new int[]{0,0,120,70}));
            frame=new JSONObject().put("generation",frame==null ? 1 : frame.optLong("generation")+1).put("nodes",new JSONArray().put(node));
            ime.presented(frame,ime.capture()); handles.update();
        }
        boolean touch(int action,float x,float y) {
            MotionEvent event=MotionEvent.obtain(0,0,action,x,y,0);
            try { return handles.touch(event); } finally { event.recycle(); }
        }
        void hit(int request,int offset) throws Exception { replies.get(request).accept(new JSONObject().put("selection",new JSONArray(new int[]{offset,offset}))); }
        void ack(int request,int anchor,int focus) throws Exception { replies.get(request).accept(new JSONObject().put("token",7).put("state",state(anchor,focus))); }
    }
    @Test public void dragCoalescesMotionAndWaitsForPresentedSelectionBeforeFinishing() throws Exception {
        Client c=new Client(); assertTrue(c.touch(MotionEvent.ACTION_DOWN,80.75f,43));
        assertTrue(c.touch(MotionEvent.ACTION_MOVE,90,43)); assertEquals(1,c.requests.size());
        c.touch(MotionEvent.ACTION_MOVE,100,43); assertEquals(1,c.requests.size());
        c.hit(0,4); assertEquals(2,c.requests.size()); // Discard the first position and query the latest.
        assertEquals("drag_hit_test",c.requests.get(1).getString("op"));
        c.touch(MotionEvent.ACTION_UP,100,43); c.hit(1,5);
        assertEquals(3,c.requests.size());
        JSONObject edit=c.requests.get(2).getJSONArray("edits").getJSONObject(0);
        assertEquals(1,edit.getInt("anchor")); assertEquals(5,edit.getInt("focus"));
        assertTrue(c.handles.dragging()); assertEquals(0,c.shown);
        c.ack(2,1,5); assertTrue(c.handles.dragging()); // The old frame still owns the hit geometry.
        c.present(1,5,20,100); assertFalse(c.handles.dragging()); assertEquals(1,c.shown);
    }
    @Test public void edgeDragUsesContentViewportAndRepeatsOnlyAfterPresentation() throws Exception {
        Client c=new Client(); c.touch(MotionEvent.ACTION_DOWN,80.75f,43);
        c.touch(MotionEvent.ACTION_MOVE,-30,43);
        JSONObject query=c.requests.get(0);
        assertEquals("drag_hit_test",query.getString("op")); assertEquals(7,query.getLong("token"));
        assertEquals(-1,query.getJSONArray("edge").getInt(0));
        assertEquals(0,query.getJSONArray("edge").getInt(1));
        assertEquals(10.5,query.getJSONArray("point").getDouble(0),0.01);
        c.hit(0,2); c.ack(1,1,2); assertEquals(2,c.requests.size());
        c.present(1,2,20,30); assertEquals(3,c.requests.size());
        c.hit(2,2); c.handles.update(); assertEquals(3,c.requests.size());
        c.touch(MotionEvent.ACTION_UP,-30,43); assertFalse(c.handles.dragging());
    }
    @Test public void movingAnchorKeepsOtherEndpointFixedAndAllowsCrossing() throws Exception {
        Client c=new Client(); c.touch(MotionEvent.ACTION_DOWN,20.75f,43);
        c.touch(MotionEvent.ACTION_MOVE,100,43); c.hit(0,5);
        JSONObject edit=c.requests.get(1).getJSONArray("edits").getJSONObject(0);
        assertEquals(4,edit.getInt("anchor")); assertEquals(5,edit.getInt("focus"));
        c.ack(1,4,5); c.present(4,5,80,100);
        c.touch(MotionEvent.ACTION_MOVE,15,43); c.hit(2,0);
        edit=c.requests.get(3).getJSONArray("edits").getJSONObject(0);
        assertEquals(4,edit.getInt("anchor")); assertEquals(0,edit.getInt("focus"));
    }
    @Test public void staleFrameRetriesAfterPresentationButCancelledGesturesCannotEdit() throws Exception {
        Client c=new Client(); c.touch(MotionEvent.ACTION_DOWN,80.75f,43); c.touch(MotionEvent.ACTION_MOVE,90,43);
        c.replies.get(0).accept(null); c.handles.update(); assertEquals(1,c.requests.size());
        c.present(1,4,20,80); assertEquals(2,c.requests.size());
        c.touch(MotionEvent.ACTION_CANCEL,90,43); c.hit(1,5); assertEquals(2,c.requests.size());
        assertFalse(c.handles.dragging()); assertEquals(0,c.shown);
    }
    @Test public void offscreenEndpointsAndUnsettledMirrorsCannotStartDragging() throws Exception {
        Client c=new Client(); c.present(1,4,-30,80);
        assertFalse(c.touch(MotionEvent.ACTION_DOWN,10,43));
        c.busy=true; assertFalse(c.touch(MotionEvent.ACTION_DOWN,80.75f,43)); c.busy=false;
        c.ime.capture().connection.setSelection(2,4);
        assertFalse(c.touch(MotionEvent.ACTION_DOWN,80.75f,43));
        c.ack(0,2,4); assertFalse(c.touch(MotionEvent.ACTION_DOWN,80.75f,43));
        c.present(2,4,40,80); assertTrue(c.touch(MotionEvent.ACTION_DOWN,80.75f,43));
        c.scale=2; c.handles.update(); assertFalse(c.handles.dragging());
    }
    @Test public void collapsedHandleMovesCaretAndFocusLossInvalidatesItsQuery() throws Exception {
        Client c=new Client(); c.present(2,2,40,40);
        c.touch(MotionEvent.ACTION_DOWN,40.75f,43); c.touch(MotionEvent.ACTION_MOVE,90,43); c.hit(0,5);
        JSONObject edit=c.requests.get(1).getJSONArray("edits").getJSONObject(0);
        assertEquals(5,edit.getInt("anchor")); assertEquals(5,edit.getInt("focus"));
        c.ack(1,5,5); c.present(5,5,90,90);
        c.touch(MotionEvent.ACTION_MOVE,100,43); c.ime.clear(false); c.hit(2,6);
        assertEquals(3,c.requests.size()); assertFalse(c.handles.dragging());
    }
    @Test public void newerImeSelectionCancelsEvenWhenAnotherDragPointIsQueued() throws Exception {
        Client c=new Client(); c.touch(MotionEvent.ACTION_DOWN,80.75f,43); c.touch(MotionEvent.ACTION_MOVE,90,43);
        c.touch(MotionEvent.ACTION_MOVE,100,43); c.ime.capture().connection.setSelection(2,3);
        c.hit(0,5); assertFalse(c.handles.dragging()); assertEquals(2,c.requests.size());
        assertEquals(2,c.ime.capture().connection.anchor()); assertEquals(3,c.ime.capture().connection.focus());
    }
    @Test public void handleHitTargetsRespectAncestorClipsAndDoNotStealTextLineTaps() throws Exception {
        Client c=new Client();
        c.frame.getJSONArray("nodes").getJSONObject(0).put("clips",new JSONArray().put(new JSONObject()
            .put("bounds",new JSONArray(new int[]{0,0,120,40})).put("radius",0)));
        assertFalse(c.touch(MotionEvent.ACTION_DOWN,80.75f,25));
        assertFalse(c.touch(MotionEvent.ACTION_DOWN,80.75f,50));
        assertTrue(c.touch(MotionEvent.ACTION_DOWN,80.75f,38));
    }
    @Test @Config(sdk=31) @org.robolectric.annotation.GraphicsMode(org.robolectric.annotation.GraphicsMode.Mode.NATIVE)
    public void paintedHandlesRespectAncestorClips() throws Exception {
        Client c=new Client();
        c.frame.getJSONArray("nodes").getJSONObject(0).put("clips",new JSONArray().put(new JSONObject()
            .put("bounds",new JSONArray(new int[]{0,0,120,40})).put("radius",0)));
        android.graphics.Bitmap bitmap=android.graphics.Bitmap.createBitmap(120,90,android.graphics.Bitmap.Config.ARGB_8888);
        c.handles.draw(new android.graphics.Canvas(bitmap));
        assertEquals(0xff465cda,bitmap.getPixel(80,38)); assertEquals(0,bitmap.getPixel(80,43));
        bitmap.recycle();
    }
}
