import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;

public final class GpuImageSemanticsProbe {
    public static void main(String[] args) throws Exception {
        HandlerThread thread = new HandlerThread("image-semantics");
        thread.start();
        Class<?> connectionType = Class.forName("android.app.IUiAutomationConnection");
        Object connection = Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
        UiAutomation automation = (UiAutomation) UiAutomation.class
                .getConstructor(Looper.class, connectionType).newInstance(thread.getLooper(), connection);
        GpuRadioProbe.automation = automation;
        GpuApiProbe.a = automation;
        try {
            UiAutomation.class.getMethod("connect").invoke(automation);
            android.accessibilityservice.AccessibilityServiceInfo info = automation.getServiceInfo();
            info.flags |= android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
            automation.setServiceInfo(info);
            GpuChromeProbe.awaitHomeReady();
            GpuChromeProbe.tab("Home");
            GpuApiProbe.node("home-name").recycle();
            for (String id : new String[] { "home-background", "home-logo" }) {
                AccessibilityNodeInfo decorative = GpuRadioProbe.find(automation.getRootInActiveWindow(), id);
                GpuUiProbe.check(decorative == null, "decorative image excluded: " + id);
            }
            GpuChromeProbe.tab("API");
            GpuApiProbe.click("api-clipboard");
            GpuApiProbe.click("clipboard-write-sample-image");
            GpuUiProbe.waitText("clipboard-status", "Wrote sample PNG");
            GpuApiProbe.click("clipboard-read-all");
            GpuUiProbe.waitText("clipboard-status", "Read image");
            AccessibilityNodeInfo image = GpuApiProbe.node("clipboard-image");
            try {
                GpuUiProbe.check("Image from clipboard".contentEquals(image.getContentDescription()), "image alternative text");
                GpuUiProbe.check("android.widget.ImageView".contentEquals(image.getClassName()), "image role");
                GpuUiProbe.check(!image.isClickable(), "description does not create an action");
                image.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());
            } finally {
                image.recycle();
            }
            Thread.sleep(500);
            GpuRadioProbe.screenshot("gpu-image-alt-clipboard");
            GpuChromeProbe.back();
            GpuChromeProbe.tab("Home");
            System.out.println("PASS decorative images excluded, clipboard image description and role preserved");
        } catch (Throwable error) {
            error.printStackTrace();
            throw error;
        } finally {
            UiAutomation.class.getMethod("disconnect").invoke(automation);
            thread.quitSafely();
        }
    }
}
