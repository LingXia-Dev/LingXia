package com.lingxia.ui.tests;

import android.app.Instrumentation;
import android.app.UiAutomation;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.Rect;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.io.FileOutputStream;
import java.util.List;
import static org.junit.Assert.*;

/** Drives the host as an accessibility service does; GPU virtual nodes are reached by traversal. */
final class A11yDriver {
    final Instrumentation instrumentation;
    final UiAutomation automation;

    A11yDriver(Instrumentation instrumentation) {
        this.instrumentation = instrumentation;
        automation = instrumentation.getUiAutomation(UiAutomation.FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES);
    }

    String id(String testId) { return instrumentation.getTargetContext().getPackageName() + ":id/" + testId; }

    AccessibilityNodeInfo root() { return automation.getRootInActiveWindow(); }

    AccessibilityNodeInfo node(String testId) throws Exception {
        long end = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < end) {
            AccessibilityNodeInfo found = byId(root(), id(testId));
            if (found != null) return found;
            Thread.sleep(200);
        }
        throw new AssertionError("no node " + testId);
    }

    /** Framework view-id lookup skips node providers. */
    static AccessibilityNodeInfo byId(AccessibilityNodeInfo node, String id) {
        if (node == null) return null;
        if (id.equals(node.getViewIdResourceName())) return node;
        for (int i = 0; i < node.getChildCount(); i++) {
            AccessibilityNodeInfo found = byId(node.getChild(i), id);
            if (found != null) return found;
        }
        return null;
    }

    static AccessibilityNodeInfo largestScrollable(AccessibilityNodeInfo node) {
        if (node == null) return null;
        AccessibilityNodeInfo best = node.isScrollable() ? node : null;
        for (int i = 0; i < node.getChildCount(); i++) {
            AccessibilityNodeInfo child = largestScrollable(node.getChild(i));
            if (child != null && (best == null || area(child) > area(best))) best = child;
        }
        return best;
    }

    static long area(AccessibilityNodeInfo node) {
        Rect bounds = new Rect();
        node.getBoundsInScreen(bounds);
        return (long)bounds.width() * bounds.height();
    }

    /** TalkBack asks for a permission and offers its tutorial each time it starts; decline both. */
    void declineTalkBackPrompts() throws Exception {
        for (int i = 0; i < 6; i++) {
            AccessibilityNodeInfo root = root();
            if (root == null || instrumentation.getTargetContext().getPackageName().contentEquals(root.getPackageName())) return;
            AccessibilityNodeInfo button = null;
            for (String label : new String[] {"Don’t allow", "Don't allow", "Close"})
                for (AccessibilityNodeInfo found : root.findAccessibilityNodeInfosByText(label))
                    if (button == null && found.isClickable()) button = found;
            if (button == null) return;
            button.performAction(AccessibilityNodeInfo.ACTION_CLICK);
            Thread.sleep(1000);
        }
    }

    /** Launch the host and open the tab labelled `label`. */
    void openTab(String label) throws Exception {
        declineTalkBackPrompts();
        Intent launch = instrumentation.getTargetContext().getPackageManager()
            .getLaunchIntentForPackage(instrumentation.getTargetContext().getPackageName());
        launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        instrumentation.getTargetContext().startActivity(launch);
        long end = SystemClock.uptimeMillis() + 20000;
        AccessibilityNodeInfo tab = null;
        while (tab == null && SystemClock.uptimeMillis() < end) {
            AccessibilityNodeInfo root = root();
            List<AccessibilityNodeInfo> found = root == null ? List.of() : root.findAccessibilityNodeInfosByText(label);
            for (AccessibilityNodeInfo candidate : found)
                for (AccessibilityNodeInfo n = candidate; n != null && tab == null; n = n.getParent()) if (n.isClickable()) tab = n;
            if (tab == null) Thread.sleep(200);
        }
        assertNotNull(label + " tab", tab);
        assertTrue(tab.performAction(AccessibilityNodeInfo.ACTION_CLICK));
        Thread.sleep(1500);
    }

    /** Scroll the page until `testId` is in the tree, then onto the screen. */
    AccessibilityNodeInfo reveal(String testId) throws Exception {
        for (int i = 0; i < 30 && byId(root(), id(testId)) == null; i++) {
            AccessibilityNodeInfo page = largestScrollable(root());
            assertNotNull("page scroll", page);
            assertTrue(page.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD));
            Thread.sleep(600);
        }
        AccessibilityNodeInfo found = node(testId);
        found.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());
        Thread.sleep(1000);
        return node(testId);
    }

    File out(String name) { return new File(instrumentation.getTargetContext().getExternalFilesDir(null), name); }

    /** Screenshot, optionally cropped to `[top, bottom)` as fractions of the height. */
    void capture(String name, float top, float bottom) throws Exception {
        Bitmap shot = automation.takeScreenshot();
        if (top > 0 || bottom < 1) {
            int y = (int)(shot.getHeight() * top);
            shot = Bitmap.createBitmap(shot, 0, y, shot.getWidth(), (int)(shot.getHeight() * bottom) - y);
        }
        try (FileOutputStream stream = new FileOutputStream(out(name))) {
            shot.compress(Bitmap.CompressFormat.PNG, 100, stream);
        }
    }
}
