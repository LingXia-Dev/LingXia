package com.lingxia.ui;

import android.graphics.*;
import android.graphics.drawable.Drawable;
import android.view.MotionEvent;
import org.json.*;

/** Selection chrome above the SurfaceView; positions always come from a presented frame. */
final class GpuSelectionHandles extends Drawable {
    interface Host {
        JSONObject frame();
        float scale();
        boolean busy();
        void hideMenu();
        void showMenu();
    }
    private final GpuIme ime;
    private final Host host;
    private final Paint paint=new Paint(Paint.ANTI_ALIAS_FLAG);
    private GpuIme.Ticket owner;
    private String text;
    private float scale, x, y, dx, dy;
    private int fixed, pointer;
    private boolean collapsed, released, pending, querying;
    private long movement, failedGeneration=-1, gesture;

    GpuSelectionHandles(GpuIme ime,Host host) { this.ime=ime; this.host=host; paint.setColor(0xff465cda); }
    boolean dragging() { return owner != null; }
    private JSONObject node() {
        if (!ime.menuActive()) return null;
        GpuInputConnection connection=ime.capture().connection;
        JSONObject frame=host.frame(); JSONArray nodes=frame == null ? null : frame.optJSONArray("nodes");
        for(int i=0;nodes != null && i<nodes.length();i++) {
            JSONObject node=nodes.optJSONObject(i), input=node == null ? null : node.optJSONObject("input");
            if (input != null && GpuIme.editable(node) && input.optJSONObject("geometry")!=null && input.optJSONObject("edit")!=null
                && connection.identity.equals(node.optString("identity")) && input.optLong("edit_token") == connection.token) return node;
        }
        return null;
    }
    private static JSONObject geometry(JSONObject node) { return node.optJSONObject("input").optJSONObject("geometry"); }
    private boolean matches(JSONObject node) {
        JSONObject state=node.optJSONObject("input").optJSONObject("edit");
        JSONArray selection=state == null ? null : state.optJSONArray("selection");
        GpuInputConnection c=ime.capture().connection;
        return c != null && c.settled() && selection != null && state.optString("text").equals(ime.menuText())
            && selection.optInt(0,-1)==c.anchor() && selection.optInt(1,-1)==c.focus();
    }
    // Center x, line bottom, line midpoint. Offscreen endpoints must not become edge handles.
    private float[] endpoint(JSONObject node,String name) {
        JSONObject geometry=geometry(node); if (geometry == null) return null;
        JSONArray r=geometry.optJSONArray(name), clip=geometry.optJSONArray("content_bounds");
        if (r == null || clip == null) return null;
        float cx=(float)(r.optDouble(0)+r.optDouble(2)/2), bottom=(float)(r.optDouble(1)+r.optDouble(3));
        float mid=(float)(r.optDouble(1)+r.optDouble(3)/2);
        if (Float.isNaN(cx) || Float.isInfinite(cx) || Float.isNaN(bottom) || Float.isInfinite(bottom) || Float.isNaN(mid) || Float.isInfinite(mid)
            || cx < clip.optDouble(0) || cx > clip.optDouble(0)+clip.optDouble(2)
            || bottom <= clip.optDouble(1) || bottom > clip.optDouble(1)+clip.optDouble(3)+0.5
            || !GpuClip.contains(node,cx,bottom-0.5,false)) return null;
        return new float[]{cx,bottom,mid};
    }
    void update() {
        if (owner != null) {
            GpuIme.Ticket now=ime.capture();
            if (!ime.menuActive() || now.epoch != owner.epoch || now.connection != owner.connection
                || host.scale()!=scale || !ime.menuText().equals(text) || node()==null) cancel();
            else pump();
        }
        invalidateSelf();
    }
    void cancel() { owner=null; pending=querying=released=false; gesture++; invalidateSelf(); }
    private void finish() { cancel(); if (ime.menuActive()) host.showMenu(); }
    boolean touch(MotionEvent event) {
        int action=event.getActionMasked();
        if (action==MotionEvent.ACTION_DOWN) {
            cancel(); JSONObject node=node(); if (node==null || !matches(node) || host.busy()) return false;
            float factor=host.scale(), px=event.getX()/factor, py=event.getY()/factor;
            if (!GpuClip.containsAncestors(node,px,py)) return false;
            GpuInputConnection c=ime.capture().connection;
            float[] hit=null; int chosen=-1; float best=22*22;
            for(int i=0;i<2;i++) {
                if (i==0 && c.anchor()==c.focus()) continue;
                float[] endpoint=endpoint(node,i==0 ? "anchor" : "caret"); if (endpoint==null) continue;
                if (py<endpoint[1]-4) continue;
                float distance=(px-endpoint[0])*(px-endpoint[0])+(py-endpoint[1]-8)*(py-endpoint[1]-8);
                if (distance<best) { best=distance; hit=endpoint; chosen=i; }
            }
            if (hit==null) return false;
            owner=ime.capture(); text=ime.menuText(); scale=factor; pointer=event.getPointerId(0);
            collapsed=c.anchor()==c.focus(); fixed=chosen==0 ? c.focus() : c.anchor();
            dx=px-hit[0]; dy=py-hit[2]; x=hit[0]; y=hit[2]; movement=0; failedGeneration=-1;
            host.hideMenu(); ime.focus(node,host.frame().optLong("generation")); return true;
        }
        if (owner==null) return false;
        if (action==MotionEvent.ACTION_CANCEL || action==MotionEvent.ACTION_POINTER_DOWN) { cancel(); return true; }
        int index=event.findPointerIndex(pointer); if (index<0) { cancel(); return true; }
        if (action==MotionEvent.ACTION_MOVE || action==MotionEvent.ACTION_UP) {
            float nextX=event.getX(index)/scale-dx, nextY=event.getY(index)/scale-dy;
            if (nextX!=x || nextY!=y) { x=nextX; y=nextY; pending=true; movement++; failedGeneration=-1; }
            if (action==MotionEvent.ACTION_UP) released=true;
            update();
        }
        return true;
    }
    private void pump() {
        if (querying || host.busy()) return;
        JSONObject node=node(); if (node==null || !matches(node)) return;
        if (!pending) { if (released) finish(); return; }
        long generation=host.frame().optLong("generation"); if (generation==failedGeneration) return;
        JSONArray b=node.optJSONArray("visible_bounds"), content=geometry(node).optJSONArray("content_bounds");
        if (b==null || content==null) return;
        float left=(float)Math.max(b.optDouble(0),content.optDouble(0));
        float top=(float)Math.max(b.optDouble(1),content.optDouble(1));
        float right=(float)Math.min(b.optDouble(0)+b.optDouble(2),content.optDouble(0)+content.optDouble(2));
        float bottom=(float)Math.min(b.optDouble(1)+b.optDouble(3),content.optDouble(1)+content.optDouble(3));
        if (right-left<1 || bottom-top<1) { cancel(); return; }
        float px=Math.max(left+0.5f,Math.min(x,right-0.5f));
        float py=Math.max(top+0.5f,Math.min(y,bottom-0.5f));
        int edgeX=x<content.optDouble(0) ? -1 : x>content.optDouble(0)+content.optDouble(2) ? 1 : 0;
        int edgeY=y<content.optDouble(1) ? -1 : y>content.optDouble(1)+content.optDouble(3) ? 1 : 0;
        if (!GpuClip.contains(node,px,py,false)) { pending=false; if (released) finish(); return; }
        long serial=gesture, point=movement; querying=true;
        boolean sent=ime.dragSelection(node,generation,px,py,edgeX,edgeY,fixed,collapsed,() -> owner!=null && gesture==serial && movement==point,result -> {
            if (gesture!=serial || owner==null) return;
            querying=false;
            if (result==GpuIme.DragResult.CANCELLED) { cancel(); return; }
            if (point!=movement) { pump(); return; }
            if (result==GpuIme.DragResult.STALE_FRAME) { failedGeneration=generation; return; }
            pending=false;
            // Re-query after a scrolling frame while the finger remains beyond the content edge.
            if (result==GpuIme.DragResult.EDITED && !released) pending=edgeX!=0 || edgeY!=0;
            update();
        });
        if (!sent) querying=false;
    }
    @Override public void draw(Canvas canvas) {
        JSONObject node=node(); if (node==null) return;
        float factor=host.scale(); int saved=canvas.save(); canvas.scale(factor,factor);
        JSONArray clips=node.optJSONArray("clips");
        for(int i=0;clips!=null && i<clips.length();i++) {
            JSONObject clip=clips.optJSONObject(i); JSONArray b=clip.optJSONArray("bounds");
            float left=(float)b.optDouble(0),top=(float)b.optDouble(1),right=left+(float)b.optDouble(2),bottom=top+(float)b.optDouble(3);
            float radius=Math.max(0,Math.min((float)clip.optDouble("radius"),Math.min(right-left,bottom-top)/2));
            Path path=new Path(); path.addRoundRect(new RectF(left,top,right,bottom),radius,radius,Path.Direction.CW); canvas.clipPath(path);
        }
        JSONObject edit=node.optJSONObject("input").optJSONObject("edit");
        JSONArray selected=edit==null ? null : edit.optJSONArray("selection");
        if (selected==null) { canvas.restoreToCount(saved); return; }
        for(int i=0;i<2;i++) {
            if (i==0 && selected.optInt(0)==selected.optInt(1)) continue;
            float[] endpoint=endpoint(node,i==0 ? "anchor" : "caret"); if (endpoint==null) continue;
            canvas.drawRect(endpoint[0]-1,endpoint[1]-1,endpoint[0]+1,endpoint[1]+7,paint);
            canvas.drawCircle(endpoint[0],endpoint[1]+8,7,paint);
        }
        canvas.restoreToCount(saved);
    }
    @Override public void setAlpha(int alpha) { paint.setAlpha(alpha); invalidateSelf(); }
    @Override public void setColorFilter(ColorFilter filter) { paint.setColorFilter(filter); invalidateSelf(); }
    @Override public int getOpacity() { return PixelFormat.TRANSLUCENT; }
}
