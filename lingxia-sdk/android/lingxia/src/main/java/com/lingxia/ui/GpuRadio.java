package com.lingxia.ui;

import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONArray;
import org.json.JSONObject;

/** Group membership comes from presented native identities, never labels or array positions. */
final class GpuRadio {
    static boolean group(JSONObject node) { return node!=null && "radio".equals(node.optString("tag")); }
    static boolean option(JSONObject node) { return node!=null && "radio-option".equals(node.optString("tag")); }
    static int parent(JSONArray nodes,JSONObject option) {
        String identity=option.optString("radio_group");
        if (identity.isEmpty()) return -1;
        for (int i=0;nodes!=null && i<nodes.length();i++) {
            JSONObject candidate=nodes.optJSONObject(i);
            if (group(candidate) && identity.equals(candidate.optString("identity"))) return i;
        }
        return -1;
    }
    static boolean member(JSONObject node,String identity) { return option(node) && identity.equals(node.optString("radio_group")); }
    static boolean tabStop(JSONArray nodes,int index) {
        JSONObject node=nodes.optJSONObject(index);
        if (!option(node)) return true;
        String identity=node.optString("radio_group"); int first=-1;
        for (int i=0;i<nodes.length();i++) {
            JSONObject candidate=nodes.optJSONObject(i);
            if (!member(candidate,identity) || candidate.optLong("on_click",0)==0) continue;
            if (first<0) first=i;
            if (candidate.optBoolean("checked")) return i==index;
        }
        return first==index;
    }
    static int adjacent(JSONArray nodes,int index,int direction) {
        JSONObject node=nodes==null?null:nodes.optJSONObject(index);
        if (!option(node)) return -1;
        String identity=node.optString("radio_group");
        for (int step=1;step<=nodes.length();step++) {
            int next=Math.floorMod(index+direction*step,nodes.length());JSONObject candidate=nodes.optJSONObject(next);
            if (member(candidate,identity) && candidate.optLong("on_click",0)!=0) return next;
        }
        return -1;
    }
    static void populate(AccessibilityNodeInfo info,JSONArray nodes,int index) {
        JSONObject node=nodes.optJSONObject(index);
        if (group(node)) {
            info.setClassName("android.widget.RadioGroup");
            info.setContentDescription(node.optString("accessibility_label"));
            int count=0;boolean enabled=false;
            for (int i=0;i<nodes.length();i++) if (member(nodes.optJSONObject(i),node.optString("identity"))) {
                count++;enabled|=nodes.optJSONObject(i).optLong("on_click",0)!=0;
            }
            info.setEnabled(enabled);
            info.setCollectionInfo(AccessibilityNodeInfo.CollectionInfo.obtain(count,1,false,AccessibilityNodeInfo.CollectionInfo.SELECTION_MODE_SINGLE));
        } else if (option(node)) {
            info.setClassName("android.widget.RadioButton");
            int row=0;for(int i=0;i<index;i++) if(member(nodes.optJSONObject(i),node.optString("radio_group")))row++;
            info.setCollectionItemInfo(AccessibilityNodeInfo.CollectionItemInfo.obtain(row,1,0,1,false,node.optBoolean("checked")));
        }
    }
}
