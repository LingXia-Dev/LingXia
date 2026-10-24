import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
public final class GpuCloudProviderProbe {
 public static void main(String[] args)throws Exception{
  HandlerThread t=new HandlerThread("cloud-provider-probe");t.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),connection);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1000);
   GpuApiProbe.click("open-api");GpuApiProbe.click("api-cloud-auth");
   if(GpuApiProbe.text("cloud-current-tenant").equals("Tenant A")){GpuApiProbe.click("cloud-logout");GpuUiProbe.waitText("cloud-current-tenant","No active tenant");}
   GpuApiProbe.click("cloud-login");
   // WebView exposes the link label separately from its clickable ancestor.
   android.view.accessibility.AccessibilityNodeInfo tenant=GpuUiProbe.waitNative("Tenant A");
   android.graphics.Rect bounds=new android.graphics.Rect();tenant.getBoundsInScreen(bounds);tenant.recycle();
   GpuUiProbe.check(!bounds.isEmpty(),"visible tenant authorization link");
   Process tap=Runtime.getRuntime().exec(new String[]{"/system/bin/input","tap",String.valueOf(bounds.centerX()),String.valueOf(bounds.centerY())});
   GpuUiProbe.check(tap.waitFor()==0,"tap tenant authorization link");
   GpuUiProbe.waitText("cloud-current-tenant","Tenant A");GpuUiProbe.waitText("cloud-current-user","User A");GpuRadioProbe.screenshot("gpu-cloud-provider-auth");
   GpuChromeProbe.back();GpuApiProbe.click("api-cloud-functions");GpuApiProbe.click("cloud-function-echo");GpuUiProbe.waitText("cloud-functions-status","echo succeeded");GpuUiProbe.contains("cloud-functions-result","world");
   GpuApiProbe.click("cloud-function-whoami");GpuUiProbe.waitText("cloud-functions-status","whoami succeeded");GpuUiProbe.contains("cloud-functions-result","tenant-a");
   GpuApiProbe.click("cloud-function-fail");GpuUiProbe.contains("cloud-functions-status","function execution failed");
   GpuApiProbe.click("cloud-function-echo");GpuUiProbe.waitText("cloud-functions-status","echo succeeded");GpuRadioProbe.screenshot("gpu-cloud-provider-functions");
   GpuChromeProbe.back();GpuChromeProbe.back();
   System.out.println("PASS provider login, tenant/user identity, echo/whoami, intentional failure and subsequent recovery");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
