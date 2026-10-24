import android.app.UiAutomation;
import android.os.*;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;
public final class GpuStreamLayoutProbe {
 static void settled()throws Exception{for(int j=0;j<100;j++){AccessibilityNodeInfo n=GpuApiProbe.node("stream-input");boolean ready=n.isEnabled();n.recycle();if(ready){GpuApiProbe.node("stream-clear").recycle();return;}Thread.sleep(100);}throw new AssertionError("stream did not settle");}
 static void empty()throws Exception{GpuApiProbe.node("stream-empty").recycle();AccessibilityNodeInfo n=GpuApiProbe.node("stream-send");GpuUiProbe.check(!n.isEnabled(),"empty send disabled");n.recycle();}
 static void centeredEmpty()throws Exception{
  android.graphics.Rect viewport=GpuThemeProbe.bounds("page-scroll"),title=GpuThemeProbe.bounds("stream-empty"),hint=GpuThemeProbe.bounds("stream-hint-How does the bridge protocol work?");
  float scale=GpuThemeProbe.bounds("stream-send").height()/36f;
  float contentTop=title.top-76*scale;
  GpuUiProbe.check(contentTop>=viewport.top&&viewport.contains(hint),"empty content fits message viewport");
  GpuUiProbe.check(Math.abs((contentTop+hint.bottom)/2f-viewport.exactCenterY())<=3,"empty content is vertically centered");
 }
 public static void main(String[]args)throws Exception{
  HandlerThread t=new HandlerThread("stream-layout");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);Thread.sleep(900);
   for(boolean dark:new boolean[]{false,true}){
    GpuChromeProbe.tab("Home");GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");GpuApiProbe.click("api-stream");empty();centeredEmpty();GpuRadioProbe.screenshot(dark?"gpu-stream-empty-dark":"gpu-stream-empty-light");
    GpuStreamProbe.set("Tell me about LingXia streaming");GpuRadioProbe.key(KeyEvent.KEYCODE_BACK);GpuStreamProbe.clickNow("stream-send");GpuStreamProbe.firstToken();GpuRadioProbe.screenshot(dark?"gpu-stream-live-dark":"gpu-stream-live-light");settled();GpuRadioProbe.screenshot(dark?"gpu-stream-complete-dark":"gpu-stream-complete-light");
    GpuApiProbe.click("stream-clear");empty();GpuStreamProbe.set("Cancel this response");GpuRadioProbe.key(KeyEvent.KEYCODE_BACK);GpuStreamProbe.clickNow("stream-send");GpuStreamProbe.clickNow("stream-stop");settled();GpuApiProbe.click("stream-clear");empty();
    GpuStreamProbe.set("Leave while streaming");GpuRadioProbe.key(KeyEvent.KEYCODE_BACK);GpuStreamProbe.clickNow("stream-send");GpuChromeProbe.back();GpuApiProbe.click("api-stream");empty();GpuChromeProbe.back();
   }
   GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS stream light/dark empty, live and completed layout, cancel, clear, teardown and fresh entry");
  }catch(Throwable e){e.printStackTrace();throw e;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
