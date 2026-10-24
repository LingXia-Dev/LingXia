import android.app.UiAutomation;
import android.graphics.Rect;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;

/** Run at a 320 logical-pixel portrait viewport, from Home. */
public class GpuChannelScrollProbe {
 static void check(boolean ok,String message){if(!ok)throw new AssertionError(message);}
 static void swipe(int x1,int y1,int x2,int y2)throws Exception {
  java.lang.Process process=Runtime.getRuntime().exec(new String[]{"/system/bin/input","swipe",String.valueOf(x1),String.valueOf(y1),String.valueOf(x2),String.valueOf(y2),"600"});
  check(process.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&process.exitValue()==0,"physical swipe completed");Thread.sleep(800);
 }
 static void scroll(String id,int action)throws Exception {
  AccessibilityNodeInfo node=GpuApiProbe.node(id);
  try{check(node.performAction(action),"accessible scroll action "+id);}finally{node.recycle();}
  Thread.sleep(700);
 }
 public static void main(String[] args)throws Exception {
  HandlerThread thread=new HandlerThread("channel-scroll");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=automation;GpuApiProbe.a=automation;
  try {
   UiAutomation.class.getMethod("connect").invoke(automation);
   android.accessibilityservice.AccessibilityServiceInfo info=automation.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(info);Thread.sleep(800);
   float density=GpuPaddingProbe.density();
   android.graphics.Bitmap screen=automation.takeScreenshot();float width=screen.getWidth()/density;screen.recycle();
   check(width>=315&&width<=325,"requires 320 logical-pixel viewport, got "+width);
   for(boolean dark:new boolean[]{false,true}) {
    GpuChromeProbe.tab("Home");GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");
    GpuChromeProbe.tab("API");GpuApiProbe.click("api-channel");GpuChannelProbe.waitText("channel-status","Connected");GpuChannelProbe.price();
    Rect strip=GpuThemeProbe.bounds("channel-symbols"),middle=GpuThemeProbe.bounds("channel-symbol-GOOGL"),price=GpuThemeProbe.bounds("channel-price");
    AccessibilityNodeInfo region=GpuApiProbe.node("channel-symbols");
    check(region.isScrollable()&&"android.widget.HorizontalScrollView".contentEquals(region.getClassName()),"horizontal accessible region");region.recycle();
    swipe(strip.right-(int)(15*density),strip.top+(int)(15*density),strip.left+(int)(15*density),strip.top+(int)(15*density));
    check(GpuThemeProbe.bounds("channel-symbol-GOOGL").left<middle.left-10*density,"horizontal drag reveals trailing tabs");
    check(Math.abs(GpuThemeProbe.bounds("channel-price").top-price.top)<2,"horizontal gesture does not move page");
    GpuTodoProbe.touch("channel-symbol-TSLA");GpuChannelProbe.waitText("channel-active-symbol","TSLA");GpuChannelProbe.price();
    GpuRadioProbe.screenshot(dark?"gpu-channel-scroll-dark":"gpu-channel-scroll-light");
    scroll("channel-symbols",AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD);
    check(Math.abs(GpuThemeProbe.bounds("channel-symbol-AAPL").left-strip.left)<2,"accessible backward restores horizontal origin");
    scroll("channel-symbols",AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
    check(GpuThemeProbe.bounds("channel-symbol-GOOGL").left<middle.left-10*density,"accessible forward uses width");
    boolean pageScrollable=false;
    for(int attempt=0;attempt<50&&!pageScrollable;attempt++) {
     AccessibilityNodeInfo page=GpuApiProbe.node("page-scroll");pageScrollable=page.isScrollable();page.recycle();
     if(!pageScrollable)Thread.sleep(200);
    }
    check(pageScrollable,"incoming history makes outer page scrollable");
    strip=GpuThemeProbe.bounds("channel-symbols");price=GpuThemeProbe.bounds("channel-price");
    swipe(strip.centerX(),strip.top+(int)(15*density),strip.centerX(),strip.top-(int)(100*density));
    check(GpuThemeProbe.bounds("channel-price").top<price.top-10*density,"vertical drag on symbol strip scrolls outer page");
    GpuChromeProbe.back();
   }
   GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");
   System.out.println("PASS narrow Channel physical horizontal/vertical gestures, trailing-symbol tap and accessible scroll in both themes");
  } catch(Throwable error) {error.printStackTrace();throw error;}
  finally {UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
