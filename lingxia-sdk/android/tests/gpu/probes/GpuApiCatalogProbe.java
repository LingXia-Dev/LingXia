import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;
public class GpuApiCatalogProbe {
 static void check(boolean value,String message){GpuApiProbe.check(value,message);}
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void tapDescription(String id)throws Exception{
  AccessibilityNodeInfo n=GpuApiProbe.node(id);n.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());n.recycle();Thread.sleep(300);
  n=GpuApiProbe.node(id);check(n.isClickable(),"composed row is clickable");check(n.getChildCount()==0,"composed row exposes one accessible control");
  android.graphics.Rect bounds=new android.graphics.Rect();n.getBoundsInScreen(bounds);n.recycle();
  GpuChartProbe.tap(bounds.left+bounds.width()*0.3f,bounds.top+bounds.height()*0.72f);
 }
 static boolean present(String id){AccessibilityNodeInfo n=GpuRadioProbe.find(GpuApiProbe.a.getRootInActiveWindow(),id);if(n==null)return false;n.recycle();return true;}
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("api-catalog");thread.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo service=a.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(service);
   GpuChromeProbe.tab("API");
   for(String section:new String[]{"bridge","navigation","interface","system","device","media","file"})check(GpuApiProbe.text("api-"+section+"-section").endsWith("collapsed"),"initially collapsed "+section);
   if(present("api-cloud-section"))check(GpuApiProbe.text("api-cloud-section").endsWith("collapsed"),"optional cloud group initially collapsed");
   check(!present("api-ui-navigation"),"collapsed interface has no visible entry");
   GpuApiProbe.node("api-bridge-section").performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());Thread.sleep(300);GpuRadioProbe.screenshot("gpu-api-composed-collapsed");
   click("api-system-section");GpuApiProbe.node("api-system-cache").recycle();check(!present("api-system-banner"),"mobile hides desktop banner");
   click("api-system-section");check(!present("api-system-cache"),"collapse removes entries");
   click("api-device-section");GpuApiProbe.node("api-device-localIP").recycle();GpuApiProbe.node("api-device-networkStatus").recycle();
   tapDescription("api-clipboard");GpuApiProbe.node("clipboard-write-text").recycle();GpuChromeProbe.back();
   check(GpuApiProbe.text("api-device-section").endsWith("expanded"),"expanded group retained across navigation");click("api-device-section");
   click("api-file-section");click("api-choose-file");GpuApiProbe.node("file-choose").recycle();check(!present("file-pdf-open"),"choose-file route selects choose demo");GpuChromeProbe.back();
   click("api-file");GpuApiProbe.node("file-pdf-open").recycle();check(!present("file-choose"),"open-file route selects open demo");GpuChromeProbe.back();click("api-file-section");
   click("api-bridge-section");GpuApiProbe.node("api-stream").performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());Thread.sleep(300);GpuRadioProbe.screenshot("gpu-api-composed-bridge");
   GpuChromeProbe.tab("Home");click("home-appearance-dark");GpuChromeProbe.tab("API");
   check(GpuApiProbe.text("api-bridge-section").endsWith("expanded"),"expanded group retained across tabs/theme");GpuRadioProbe.screenshot("gpu-api-composed-dark");
   GpuChromeProbe.tab("Home");click("home-appearance-light");
   System.out.println("PASS API collapsed groups, expand/collapse, platform gating, native navigation retention, distinct file routes and theme retention");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
