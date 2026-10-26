package com.lingxia.ui.tests;

import android.app.Instrumentation;
import android.app.UiAutomation;
import android.graphics.Bitmap;
import android.view.View;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;
import java.io.FileOutputStream;
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
    private final Instrumentation instrumentation = InstrumentationRegistry.getInstrumentation();
    private final UiAutomation automation = instrumentation.getUiAutomation(UiAutomation.FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES);

    private AccessibilityNodeInfo node(String testId) throws Exception {
        String id = instrumentation.getTargetContext().getPackageName() + ":id/" + testId;
        long end = android.os.SystemClock.uptimeMillis() + 15000;
        while (android.os.SystemClock.uptimeMillis() < end) {
            AccessibilityNodeInfo found = byId(automation.getRootInActiveWindow(), id);
            if (found != null) return found;
            Thread.sleep(200);
        }
        throw new AssertionError("no node " + testId);
    }

    /** Virtual nodes are reached by traversal; framework view-id lookup skips node providers. */
    private static AccessibilityNodeInfo byId(AccessibilityNodeInfo node, String id) {
        if (node == null) return null;
        if (id.equals(node.getViewIdResourceName())) return node;
        for (int i = 0; i < node.getChildCount(); i++) {
            AccessibilityNodeInfo found = byId(node.getChild(i), id);
            if (found != null) return found;
        }
        return null;
    }

    private static void collectionItems(AccessibilityNodeInfo node, List<AccessibilityNodeInfo> out) {
        if (node == null) return;
        if (node.getCollectionItemInfo() != null && !node.isHeading()) out.add(node);
        for (int i = 0; i < node.getChildCount(); i++) collectionItems(node.getChild(i), out);
    }

    private static AccessibilityNodeInfo largestScrollable(AccessibilityNodeInfo node) {
        if (node == null) return null;
        AccessibilityNodeInfo best = node.isScrollable() ? node : null;
        for (int i = 0; i < node.getChildCount(); i++) {
            AccessibilityNodeInfo child = largestScrollable(node.getChild(i));
            if (child != null && (best == null || area(child) > area(best))) best = child;
        }
        return best;
    }

    private static long area(AccessibilityNodeInfo node) {
        android.graphics.Rect bounds = new android.graphics.Rect();
        node.getBoundsInScreen(bounds);
        return (long)bounds.width() * bounds.height();
    }

    /** TalkBack asks for a permission and offers its tutorial each time it starts; decline both. */
    private void declineTalkBackPrompts() throws Exception {
        for (int i = 0; i < 6; i++) {
            AccessibilityNodeInfo root = automation.getRootInActiveWindow();
            if (root == null || instrumentation.getTargetContext().getPackageName().contentEquals(root.getPackageName())) return;
            AccessibilityNodeInfo button = null;
            for (String label : new String[] {"Don\u2019t allow", "Don't allow", "Close"})
                for (AccessibilityNodeInfo found : root.findAccessibilityNodeInfosByText(label))
                    if (button == null && found.isClickable()) button = found;
            if (button == null) return;
            button.performAction(AccessibilityNodeInfo.ACTION_CLICK);
            Thread.sleep(1000);
        }
    }

    private void openSection() throws Exception {
        declineTalkBackPrompts();
        android.content.Intent launch = instrumentation.getTargetContext().getPackageManager()
            .getLaunchIntentForPackage(instrumentation.getTargetContext().getPackageName());
        launch.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK);
        instrumentation.getTargetContext().startActivity(launch);
        long end = android.os.SystemClock.uptimeMillis() + 20000;
        AccessibilityNodeInfo tab = null;
        while (tab == null && android.os.SystemClock.uptimeMillis() < end) {
            AccessibilityNodeInfo root = automation.getRootInActiveWindow();
            List<AccessibilityNodeInfo> found = root == null ? List.of() : root.findAccessibilityNodeInfosByText("Lab");
            for (AccessibilityNodeInfo candidate : found) {
                for (AccessibilityNodeInfo n = candidate; n != null && tab == null; n = n.getParent()) if (n.isClickable()) tab = n;
            }
            if (tab == null) Thread.sleep(200);
        }
        assertNotNull("Lab tab", tab);
        assertTrue(tab.performAction(AccessibilityNodeInfo.ACTION_CLICK));
        Thread.sleep(1500);
        String id = instrumentation.getTargetContext().getPackageName() + ":id/lab-a11y-save";
        for (int i = 0; i < 30 && byId(automation.getRootInActiveWindow(), id) == null; i++) {
            AccessibilityNodeInfo page = largestScrollable(automation.getRootInActiveWindow());
            assertNotNull("page scroll", page);
            assertTrue(page.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD));
            Thread.sleep(600);
        }
        assertTrue(node("lab-a11y-save").performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId()));
        Thread.sleep(1500);
    }

    private void focusAndCapture(AccessibilityNodeInfo node, String name) throws Exception {
        assertTrue(node.performAction(AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS));
        Thread.sleep(900);
        capture(name);
    }

    private void capture(String name) throws Exception {
        Bitmap shot = automation.takeScreenshot();
        try (FileOutputStream out = new FileOutputStream(new java.io.File(instrumentation.getTargetContext().getExternalFilesDir(null), "gpu-a11y-" + name + ".png"))) {
            shot.compress(Bitmap.CompressFormat.PNG, 100, out);
        }
    }

    @Test public void rolesReachServicesAndLiveRegionsAnnounce() throws Exception {
        openSection();
        AccessibilityNodeInfo heading = node("lab-a11y-heading");
        assertTrue("heading", heading.isHeading());
        AccessibilityNodeInfo list = node("lab-a11y-list");
        assertEquals("android.widget.ListView", list.getClassName());
        assertEquals("rows", 3, list.getCollectionInfo().getRowCount());
        java.util.ArrayList<AccessibilityNodeInfo> items = new java.util.ArrayList<>();
        collectionItems(automation.getRootInActiveWindow(), items);
        StringBuilder spoken = new StringBuilder();
        for (AccessibilityNodeInfo item : items) spoken.append(item.getText()).append('|').append(item.getContentDescription()).append(';');
        assertEquals(spoken.toString(), 3, items.size());
        for (int i = 0; i < 3; i++) assertEquals("row index", i, items.get(i).getCollectionItemInfo().getRowIndex());
        AccessibilityNodeInfo status = node("lab-a11y-status");
        assertEquals(View.ACCESSIBILITY_LIVE_REGION_POLITE, status.getLiveRegion());

        focusAndCapture(heading, "heading");
        focusAndCapture(items.get(1), "item");
        focusAndCapture(list, "list");

        List<AccessibilityEvent> events = new CopyOnWriteArrayList<>();
        automation.setOnAccessibilityEventListener(event -> events.add(AccessibilityEvent.obtain(event)));
        assertTrue(node("lab-a11y-save").performAction(AccessibilityNodeInfo.ACTION_CLICK));
        long end = android.os.SystemClock.uptimeMillis() + 5000;
        AccessibilityEvent live = null;
        while (live == null && android.os.SystemClock.uptimeMillis() < end) {
            for (AccessibilityEvent event : events)
                if (event.getEventType() == AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED
                        && (event.getContentChangeTypes() & AccessibilityEvent.CONTENT_CHANGE_TYPE_TEXT) != 0
                        && event.getText().toString().contains("Saved 1 times")) live = event;
            Thread.sleep(50);
        }
        automation.setOnAccessibilityEventListener(null);
        assertNotNull("polite live-region change event", live);
        Thread.sleep(1500);
        capture("status");
    }
}
