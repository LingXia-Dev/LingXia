package com.lingxia.ui.tests;

import android.view.View;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import org.junit.Test;
import org.junit.runner.RunWith;
import static org.junit.Assert.*;

/**
 * Reads view-lab's Accessibility section as an accessibility service does and walks TalkBack
 * focus over it. Needs a host whose tab bar opens view-lab as "Lab"; run with TalkBack on to get
 * its speech in the host's external `gpu-a11y-*.png` screenshots (Developer settings, Display speech output).
 */
@RunWith(AndroidJUnit4.class)
public class GpuAccessibilityProbe {
    private final A11yDriver driver = new A11yDriver(InstrumentationRegistry.getInstrumentation());

    private static void collectionItems(AccessibilityNodeInfo node, List<AccessibilityNodeInfo> out) {
        if (node == null) return;
        if (node.getCollectionItemInfo() != null && !node.isHeading()) out.add(node);
        for (int i = 0; i < node.getChildCount(); i++) collectionItems(node.getChild(i), out);
    }

    private void focusAndCapture(AccessibilityNodeInfo node, String name) throws Exception {
        assertTrue(node.performAction(AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS));
        Thread.sleep(900);
        driver.capture("gpu-a11y-" + name + ".png", 0, 1);
    }

    @Test public void rolesReachServicesAndLiveRegionsAnnounce() throws Exception {
        driver.openTab("Lab");
        driver.reveal("lab-a11y-save");
        AccessibilityNodeInfo heading = driver.node("lab-a11y-heading");
        assertTrue("heading", heading.isHeading());
        AccessibilityNodeInfo list = driver.node("lab-a11y-list");
        assertEquals("android.widget.ListView", list.getClassName());
        assertEquals("rows", 3, list.getCollectionInfo().getRowCount());
        List<AccessibilityNodeInfo> items = new ArrayList<>();
        collectionItems(driver.root(), items);
        assertEquals(3, items.size());
        for (int i = 0; i < 3; i++) assertEquals("row index", i, items.get(i).getCollectionItemInfo().getRowIndex());
        AccessibilityNodeInfo status = driver.node("lab-a11y-status");
        assertEquals(View.ACCESSIBILITY_LIVE_REGION_POLITE, status.getLiveRegion());

        focusAndCapture(heading, "heading");
        focusAndCapture(items.get(1), "item");
        focusAndCapture(list, "list");

        List<AccessibilityEvent> events = new CopyOnWriteArrayList<>();
        driver.automation.setOnAccessibilityEventListener(event -> events.add(AccessibilityEvent.obtain(event)));
        assertTrue(driver.node("lab-a11y-save").performAction(AccessibilityNodeInfo.ACTION_CLICK));
        long end = android.os.SystemClock.uptimeMillis() + 5000;
        AccessibilityEvent live = null;
        while (live == null && android.os.SystemClock.uptimeMillis() < end) {
            for (AccessibilityEvent event : events)
                if (event.getEventType() == AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED
                        && (event.getContentChangeTypes() & AccessibilityEvent.CONTENT_CHANGE_TYPE_TEXT) != 0
                        && event.getText().toString().contains("Saved 1 times")) live = event;
            Thread.sleep(50);
        }
        driver.automation.setOnAccessibilityEventListener(null);
        assertNotNull("polite live-region change event", live);
        Thread.sleep(900);
        driver.capture("gpu-a11y-status.png", 0, 1);
    }
}
