import android.app.UiAutomation;
import android.os.*;
import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;

public final class GpuClipboardLayoutProbe {
 static void height(String id,float expected,float scale)throws Exception {
  Rect bounds=GpuThemeProbe.bounds(id);
  GpuApiProbe.check(Math.abs(bounds.height()/scale-expected)<1.5f,id+" height "+bounds.height()/scale);
 }
 public static void main(String[] args)throws Exception {
  HandlerThread thread=new HandlerThread("clipboard-layout");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuApiProbe.a=automation;GpuRadioProbe.automation=automation;
  try {
   UiAutomation.class.getMethod("connect").invoke(automation);
   android.accessibilityservice.AccessibilityServiceInfo info=automation.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(info);
   float scale=GpuPaddingProbe.density();
   for(boolean dark:new boolean[]{false,true}) {
    GpuChromeProbe.tab("Home");GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");
    GpuChromeProbe.tab("API");GpuApiProbe.click("api-clipboard");
    height("clipboard-draft",78,scale);height("clipboard-write-text",48,scale);
    Rect write=GpuThemeProbe.bounds("clipboard-write-text"),read=GpuThemeProbe.bounds("clipboard-read-text");
    GpuApiProbe.check(write.right<read.left&&Math.abs(write.top-read.top)<2,"text actions stay in separate aligned columns");
    GpuRadioProbe.screenshot(dark?"gpu-clipboard-sizing-dark":"gpu-clipboard-sizing-light");
    GpuApiProbe.click("clipboard-draft");
    AccessibilityNodeInfo draft=GpuApiProbe.node("clipboard-draft");Bundle value=new Bundle();value.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,"Three lines\nKeyboard input\nStill visible");
    GpuApiProbe.check(draft.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,value),"set multiline draft");draft.recycle();Thread.sleep(500);
    GpuApiProbe.check(GpuApiProbe.text("clipboard-draft").contains("Still visible"),"multiline value retained");height("clipboard-draft",78,scale);
    GpuRadioProbe.screenshot(dark?"gpu-clipboard-sizing-ime-dark":"gpu-clipboard-sizing-ime-light");GpuRadioProbe.key(4);
    AccessibilityNodeInfo sample=GpuApiProbe.node("clipboard-write-sample-image");sample.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());sample.recycle();Thread.sleep(400);
    Rect sampleBox=GpuThemeProbe.bounds("clipboard-write-sample-image"),chooseBox=GpuThemeProbe.bounds("clipboard-choose-image");
    GpuApiProbe.check(sampleBox.right<chooseBox.left&&Math.abs(sampleBox.top-chooseBox.top)<2,"image actions stay in separate aligned columns");
    GpuRadioProbe.screenshot(dark?"gpu-clipboard-sizing-image-dark":"gpu-clipboard-sizing-image-light");GpuChromeProbe.back();
    GpuApiProbe.click("api-share");height("share-text",48,scale);height("share-page",48,scale);
    GpuRadioProbe.screenshot(dark?"gpu-share-sizing-dark":"gpu-share-sizing-light");GpuChromeProbe.back();
   }
   GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");
   System.out.println("PASS clipboard three-line editor, action geometry, multiline IME and share actions in both themes");
  }finally {UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
