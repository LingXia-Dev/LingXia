package com.lingxia.ui;

import android.graphics.Canvas;
import android.graphics.Path;
import android.graphics.RectF;
import android.graphics.drawable.GradientDrawable;
import org.json.JSONArray;
import org.json.JSONObject;

final class GpuClip {
    static boolean contains(JSONObject node, double x, double y, boolean ownRadius) {
        JSONArray visible = node.optJSONArray("visible_bounds");
        if (!inside(visible == null ? node.optJSONArray("bounds") : visible, 0, x, y)) return false;
        if (ownRadius && !inside(node.optJSONArray("bounds"), node.optDouble("radius"), node.optJSONArray("radii"), x, y)) return false;
        return containsAncestors(node,x,y);
    }
    static boolean containsAncestors(JSONObject node,double x,double y) {
        JSONArray clips = node.optJSONArray("clips");
        for (int i = 0; clips != null && i < clips.length(); i++) {
            JSONObject clip = clips.optJSONObject(i);
            if (clip == null || !inside(clip.optJSONArray("bounds"), clip.optDouble("radius"), clip.optJSONArray("radii"), x, y)) return false;
        }
        return true;
    }
    static boolean containsContent(JSONObject node, double x, double y) {
        JSONObject clip = node.optJSONObject("content_clip");
        return clip == null || inside(clip.optJSONArray("bounds"), clip.optDouble("radius"), clip.optJSONArray("radii"), x, y);
    }
    private static double[] radii(JSONArray values, double radius, double width, double height) {
        double[] result = new double[8];
        double r = Math.max(0, Math.min(radius, Math.min(width, height) / 2));
        if (values != null && values.length() != 4) return null;
        for (int i = 0; i < 4; i++) {
            JSONArray pair = values == null ? null : values.optJSONArray(i);
            if (values != null && (pair == null || pair.length() != 2)) return null;
            double rx = pair == null ? r : pair.optDouble(0), ry = pair == null ? r : pair.optDouble(1);
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
    private static boolean inside(JSONArray b, double radius, double x, double y) {
        return inside(b,radius,null,x,y);
    }
    private static boolean inside(JSONArray b, double radius, JSONArray corners, double x, double y) {
        if (b == null) return false;
        double left = b.optDouble(0), top = b.optDouble(1), width = b.optDouble(2), height = b.optDouble(3);
        if (!Double.isFinite(x) || !Double.isFinite(y) || !Double.isFinite(left) || !Double.isFinite(top)
            || !Double.isFinite(width) || !Double.isFinite(height) || width <= 0 || height <= 0
            || x < left || y < top || x >= left + width || y >= top + height) return false;
        double[] rs = radii(corners,radius,width,height);
        if (rs == null) return false;
        for (int i = 0; i < 4; i++) {
            double rx=rs[i*2],ry=rs[i*2+1];
            if (rx == 0 || ry == 0) continue;
            boolean right=i==1||i==2, bottom=i>=2;
            double cx=right?left+width-rx:left+rx, cy=bottom?top+height-ry:top+ry;
            if ((right?x>cx:x<cx) && (bottom?y>cy:y<cy)) {
                double dx=(x-cx)/rx,dy=(y-cy)/ry;
                return dx*dx+dy*dy<=1;
            }
        }
        return true;
    }

    /** Native pressed/focus feedback must obey the same ancestor masks as GPU paint. */
    static final class Highlight extends GradientDrawable {
        private Path[] masks = new Path[0];
        void setNode(JSONObject node, float scale) {
            JSONArray clips = node.optJSONArray("clips"), original = node.optJSONArray("bounds");
            JSONArray visible = node.optJSONArray("visible_bounds");
            masks = new Path[1 + (clips == null ? 0 : clips.length())];
            for (int i = 0; i < masks.length; i++) {
                JSONObject clip = i == 0 ? null : clips.optJSONObject(i-1);
                JSONArray b = i == 0 ? (visible == null ? original : visible) : clip.optJSONArray("bounds");
                float x = (float)b.optDouble(0) * scale, y = (float)b.optDouble(1) * scale;
                float w = (float)b.optDouble(2) * scale, h = (float)b.optDouble(3) * scale;
                double[] rs = i == 0 ? new double[8] : radii(clip.optJSONArray("radii"),clip.optDouble("radius"),w/scale,h/scale);
                masks[i] = new Path();
                if (rs != null) {
                    float[] corners = new float[8];
                    for (int j=0;j<8;j++) corners[j]=(float)rs[j]*scale;
                    masks[i].addRoundRect(new RectF(x,y,x+w,y+h),corners,Path.Direction.CW);
                }
            }
            double[] own = radii(node.optJSONArray("radii"),node.optDouble("radius",0),original.optDouble(2),original.optDouble(3));
            if (own == null) { setBounds(0,0,0,0); return; }
            float[] ownCorners = new float[8];
            for(int j=0;j<8;j++) ownCorners[j]=(float)own[j]*scale;
            setCornerRadii(ownCorners);
            setBounds((int)Math.floor(original.optDouble(0)*scale),(int)Math.floor(original.optDouble(1)*scale),
                (int)Math.ceil((original.optDouble(0)+original.optDouble(2))*scale),(int)Math.ceil((original.optDouble(1)+original.optDouble(3))*scale));
        }
        @Override public void draw(Canvas canvas) {
            int save = canvas.save();
            for (Path mask : masks) canvas.clipPath(mask);
            super.draw(canvas);
            canvas.restoreToCount(save);
        }
    }
}
