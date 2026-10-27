package com.lingxia.ui;

import android.graphics.Canvas;
import android.graphics.Path;
import android.graphics.RectF;
import android.graphics.drawable.GradientDrawable;
import com.lingxia.ui.generated.GpuFrame;
import org.json.JSONArray;
import org.json.JSONObject;

/** Pressed/focus feedback drawn from presented geometry; hit policy is in the shared core. */
final class GpuHighlight {
    /** Whether chrome anchored at a logical point would show through the node's clips. */
    static boolean contains(JSONObject node, double x, double y, boolean ownRadius) {
        return GpuView.nativeChromeContains(node.toString(), (float)x, (float)y, ownRadius);
    }
    private static double[] radii(float[][] values, double radius, double width, double height) {
        double[] result = new double[8];
        if (Double.isNaN(radius)) radius = 0;
        double r = Math.max(0, Math.min(radius, Math.min(width, height) / 2));
        if (values != null && values.length != 4) return null;
        for (int i = 0; i < 4; i++) {
            float[] pair = values == null ? null : values[i];
            if (values != null && (pair == null || pair.length != 2)) return null;
            double rx = pair == null ? r : pair[0], ry = pair == null ? r : pair[1];
            if (!Double.isFinite(rx) || !Double.isFinite(ry) || rx < 0 || ry < 0) return null;
            result[i*2] = rx == 0 || ry == 0 ? 0 : rx;
            result[i*2+1] = rx == 0 || ry == 0 ? 0 : ry;
        }
        double factor = 1;
        double[] sums = {result[0]+result[2],result[6]+result[4],result[1]+result[7],result[3]+result[5]};
        for (int i = 0; i < 4; i++) if (sums[i] > 0) factor = Math.min(factor,(i<2?width:height)/sums[i]);
        for (int i = 0; i < 8; i++) result[i] *= factor;
        return result;
    }
    /** Feedback obeys the same ancestor masks as GPU paint. */
    static final class Highlight extends GradientDrawable {
        private Path[] masks = new Path[0];
        void setNode(JSONObject node, float scale) {
            GpuFrame.Node frame = new GpuFrame.Node(node);
            GpuFrame.Clip[] clips = frame.clips();
            float[] original = frame.bounds(), visible = frame.visibleBounds();
            masks = new Path[1 + (clips == null ? 0 : clips.length)];
            for (int i = 0; i < masks.length; i++) {
                GpuFrame.Clip clip = i == 0 ? null : clips[i-1];
                float[] b = i == 0 ? (visible == null ? original : visible) : clip.bounds();
                float x = b[0] * scale, y = b[1] * scale;
                float w = b[2] * scale, h = b[3] * scale;
                double[] rs = i == 0 ? new double[8] : radii(clip.radii(),clip.radius(),w/scale,h/scale);
                masks[i] = new Path();
                if (rs != null) {
                    float[] corners = new float[8];
                    for (int j=0;j<8;j++) corners[j]=(float)rs[j]*scale;
                    masks[i].addRoundRect(new RectF(x,y,x+w,y+h),corners,Path.Direction.CW);
                }
            }
            double[] own = radii(frame.radii(),frame.radius(),original[2],original[3]);
            if (own == null) { setBounds(0,0,0,0); return; }
            float[] ownCorners = new float[8];
            for(int j=0;j<8;j++) ownCorners[j]=(float)own[j]*scale;
            setCornerRadii(ownCorners);
            setBounds((int)Math.floor(original[0]*scale),(int)Math.floor(original[1]*scale),
                (int)Math.ceil((original[0]+original[2])*scale),(int)Math.ceil((original[1]+original[3])*scale));
        }
        @Override public void draw(Canvas canvas) {
            int save = canvas.save();
            for (Path mask : masks) canvas.clipPath(mask);
            super.draw(canvas);
            canvas.restoreToCount(save);
        }
    }
}
