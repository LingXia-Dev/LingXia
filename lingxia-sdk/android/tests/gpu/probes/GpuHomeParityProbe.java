import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;
public class GpuHomeParityProbe {
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void check(boolean ok,String s){if(!ok)throw new AssertionError(s);}
 static String text(String id)throws Exception{return GpuApiProbe.text(id);}
 public static void main(String[] args)throws Exception{
 HandlerThread t=new HandlerThread("home-parity");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
 String auto=GpuGridProbe.settings("get","system","accelerometer_rotation"),rotation=GpuGridProbe.settings("get","system","user_rotation");
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);Thread.sleep(700);
 GpuApiProbe.node("home-name").recycle();a.waitForIdle(500,10000);
 check(GpuRadioProbe.find(a.getRootInActiveWindow(),"home-background")==null,"decorative background is excluded from the semantic tree");
 check(GpuRadioProbe.find(a.getRootInActiveWindow(),"open-api")==null,"Home has no extra navigation shortcuts");
 boolean backgroundLoaded=false;
 for(int attempt=0;attempt<40&&!backgroundLoaded;attempt++){
  android.graphics.Bitmap shot=a.takeScreenshot(),pixels=shot.copy(android.graphics.Bitmap.Config.ARGB_8888,false);shot.recycle();
  java.util.HashSet<Integer> colors=new java.util.HashSet<>();
  for(int y=pixels.getHeight()/5;y<pixels.getHeight()*3/4;y+=13){
     int pixel=pixels.getPixel(16,y),r=android.graphics.Color.red(pixel),g=android.graphics.Color.green(pixel),b=android.graphics.Color.blue(pixel);
     if(Math.max(r,Math.max(g,b))-Math.min(r,Math.min(g,b))>30)colors.add(pixel);
    }
  pixels.recycle();backgroundLoaded=colors.size()>15;if(!backgroundLoaded)Thread.sleep(500);
 }
 check(backgroundLoaded,"remote background pixels are decoded and presented");
 GpuRadioProbe.screenshot("gpu-home-background");
 AccessibilityNodeInfo n=GpuApiProbe.node("home-name");Bundle b=new Bundle();b.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,"GPU Ada");check(n.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,b),"name input");n.recycle();GpuUiProbe.waitText("home-name","GPU Ada");a.waitForIdle(300,10000);click("home-greet");check(text("home-greeting").contains("GPU Ada")&&text("home-greeting").contains("#1"),"Logic greeting and flush");
 click("home-language-zh-CN");check(text("home-tagline").equals("\u8f7b\u91cf\u5e94\u7528\u6846\u67b6"),"Chinese copy");click("home-greet");check(text("home-greeting").contains("\u4f60\u597d")&&text("home-greeting").contains("#2"),"Chinese Logic greeting");GpuRadioProbe.screenshot("gpu-home-chinese");
 click("home-appearance-dark");check(text("home-appearance-resolved").equals("\u6df1\u8272"),"dark product preference");GpuGridProbe.settings("put","system","accelerometer_rotation","0");GpuGridProbe.settings("put","system","user_rotation","1");Thread.sleep(1100);GpuApiProbe.node("home-greet").performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());Thread.sleep(300);GpuRadioProbe.screenshot("gpu-home-landscape-dark");
 GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);Thread.sleep(900);
 click("home-language-en-US");click("home-appearance-light");GpuChromeProbe.tab("ToDo");GpuChromeProbe.tab("Home");check(text("home-greeting").contains("#2"),"covered home retains data");check(text("home-name").equals("GPU Ada"),"covered home retains local input");check(text("home-version").startsWith("v1.0.0"),"version");check(!text("home-ip").isBlank(),"network state");
 click("home-language-auto");GpuApiProbe.node("home-name").performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());Thread.sleep(300);GpuRadioProbe.screenshot("gpu-home-parity");
 System.out.println("PASS home greeting/flush, language and appearance preferences, rotation, navigation/local-state retention, version and network status");
 }finally{GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
