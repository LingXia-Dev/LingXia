import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;
public class GpuDeviceSystemProbe {
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static String text(String id)throws Exception{return GpuApiProbe.text(id);}
 static void check(boolean value,String message){if(!value)throw new AssertionError(message);}
 public static void main(String[] args)throws Exception{
 HandlerThread t=new HandlerThread("device-system");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);Thread.sleep(800);
 GpuChromeProbe.tab("API");click("api-device-device");click("device-get-info");check(text("device-info-osName").equals("Android"),"device OS");check(text("device-info-model").contains("Pixel"),"physical model");GpuRadioProbe.screenshot("gpu-independent-device");GpuChromeProbe.back();
 click("api-device-screen");click("device-screen-get-info");check(Double.parseDouble(text("device-screen-width"))>0,"screen width");check(Double.parseDouble(text("device-screen-scale"))>0,"screen density");GpuChromeProbe.back();
 click("api-device-network");click("device-network-get-info");check(!text("device-network-type").equals("--"),"network type");GpuChromeProbe.back();
 click("api-device-networkStatus");click("device-network-listen-start");check(text("device-network-listening").equals("Yes"),"listener started");GpuChromeProbe.back();click("api-device-networkStatus");check(text("device-network-listening").equals("No"),"listener stops when the page hides");GpuChromeProbe.back();
 click("api-device-orientation");click("device-orientation-listen-start");click("device-orientation-landscape");Thread.sleep(900);android.graphics.Bitmap shot=a.takeScreenshot();check(shot.getWidth()>shot.getHeight(),"actual landscape");shot.recycle();check(text("device-orientation-lock").equals("landscape"),"orientation result");GpuRadioProbe.screenshot("gpu-independent-orientation");click("device-orientation-portrait");Thread.sleep(900);GpuChromeProbe.back();
 click("api-system-appBaseInfo");click("system-base-info");check(text("system-base-os").equals("Android"),"host OS");check(text("system-base-productName").contains("GPU"),"independent host identity");GpuChromeProbe.back();
 click("api-system-systemSetting");click("system-setting-info");check(text("system-settings-wifiEnabled").matches("true|false"),"system settings");GpuChromeProbe.back();
 click("api-system-autostart");check(text("system-autostart-status").equals("No"),"Android capability absence");check(GpuRadioProbe.find(a.getRootInActiveWindow(),"system-autostart-toggle")==null,"unsupported control absent");GpuChromeProbe.back();
 check(GpuRadioProbe.find(a.getRootInActiveWindow(),"api-system-banner")==null,"desktop banner entry absent on Android");
 click("api-system-cache");click("system-cache-refresh");check(text("system-cache-size").endsWith("bytes"),"cache read");click("system-cache-clear");check(!text("system-cache-status").equals("--"),"cache result");GpuChromeProbe.back();
 click("api-system-notification");click("system-notification-refresh");check(!text("system-notification-status").equals("--"),"notification capability");click("system-notification-show");check(text("system-notification-result").equals("showcase-local"),"scheduled id");click("system-notification-cancel");check(text("system-notification-result").equals("--"),"cancel scheduled notification");GpuRadioProbe.screenshot("gpu-independent-system");GpuChromeProbe.back();GpuChromeProbe.tab("Home");
 System.out.println("PASS independent identity, device/screen/network, listener cleanup, real orientation, system settings, unsupported capability gating, cache and notification schedule/cancel");
 }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}