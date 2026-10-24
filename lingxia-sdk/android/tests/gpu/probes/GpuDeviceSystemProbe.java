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
 click("open-api");click("api-device-device");click("device-get-info");check(text("device-info-osName").equals("Android"),"device OS");check(text("device-info-model").contains("Pixel"),"physical model");GpuRadioProbe.screenshot("gpu-independent-device");
 click("device-section-screen");click("device-screen-get-info");check(Double.parseDouble(text("device-screen-width"))>0,"screen width");check(Double.parseDouble(text("device-screen-scale"))>0,"screen density");
 click("device-section-network");click("device-network-get-info");check(!text("device-network-networkType").isEmpty(),"network type");click("device-network-listen");check(text("device-network-listening").equals("Listening"),"listener started");click("device-section-orientation");click("device-section-network");check(text("device-network-listening").equals("Not listening"),"listener stops on section change");
 click("device-section-orientation");click("device-orientation-listen");click("device-orientation-landscape");Thread.sleep(900);android.graphics.Bitmap shot=a.takeScreenshot();check(shot.getWidth()>shot.getHeight(),"actual landscape");shot.recycle();check(text("device-orientation-lock").equals("Requested: landscape"),"orientation result");GpuRadioProbe.screenshot("gpu-independent-orientation");click("device-orientation-portrait");Thread.sleep(900);GpuChromeProbe.back();
 click("api-system-appBaseInfo");click("system-get-base");check(text("system-base-os").equals("Android"),"host OS");check(text("system-base-productName").contains("GPU"),"independent host identity");
 click("system-section-systemSetting");click("system-get-settings");check(text("system-settings-wifiEnabled").matches("true|false"),"system settings");
 click("system-section-autostart");check(text("system-autostart-status").contains("unavailable"),"Android capability absence");AccessibilityNodeInfo n=GpuApiProbe.node("system-autostart-toggle");check(!n.isEnabled(),"unsupported control disabled");n.recycle();
 click("system-section-banner");check(text("system-banner-status").contains("unavailable"),"desktop banner unavailable");
 click("system-section-cache");click("system-cache-refresh");check(text("system-cache-size").endsWith("bytes"),"cache read");click("system-cache-clear");check(!text("system-cache-status").equals("Ready"),"cache result");
 click("system-section-notification");click("system-notification-refresh");check(text("system-notification-status").startsWith("Permission:"),"notification capability");click("system-notification-show");check(text("system-notification-result").equals("showcase-local"),"scheduled id");click("system-notification-cancel");check(text("system-notification-result").equals("No pending notification"),"cancel scheduled notification");GpuRadioProbe.screenshot("gpu-independent-system");GpuChromeProbe.back();GpuChromeProbe.back();
 System.out.println("PASS independent identity, device/screen/network, listener cleanup, real orientation, system settings, unsupported capability gating, cache and notification schedule/cancel");
 }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}