package com.lingxia.ui;

import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONArray;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest=Config.NONE,sdk=28)
public class GpuRadioTest {
    private JSONArray nodes()throws Exception{return new JSONArray("[{tag:'radio',identity:'pace',accessibility_label:'Pace'},"
        +"{tag:'radio-option',identity:'daily',radio_group:'pace',checked:false,on_click:1},"
        +"{tag:'radio-option',identity:'weekly',radio_group:'pace',checked:true,on_click:2},"
        +"{tag:'radio-option',identity:'later',radio_group:'pace',checked:false},"
        +"{tag:'radio',identity:'other'},"
        +"{tag:'radio-option',radio_group:'other',checked:true,on_click:3}]");}
    @Test public void tabUsesSelectedEnabledOptionAndFallsBackToFirst()throws Exception{
        JSONArray nodes=nodes();assertFalse(GpuRadio.tabStop(nodes,1));assertTrue(GpuRadio.tabStop(nodes,2));assertFalse(GpuRadio.tabStop(nodes,3));assertTrue(GpuRadio.tabStop(nodes,5));
        nodes.getJSONObject(2).remove("on_click");assertTrue(GpuRadio.tabStop(nodes,1));assertFalse(GpuRadio.tabStop(nodes,2));
        nodes.getJSONObject(1).remove("on_click");assertFalse(GpuRadio.tabStop(nodes,1));
    }
    @Test public void arrowsWrapWithinGroupAndSkipDisabledOptions()throws Exception{
        JSONArray nodes=nodes();assertEquals(2,GpuRadio.adjacent(nodes,1,1));assertEquals(1,GpuRadio.adjacent(nodes,2,1));assertEquals(2,GpuRadio.adjacent(nodes,1,-1));assertEquals(5,GpuRadio.adjacent(nodes,5,1));assertEquals(-1,GpuRadio.adjacent(nodes,0,1));
    }
    @Test public void membershipUsesNativeIdentityAndCollectionReportsPosition()throws Exception{
        JSONArray nodes=nodes();assertEquals(0,GpuRadio.parent(nodes,nodes.getJSONObject(2)));assertEquals(4,GpuRadio.parent(nodes,nodes.getJSONObject(5)));
        AccessibilityNodeInfo group=AccessibilityNodeInfo.obtain(),option=AccessibilityNodeInfo.obtain();
        GpuRadio.populate(group,nodes,0);assertEquals("android.widget.RadioGroup",group.getClassName());assertEquals("Pace",group.getContentDescription());assertEquals(3,group.getCollectionInfo().getRowCount());assertEquals(AccessibilityNodeInfo.CollectionInfo.SELECTION_MODE_SINGLE,group.getCollectionInfo().getSelectionMode());
        GpuRadio.populate(option,nodes,2);assertEquals("android.widget.RadioButton",option.getClassName());assertEquals(1,option.getCollectionItemInfo().getRowIndex());assertTrue(option.getCollectionItemInfo().isSelected());group.recycle();option.recycle();
    }
}
