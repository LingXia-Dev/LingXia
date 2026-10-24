import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;
public final class GpuClipboardImageProbe {
 public static void main(String[] args)throws Exception {
  HandlerThread t=new HandlerThread("clipboard-image-probe");t.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),connection);
  GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try {
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1000);
   GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-dark");GpuUiProbe.waitText("home-appearance-resolved","Dark");GpuRadioProbe.screenshot("gpu-clipboard-home-dark");GpuChromeProbe.tab("API");GpuRadioProbe.screenshot("gpu-clipboard-api-dark");GpuApiProbe.click("api-clipboard");GpuApiProbe.node("clipboard-draft").recycle();boolean dark=false;for(int frame=0;frame<25&&!dark;frame++){android.graphics.Bitmap rawFrame=a.takeScreenshot();android.graphics.Bitmap darkFrame=rawFrame.copy(android.graphics.Bitmap.Config.ARGB_8888,false);rawFrame.recycle();try{int pixel=darkFrame.getPixel(5,darkFrame.getHeight()/3);dark=android.graphics.Color.red(pixel)<80&&android.graphics.Color.green(pixel)<80&&android.graphics.Color.blue(pixel)<80;}finally{darkFrame.recycle();}if(!dark)Thread.sleep(200);}GpuUiProbe.check(dark,"dark clipboard surface");
   GpuRadioProbe.screenshot("gpu-clipboard-layout-dark");
   GpuApiProbe.click("clipboard-draft");AccessibilityNodeInfo draft=GpuApiProbe.node("clipboard-draft");android.os.Bundle textArgs=new android.os.Bundle();textArgs.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,"GPU clipboard keyboard");GpuUiProbe.check(draft.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,textArgs),"edit clipboard draft");draft.recycle();GpuRadioProbe.key(66);Thread.sleep(300);GpuUiProbe.check(GpuApiProbe.text("clipboard-draft").contains("\n"),"Enter inserts a newline");GpuRadioProbe.screenshot("gpu-clipboard-keyboard-dark");GpuRadioProbe.key(4);
   GpuApiProbe.click("clipboard-choose-image");GpuUiProbe.clickNative("Album");Thread.sleep(700);
   AccessibilityNodeInfo image=GpuMediaImageProbe.firstPhoto(a.getRootInActiveWindow());GpuUiProbe.check(image!=null,"album fixture");GpuUiProbe.check(image.performAction(AccessibilityNodeInfo.ACTION_CLICK),"select fixture");image.recycle();GpuUiProbe.clickNative("Done");
   GpuUiProbe.waitText("clipboard-status","Wrote chosen image");GpuApiProbe.click("clipboard-read-all");GpuUiProbe.waitText("clipboard-status","Read image");
   GpuUiProbe.contains("clipboard-types-value","image");
   image=GpuApiProbe.node("clipboard-image");image.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());image.recycle();Thread.sleep(700);
   image=GpuApiProbe.node("clipboard-image");android.graphics.Rect box=new android.graphics.Rect();image.getBoundsInScreen(box);image.recycle();
   GpuUiProbe.check(box.width()>300&&Math.abs(box.width()-box.height())<3,"square image preview");
   android.graphics.Bitmap capture=a.takeScreenshot();GpuUiProbe.check(Math.abs(box.centerX()-capture.getWidth()/2)<5,"centered image preview");capture.recycle();
   GpuRadioProbe.screenshot("gpu-clipboard-selected-image");
   GpuApiProbe.click("clipboard-choose-image");GpuUiProbe.clickNative("Album");Thread.sleep(500);GpuRadioProbe.key(4);Thread.sleep(500);
   GpuUiProbe.waitText("clipboard-status","Image picker canceled");GpuApiProbe.node("clipboard-image").recycle();
   GpuApiProbe.click("clipboard-read-all");GpuUiProbe.waitText("clipboard-status","Read image");
   String auto=GpuGridProbe.settings("get","system","accelerometer_rotation"),rotation=GpuGridProbe.settings("get","system","user_rotation");
   try{GpuGridProbe.settings("put","system","accelerometer_rotation","0");GpuGridProbe.settings("put","system","user_rotation","1");Thread.sleep(1000);GpuApiProbe.node("clipboard-image").recycle();GpuUiProbe.waitText("clipboard-status","Read image");GpuApiProbe.node("clipboard-image").recycle();GpuRadioProbe.screenshot("gpu-clipboard-image-landscape");}
   finally{GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);Thread.sleep(800);}
   GpuApiProbe.click("clipboard-clear");GpuApiProbe.click("clipboard-read-all");GpuUiProbe.waitText("clipboard-status","Clipboard empty");GpuChromeProbe.back();
   System.out.println("PASS chosen-image clipboard roundtrip, picker cancellation preserves clipboard, rotation and clear");
  }catch(Throwable error){error.printStackTrace();throw error;}finally{try{GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");}catch(Throwable cleanup){System.err.println("Theme cleanup: "+cleanup);}finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}}
 }
}
