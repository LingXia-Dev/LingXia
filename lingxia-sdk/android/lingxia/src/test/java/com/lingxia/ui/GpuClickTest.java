package com.lingxia.ui;

import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest=Config.NONE,sdk=28)
public class GpuClickTest {
    private JSONObject button()throws Exception {
        return new JSONObject("{tag:'button',identity:'stop',text:'Stop',on_click:11,bounds:[0,0,80,40],clip:[0,0,100,100]}");
    }
    @Test public void unrelatedUpdatesAndCallbackRefreshRetainTheSameControl()throws Exception {
        JSONObject before=button(),after=button().put("on_click",29);
        JSONArray nodes=new JSONArray().put(new JSONObject("{tag:'text',text:'tick 5'}")).put(after);
        assertEquals(1,GpuClick.find(before,nodes));
        assertEquals(11,before.getLong("on_click"));
        assertEquals(29,after.getLong("on_click"));
    }
    @Test public void removalDisableMovementClippingAndSemanticChangesCancelThePress()throws Exception {
        JSONObject before=button();
        assertEquals(-1,GpuClick.find(before,new JSONArray()));
        for(String key:new String[]{"identity","text","tag","bounds","clip","button_content"}) {
            JSONObject changed=button().put(key,"changed");
            assertEquals(key,-1,GpuClick.find(before,new JSONArray().put(changed)));
        }
        JSONObject disabled=button();disabled.remove("on_click");
        assertEquals(-1,GpuClick.find(before,new JSONArray().put(disabled)));
        assertEquals(-1,GpuClick.find(null,new JSONArray().put(button())));
    }
}
