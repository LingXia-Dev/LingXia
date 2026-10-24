import android.app.UiAutomation;
import android.graphics.Rect;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;

/** Real message history exercises virtual row eviction without injecting page state. */
public final class GpuStreamHistoryProbe {
 static boolean visible(AccessibilityNodeInfo node, String text, Rect viewport) {
  if (node == null) return false;
  try {
   Rect bounds = new Rect(); node.getBoundsInScreen(bounds);
   if (text.contentEquals(node.getText() == null ? "" : node.getText())
       && node.isVisibleToUser() && Rect.intersects(bounds, viewport)) return true;
   for (int i = 0; i < node.getChildCount(); i++)
    if (visible(node.getChild(i), text, viewport)) return true;
   return false;
  } finally { node.recycle(); }
 }
 static boolean visible(String text) throws Exception {
  return visible(GpuApiProbe.a.getRootInActiveWindow(), text, GpuThemeProbe.bounds("page-scroll"));
 }
 static void reach(String text, boolean forward) throws Exception {
  for (int i = 0; i < 70; i++) {
   if (visible(text)) return;
   AccessibilityNodeInfo list = GpuApiProbe.node("page-scroll");
   try {
    GpuApiProbe.check(list.performAction(forward ? AccessibilityNodeInfo.ACTION_SCROLL_FORWARD
        : AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD), "history scroll action");
   } finally { list.recycle(); }
   Thread.sleep(200);
  }
  throw new AssertionError("history endpoint unreachable: " + text.substring(0, 24));
 }
 static String message(int index) {
  StringBuilder text = new StringBuilder("History probe message " + index + ": ");
  for (int i = 0; i < 12; i++) text.append("Keep every earlier message reachable when the list recycles its rows. ");
  return text.toString().trim();
 }
 public static void main(String[] args) throws Exception {
  HandlerThread thread = new HandlerThread("stream-history"); thread.start();
  Object connection = Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation automation = (UiAutomation) UiAutomation.class.getConstructor(Looper.class,
      Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(), connection);
  GpuApiProbe.a = automation; GpuRadioProbe.automation = automation;
  try {
   UiAutomation.class.getMethod("connect").invoke(automation);
   android.accessibilityservice.AccessibilityServiceInfo info = automation.getServiceInfo();
   info.flags |= android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
   automation.setServiceInfo(info);
   for (boolean dark : new boolean[]{false, true}) {
    GpuChromeProbe.tab("Home"); GpuApiProbe.click(dark ? "home-appearance-dark" : "home-appearance-light");
    GpuChromeProbe.tab("API"); GpuApiProbe.click("api-stream"); GpuStreamLayoutProbe.empty();
    for (int i = 0; i < 8; i++) {
     GpuStreamAutoHeightProbe.set("stream-input", message(i)); GpuRadioProbe.key(4);
     GpuStreamProbe.clickNow("stream-send");
     GpuUiProbe.waitText("stream-input", ""); GpuStreamLayoutProbe.settled();
     System.out.println("history " + (dark ? "dark" : "light") + " response " + i);
    }
    GpuApiProbe.check(!visible(message(0)), "first message starts outside the last window");
    for (int round = 0; round < 2; round++) {
     reach(message(0), false);
     if (round == 0) GpuRadioProbe.screenshot("gpu-history-first-" + (dark ? "dark" : "light"));
     reach(message(7), true);
     if (round == 0) GpuRadioProbe.screenshot("gpu-history-last-" + (dark ? "dark" : "light"));
    }
    GpuApiProbe.click("stream-clear"); GpuStreamLayoutProbe.empty();
    GpuChromeProbe.back();
   }
   GpuChromeProbe.tab("Home"); GpuApiProbe.click("home-appearance-light");
   System.out.println("PASS real Stream history: 16 long conversations, repeated first/last window recovery, clear, both themes");
  } catch (Throwable error) {
   error.printStackTrace(); throw error;
  } finally {
   UiAutomation.class.getMethod("disconnect").invoke(automation); thread.quitSafely();
  }
 }
}
