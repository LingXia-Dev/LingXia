package com.lingxia.ui;

import java.util.HashMap;
import org.json.JSONArray;
import org.json.JSONObject;

/** Virtual IDs survive reordering but are never reused after removal or page reset. */
final class GpuAccessibilityIds {
    private HashMap<String,Integer> identities=new HashMap<>();
    private final HashMap<Integer,Integer> indices=new HashMap<>();
    private int[] ids=new int[0];
    private long next=1;
    void clear() { identities.clear(); indices.clear(); ids=new int[0]; }
    void update(JSONObject frame) {
        JSONArray nodes=frame.optJSONArray("nodes");
        int count=nodes==null ? 0 : nodes.length();
        ids=new int[count]; java.util.Arrays.fill(ids,-1); indices.clear();
        HashMap<String,Integer> retained=new HashMap<>();
        for(int i=0;i<count;i++) {
            JSONObject node=nodes.optJSONObject(i); String identity=node==null ? "" : node.optString("identity");
            if(identity.isEmpty()) continue;
            Integer id=identities.get(identity);
            if(id==null) { if(next>Integer.MAX_VALUE) continue; id=(int)next++; }
            retained.put(identity,id); indices.put(id,i); ids[i]=id;
        }
        identities=retained;
    }
    int id(int index) { return index<0 || index>=ids.length ? -1 : ids[index]; }
    int index(int id) { Integer index=indices.get(id); return index==null ? -1 : index; }
}
