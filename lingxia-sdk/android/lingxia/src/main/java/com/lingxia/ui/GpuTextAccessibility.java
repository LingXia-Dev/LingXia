package com.lingxia.ui;

import android.os.Build;
import android.view.View;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONArray;
import org.json.JSONObject;

/** Text metadata and events use logical UTF-16 offsets, independent of painted glyphs. */
final class GpuTextAccessibility {
    static String text(JSONObject input) {
        JSONObject edit=input.optJSONObject("edit");
        String value=edit == null ? input.optString("value") : edit.optString("text");
        if (!input.optBoolean("secure")) return value;
        return mask(value);
    }
    private static String mask(String value) {
        char[] masked=new char[value.length()]; java.util.Arrays.fill(masked,'\u2022');
        return new String(masked);
    }
    static int selection(JSONObject input,int index) {
        JSONObject edit=input.optJSONObject("edit");
        JSONArray range=edit == null ? null : edit.optJSONArray("selection");
        return input.optLong("edit_token") > 0 && range != null ? range.optInt(index,-1) : -1;
    }
    static void populate(AccessibilityNodeInfo info,JSONObject node) {
        JSONObject input=node.optJSONObject("input");
        if (input == null) return;
        info.setText(text(input)); info.setEditable(true); info.setPassword(input.optBoolean("secure"));
        String label = node.optString("accessibility_label");
        if (!label.isEmpty()) info.setContentDescription(label);
        info.setFocused(input.optLong("edit_token") > 0);
        info.setTextSelection(selection(input,0),selection(input,1));
        info.setMultiLine("textarea".equals(node.optString("tag")));
        if (Build.VERSION.SDK_INT >= 26) {
            info.setHintText(input.optString("placeholder"));
            info.setShowingHintText(false);
        }
        if (input.optBoolean("enabled",true)) {
            info.addAction(AccessibilityNodeInfo.ACTION_SET_TEXT);
            info.addAction(AccessibilityNodeInfo.ACTION_SET_SELECTION);
        }
    }
    static void events(View view,JSONObject previous,JSONObject next,java.util.function.IntUnaryOperator virtualId) {
        android.view.accessibility.AccessibilityManager manager=(android.view.accessibility.AccessibilityManager)view.getContext().getSystemService(android.content.Context.ACCESSIBILITY_SERVICE);
        if (view.getParent() == null || manager == null || !manager.isEnabled()) return;
        JSONArray before=previous == null ? null : previous.optJSONArray("nodes"), after=next.optJSONArray("nodes");
        java.util.HashMap<String,JSONObject> old=new java.util.HashMap<>();
        for (int i=0;before != null && i<before.length();i++) {
            JSONObject node=before.optJSONObject(i);
            if (node != null && node.has("input")) old.put(node.optString("identity"),node.optJSONObject("input"));
        }
        for (int i=0;after != null && i<after.length();i++) {
            JSONObject node=after.optJSONObject(i), input=node == null ? null : node.optJSONObject("input");
            if (input == null || virtualId.applyAsInt(i)<0) continue;
            JSONObject prior=old.get(node.optString("identity"));
            if (prior == null) continue;
            String value=text(input), was=text(prior);
            if (input.optBoolean("secure")) was=mask(was);
            if (!value.equals(was)) {
                AccessibilityEvent event=event(view,virtualId.applyAsInt(i),input,AccessibilityEvent.TYPE_VIEW_TEXT_CHANGED);
                event.setBeforeText(was); event.getText().add(value); event.setFromIndex(0);
                event.setRemovedCount(was.length()); event.setAddedCount(value.length());
                view.getParent().requestSendAccessibilityEvent(view,event);
            }
            int start=selection(input,0), end=selection(input,1);
            if (start >= 0 && (start != selection(prior,0) || end != selection(prior,1) || !value.equals(was))) {
                AccessibilityEvent event=event(view,virtualId.applyAsInt(i),input,AccessibilityEvent.TYPE_VIEW_TEXT_SELECTION_CHANGED);
                event.getText().add(value); event.setFromIndex(start); event.setToIndex(end); event.setItemCount(value.length());
                view.getParent().requestSendAccessibilityEvent(view,event);
            }
        }
    }
    private static AccessibilityEvent event(View view,int id,JSONObject input,int type) {
        AccessibilityEvent event=AccessibilityEvent.obtain(type);
        event.setSource(view,id); event.setPackageName(view.getContext().getPackageName());
        event.setClassName("android.widget.EditText"); event.setPassword(input.optBoolean("secure"));
        return event;
    }
}
