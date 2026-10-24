import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;
public class GpuDeviceLayoutProbe {
 static void check(boolean value,String message){GpuApiProbe.check(value,message);}
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static String text(String id)throws Exception{return GpuApiProbe.text(id);}
 static boolean present(String id){AccessibilityNodeInfo n=GpuRadioProbe.find(GpuApiProbe.a.getRootInActiveWindow(),id);if(n==null)return false;n.recycle();return true;}
 static void open(String id,String title)throws Exception{click(id);check(text("device-mode-title").equals(title),"mode title "+title);check(!present("device-section-network"),"no added section menu");}
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("device-layout");thread.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo service=a.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(service);
   if(args.length>0 && args[0].equals("orientation-logs")){
    AccessibilityNodeInfo log=GpuApiProbe.node("device-orientation-event-scroll");log.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());log.recycle();Thread.sleep(300);
    log=GpuApiProbe.node("device-orientation-event-scroll");android.graphics.Rect box=new android.graphics.Rect();log.getBoundsInScreen(box);check(log.isScrollable(),"long orientation log scrolls independently");check(box.height()>0&&box.height()<=Math.ceil(224*GpuPaddingProbe.density())+2,"log height is bounded to 224 logical pixels");
    check(text("device-orientation-events").contains("Event 079"),"log retains all events");
    for(int i=0;i<30;i++){log.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);Thread.sleep(60);}log.recycle();
    log=GpuApiProbe.node("device-orientation-event-scroll");check((log.getActions()&AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD)!=0,"log advanced within its own viewport");log.recycle();GpuRadioProbe.screenshot("gpu-device-orientation-log-end");
    click("device-orientation-clear");check(text("device-orientation-events").equals("--"),"clear log restores placeholder");GpuChromeProbe.back();System.out.println("PASS bounded orientation log retains all events, scrolls and clears");return;
   }
   if(args.length>0 && args[0].equals("long-ip")){
    AccessibilityNodeInfo ip=GpuApiProbe.node("device-ipv6");android.graphics.Rect box=new android.graphics.Rect();ip.getBoundsInScreen(box);String value=String.valueOf(ip.getText());ip.recycle();
    AccessibilityNodeInfo ipv4=GpuApiProbe.node("device-ipv4");android.graphics.Rect shortBox=new android.graphics.Rect();ipv4.getBoundsInScreen(shortBox);ipv4.recycle();
    android.graphics.Bitmap shot=a.takeScreenshot();check(box.left>=0 && box.right<=shot.getWidth(),"long IPv6 stays within viewport");shot.recycle();check(box.right<=shortBox.right+1,"long IPv6 stays within result card");check(value.equals("2001:db8:1234:5678:90ab:cdef:1234:5678"),"address text is unchanged");check(box.height()>shortBox.height(),"long address wraps across lines");GpuRadioProbe.screenshot("gpu-device-layout-long-ip");GpuChromeProbe.back();System.out.println("PASS long IPv6 fixture wraps within device viewport");return;
   }
   GpuChromeProbe.tab("API");
   open("api-device-device","Device information");click("device-get-info");check(text("device-info-osName").equals("Android"),"device OS");check(text("device-info-model").contains("Pixel"),"device model");GpuRadioProbe.screenshot("gpu-device-layout-info");GpuChromeProbe.back();
   open("api-device-screen","Screen information");click("device-screen-get-info");check(Double.parseDouble(text("device-screen-width").replace("px",""))>0,"screen width");GpuChromeProbe.back();
   open("api-device-network","Network type");check(!present("device-ipv4")&&!present("device-network-listen-start"),"network type has no IP/listener panel");click("device-network-get-info");check(!text("device-network-type").equals("--"),"network result");GpuRadioProbe.screenshot("gpu-device-layout-network");GpuChromeProbe.back();
   open("api-device-localIP","Local IP addresses");check(!present("device-network-type")&&!present("device-network-listen-start"),"IP has no type/listener panel");click("device-network-get-info");check(!text("device-ipv4").equals("--")||!text("device-ipv6").equals("--"),"active network addresses");GpuRadioProbe.screenshot("gpu-device-layout-ip");GpuChromeProbe.back();
   open("api-device-networkStatus","Network status listener");check(!present("device-network-get-info"),"listener has separate controls");click("device-network-listen-start");check(text("device-network-listening").equals("Yes"),"listening");click("device-network-listen-stop");check(text("device-network-listening").equals("No"),"stopped");click("device-network-listen-start");GpuChromeProbe.back();
   open("api-device-networkStatus","Network status listener");check(text("device-network-listening").equals("No"),"fresh listener after leaving");GpuChromeProbe.back();
   open("api-device-orientation","Device orientation");click("device-orientation-listen-start");click("device-orientation-landscape");Thread.sleep(900);android.graphics.Bitmap shot=a.takeScreenshot();check(shot.getWidth()>shot.getHeight(),"actual landscape");shot.recycle();check(text("device-orientation-lock").equals("landscape"),"orientation lock result");GpuRadioProbe.screenshot("gpu-device-layout-landscape");click("device-orientation-portrait");Thread.sleep(900);click("device-orientation-listen-stop");GpuChromeProbe.back();
   open("api-device-phone","Phone call");AccessibilityNodeInfo call=GpuApiProbe.node("device-phone-call");check(!call.isEnabled(),"empty number cannot dial");call.recycle();GpuChromeProbe.back();
   GpuChromeProbe.tab("Home");click("home-appearance-dark");GpuChromeProbe.tab("API");open("api-device-localIP","Local IP addresses");click("device-network-get-info");GpuRadioProbe.screenshot("gpu-device-layout-dark-ip");GpuChromeProbe.back();GpuChromeProbe.tab("Home");click("home-appearance-light");
   System.out.println("PASS distinct Device routes and result fields, no added menus, network listener stop/reentry, physical orientation, empty dial gating and dark IP layout");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
