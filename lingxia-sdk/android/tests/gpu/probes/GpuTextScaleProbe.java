import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;

public final class GpuTextScaleProbe {
    private static Rect bounds(String id) throws Exception {
        AccessibilityNodeInfo node = GpuApiProbe.node(id);
        node.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());
        node.recycle();
        Thread.sleep(250);
        node = GpuApiProbe.node(id);
        Rect bounds = new Rect();
        node.getBoundsInScreen(bounds);
        node.recycle();
        return bounds;
    }

    public static void main(String[] args) throws Exception {
        String original = GpuGridProbe.settings("get", "system", "font_scale");
        HandlerThread thread = new HandlerThread("text-scale");
        thread.start();
        Class<?> connectionType = Class.forName("android.app.IUiAutomationConnection");
        Object connection = Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
        UiAutomation automation = (UiAutomation) UiAutomation.class.getConstructor(Looper.class, connectionType)
                .newInstance(thread.getLooper(), connection);
        GpuRadioProbe.automation = automation;
        GpuApiProbe.a = automation;
        try {
            UiAutomation.class.getMethod("connect").invoke(automation);
            android.accessibilityservice.AccessibilityServiceInfo info = automation.getServiceInfo();
            info.flags |= android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
            automation.setServiceInfo(info);
            GpuChromeProbe.awaitHomeReady();
            int normalTitle = 0, normalInput = 0, normalTodoInput = 0, normalTabLabel = 0;
            String previousInput = null;
            for (String scale : new String[] { "1.0", "2.0", "1.0" }) {
                GpuGridProbe.settings("put", "system", "font_scale", scale);
                Thread.sleep(1800);
                GpuChromeProbe.tab("Home");
                AccessibilityNodeInfo tabLabel = GpuChromeProbe.tabText(automation.getRootInActiveWindow(), "Home");
                Rect tabBounds = new Rect();
                GpuUiProbe.check(tabLabel != null, "native Home tab label");
                tabLabel.getBoundsInScreen(tabBounds); tabLabel.recycle();
                if (normalTabLabel == 0) normalTabLabel = tabBounds.height();
                else if (scale.equals("2.0"))
                    GpuUiProbe.check(tabBounds.height() > normalTabLabel * 1.7, "native tab label has room for enlarged text");
                else GpuUiProbe.check(Math.abs(tabBounds.height() - normalTabLabel) <= 2, "native tab label restores default height");
                if (previousInput != null)
                    GpuUiProbe.waitText("home-name", previousInput);
                Rect title = bounds("home-title"), input = bounds("home-name");
                System.out.println("scale=" + scale + " title=" + title + " input=" + input);
                GpuRadioProbe.screenshot("gpu-text-scale-geometry-" + scale);
                if (normalTitle == 0) {
                    normalTitle = title.height(); normalInput = input.height();
                } else if (scale.equals("2.0")) {
                    GpuUiProbe.check(title.height() > normalTitle * 1.7, "system font preference reaches GPU text");
                    GpuUiProbe.check(input.height() > normalInput, "input grows with enlarged text");
                } else {
                    GpuUiProbe.check(Math.abs(title.height() - normalTitle) <= 2, "default text geometry restored");
                    GpuUiProbe.check(Math.abs(input.height() - normalInput) <= 2, "default input geometry restored");
                }
                GpuUiProbe.set("home-name", "Readable " + scale);
                GpuApiProbe.click("home-greet");
                GpuUiProbe.contains("home-greeting", "Readable " + scale);
                previousInput = "Readable " + scale;
                GpuRadioProbe.screenshot("gpu-text-scale-" + scale);
                GpuChromeProbe.tab("ToDo");
                int todoHeight = bounds("todo-input").height();
                if (normalTodoInput == 0) normalTodoInput = todoHeight;
                else if (scale.equals("2.0"))
                    GpuUiProbe.check(todoHeight > normalTodoInput, "ToDo input grows with inherited line height");
                else GpuUiProbe.check(Math.abs(todoHeight - normalTodoInput) <= 2, "ToDo input restores default height");
                GpuRadioProbe.screenshot("gpu-text-scale-todo-" + scale);
                GpuChromeProbe.tab("Home");
                if (scale.equals("2.0")) {
                    GpuChromeProbe.tab("API");
                    GpuApiProbe.click("api-stream");
                    GpuStreamLayoutProbe.empty();
                    for (int index = 0; index < 2; index++) {
                        GpuStreamAutoHeightProbe.set("stream-input", GpuStreamHistoryProbe.message(index));
                        GpuRadioProbe.key(4);
                        GpuStreamProbe.clickNow("stream-send");
                        GpuUiProbe.waitText("stream-input", "");
                        GpuStreamLayoutProbe.settled();
                    }
                    GpuStreamHistoryProbe.reach(GpuStreamHistoryProbe.message(0), false);
                    GpuStreamHistoryProbe.reach(GpuStreamHistoryProbe.message(1), true);
                    GpuRadioProbe.screenshot("gpu-text-scale-stream");
                    GpuApiProbe.click("stream-clear");
                    GpuStreamLayoutProbe.empty();
                    GpuChromeProbe.back();
                    GpuChromeProbe.tab("Home");
                }
            }
            System.out.println("PASS system font scaling, growing input, greeting, large-font Stream history and default geometry restoration");
        } catch (Throwable error) {
            error.printStackTrace(); throw error;
        } finally {
            if (original.equals("null") || original.isEmpty()) GpuGridProbe.settings("delete", "system", "font_scale");
            else GpuGridProbe.settings("put", "system", "font_scale", original);
            UiAutomation.class.getMethod("disconnect").invoke(automation);
            thread.quitSafely();
        }
    }
}
