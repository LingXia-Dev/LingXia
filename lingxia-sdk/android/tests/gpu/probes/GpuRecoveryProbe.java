import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;

/** Runs against the standalone host built with fixtures/recovery. */
public class GpuRecoveryProbe {
    static UiAutomation automation;
    static AccessibilityNodeInfo find(AccessibilityNodeInfo node, String id) {
        if (node == null) return null;
        if (("com.lingxia.ui.smoke:id/" + id).equals(node.getViewIdResourceName())) return node;
        for (int i = 0; i < node.getChildCount(); i++) {
            AccessibilityNodeInfo found = find(node.getChild(i), id);
            if (found != null) return found;
        }
        return null;
    }
    static AccessibilityNodeInfo node(String id) throws Exception {
        for (int i = 0; i < 30; i++) {
            AccessibilityNodeInfo found = find(automation.getRootInActiveWindow(), id);
            if (found != null) return found;
            Thread.sleep(100);
        }
        throw new AssertionError("missing " + id);
    }
    static String value(String id) throws Exception {
        AccessibilityNodeInfo found = node(id);
        String result = String.valueOf(found.getText()); found.recycle(); return result;
    }
    static AccessibilityNodeInfo text(AccessibilityNodeInfo node, String expected) {
        if (node == null) return null;
        if (expected.equalsIgnoreCase(String.valueOf(node.getText()))) return node;
        for (int i = 0; i < node.getChildCount(); i++) {
            AccessibilityNodeInfo found = text(node.getChild(i), expected);
            if (found != null) return found;
        }
        return null;
    }
    public static void main(String[] args) throws Exception {
        HandlerThread thread = new HandlerThread("recovery-probe"); thread.start();
        Object connection = Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
        automation = (UiAutomation)UiAutomation.class.getConstructor(Looper.class,
                Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(), connection);
        GpuApiProbe.a = automation; GpuRadioProbe.automation = automation;
        try {
            UiAutomation.class.getMethod("connect").invoke(automation);
            android.accessibilityservice.AccessibilityServiceInfo info = automation.getServiceInfo();
            info.flags |= android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
            automation.setServiceInfo(info);
            GpuApiProbe.check(value("recovery-count").equals("Count 0"), "initial count");
            node("recovery-increment").performAction(AccessibilityNodeInfo.ACTION_CLICK);
            for (int i = 0; i < 30 && !value("recovery-count").equals("Count 1"); i++) Thread.sleep(100);
            GpuApiProbe.check(value("recovery-count").equals("Count 1"), "committed state before failure");
            node("recovery-trigger").performAction(AccessibilityNodeInfo.ACTION_CLICK);
            AccessibilityNodeInfo retry = null;
            for (int i = 0; i < 30 && retry == null; i++) {
                retry = text(automation.getRootInActiveWindow(), "Retry");
                if (retry == null) Thread.sleep(100);
            }
            GpuApiProbe.check(retry != null, "recovery action appears");
            GpuApiProbe.check(value("recovery-count").equals("Count 1"), "last committed frame retained");
            GpuRadioProbe.screenshot("gpu-recovery-failed");
            retry.performAction(AccessibilityNodeInfo.ACTION_CLICK); retry.recycle();
            for (int i = 0; i < 30 && text(automation.getRootInActiveWindow(), "Retrying…") != null; i++) Thread.sleep(100);
            GpuApiProbe.check(value("recovery-count").equals("Count 1"), "failed mutation rolls back and retry preserves committed state");
            GpuApiProbe.check(value("recovery-draft").equals("Retained draft"), "draft preserved");
            GpuApiProbe.check(text(automation.getRootInActiveWindow(), "Retrying…") == null, "feedback cleared after presentation");
            GpuRadioProbe.screenshot("gpu-recovery-restored");
            node("recovery-timeout").performAction(AccessibilityNodeInfo.ACTION_CLICK);
            AccessibilityNodeInfo restart = null;
            for (int i = 0; i < 50 && restart == null; i++) {
                restart = text(automation.getRootInActiveWindow(), "Reload page");
                if (restart == null) Thread.sleep(100);
            }
            GpuApiProbe.check(restart != null, "interrupted session offers reload");
            GpuApiProbe.check(value("recovery-count").equals("Count 1"), "timeout retains last committed frame");
            GpuRadioProbe.screenshot("gpu-recovery-timeout");
            restart.performAction(AccessibilityNodeInfo.ACTION_CLICK); restart.recycle();
            for (int i = 0; i < 50 && !value("recovery-count").equals("Count 0"); i++) Thread.sleep(100);
            GpuApiProbe.check(value("recovery-count").equals("Count 0"), "reload creates fresh local state");
            node("recovery-increment").performAction(AccessibilityNodeInfo.ACTION_CLICK);
            for (int i = 0; i < 30 && !value("recovery-count").equals("Count 1"); i++) Thread.sleep(100);
            GpuApiProbe.check(value("recovery-count").equals("Count 1"), "fresh session accepts interaction");
            GpuRadioProbe.screenshot("gpu-recovery-restarted");
            System.out.println("PASS candidate rollback and retry preserve committed state; timeout retains frame and explicit reload restores interaction");
        } catch (Throwable error) {
            error.printStackTrace(); throw error;
        } finally { UiAutomation.class.getMethod("disconnect").invoke(automation); thread.quitSafely(); }
    }
}
