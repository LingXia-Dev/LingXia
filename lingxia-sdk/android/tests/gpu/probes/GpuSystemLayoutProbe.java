import android.app.UiAutomation;
import android.os.*;
public final class GpuSystemLayoutProbe {
 public static void main(String[]args)throws Exception{
 HandlerThread t=new HandlerThread("system-layout");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);Thread.sleep(900);
 for(boolean dark:new boolean[]{false,true}){
 GpuChromeProbe.tab("Home");GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");
 GpuApiProbe.click("api-system-appBaseInfo");GpuUiProbe.waitText("system-mode-title","app.getBaseInfo");GpuApiProbe.click("system-get-base");GpuUiProbe.waitText("system-base-os","Android");GpuUiProbe.check(GpuApiProbe.text("system-base-productName").contains("GPU"),"host identity");GpuRadioProbe.screenshot("gpu-system-base-"+(dark?"dark":"light"));GpuChromeProbe.back();
 GpuApiProbe.click("api-system-systemSetting");GpuApiProbe.click("system-get-settings");GpuUiProbe.check(GpuApiProbe.text("system-settings-wifiEnabled").matches("Yes|No"),"actual settings");GpuRadioProbe.screenshot("gpu-system-settings-"+(dark?"dark":"light"));GpuChromeProbe.back();
 GpuApiProbe.click("api-system-autostart");GpuUiProbe.waitText("system-autostart-status","No");GpuApiProbe.click("system-autostart-refresh");GpuUiProbe.waitText("system-autostart-status","No");GpuRadioProbe.screenshot("gpu-system-autostart-"+(dark?"dark":"light"));GpuChromeProbe.back();
 GpuApiProbe.click("api-system-cache");GpuApiProbe.click("system-cache-refresh");GpuUiProbe.check(GpuApiProbe.text("system-cache-size").matches("[0-9.]+ (B|KB|MB|GB)"),"cache estimate");GpuRadioProbe.screenshot("gpu-system-cache-"+(dark?"dark":"light"));GpuChromeProbe.back();
 GpuApiProbe.click("api-system-notification");GpuApiProbe.click("system-notification-refresh");GpuApiProbe.click("system-notification-target-page");GpuApiProbe.click("system-notification-target-native");GpuApiProbe.click("system-notification-target-activate");GpuApiProbe.click("system-notification-show");GpuUiProbe.waitText("system-notification-result","showcase-local");GpuApiProbe.click("system-notification-cancel");GpuUiProbe.waitText("system-notification-result","--");GpuRadioProbe.screenshot("gpu-system-notification-"+(dark?"dark":"light"));GpuChromeProbe.back();
 }
 GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS system light/dark API routes, identity/settings, unsupported autostart, cache estimate, notification targets and schedule/cancel");
 }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}System.exit(0);
 }
}
