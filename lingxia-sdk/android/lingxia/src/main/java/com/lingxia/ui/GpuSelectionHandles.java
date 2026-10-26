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
        /** A handle was grabbed or let go; the core hides and restores the text menu. */
        void dragging(boolean active);
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

    GpuSelectionHandles(GpuIme ime,Host host) { this.ime=ime; this.host=host; }
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
    private boolean matches(JSONObject node) {
        JSONObject state=node.optJSONObject("input").optJSONObject("edit");
        JSONArray selection=state == null ? null : state.optJSONArray("selection");
        GpuInputConnection c=ime.capture().connection;
        return c != null && c.settled() && selection != null && state.optString("text").equals(ime.menuText())
            && selection.optInt(0,-1)==c.anchor() && selection.optInt(1,-1)==c.focus();
    }
    /** Center x, line bottom, line midpoint, or null when that end is offscreen or clipped. */
    private float[] endpoint(JSONObject node,String name) {
        return GpuView.nativeHandleEndpoint(node.toString(),name.equals("anchor") ? 0 : 1);
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
    void cancel() {
        boolean grabbed=owner != null;
        owner=null; pending=querying=released=false; gesture++; invalidateSelf();
        if (grabbed) host.dragging(false);
    }
    private void finish() { cancel(); }
    boolean touch(MotionEvent event) {
        int action=event.getActionMasked();
        if (action==MotionEvent.ACTION_DOWN) {
            cancel(); JSONObject node=node(); if (node==null || !matches(node) || host.busy()) return false;
            float factor=host.scale(), px=event.getX()/factor, py=event.getY()/factor;
            GpuInputConnection c=ime.capture().connection;
            float[] grabbed=GpuView.nativeHandleHit(node.toString(),px,py,c.anchor()==c.focus());
            if (grabbed==null) return false;
            int chosen=(int)grabbed[0]; float[] hit={grabbed[1],grabbed[2],grabbed[3]};
            owner=ime.capture(); text=ime.menuText(); scale=factor; pointer=event.getPointerId(0);
            collapsed=c.anchor()==c.focus(); fixed=chosen==0 ? c.focus() : c.anchor();
            dx=px-hit[0]; dy=py-hit[2]; x=hit[0]; y=hit[2]; movement=0; failedGeneration=-1;
            host.dragging(true); ime.focus(node,host.frame().optLong("generation")); return true;
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
        float[] drag=GpuView.nativeHandleDrag(node.toString(),x,y);
        if (drag==null) { cancel(); return; }
        float px=drag[0], py=drag[1]; int edgeX=(int)drag[2], edgeY=(int)drag[3];
        if (drag[4]==0) { pending=false; if (released) finish(); return; }
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
        paint.setColor(handleColor(host.frame()));
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
    /** The handle colour the core chose for the frame's theme. */
    static int handleColor(JSONObject frame) {
        JSONObject style=frame == null ? null : frame.optJSONObject("feedback");
        return style == null ? 0 : (int)style.optLong("selectionHandle");
    }
    @Override public int getOpacity() { return PixelFormat.TRANSLUCENT; }
}
