import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;

public final class GpuCloudIdentityProbe {
 static void authorize(String name)throws Exception{
  android.view.accessibility.AccessibilityNodeInfo node=GpuUiProbe.waitNative(name);
  android.graphics.Rect bounds=new android.graphics.Rect();node.getBoundsInScreen(bounds);node.recycle();
  GpuUiProbe.check(!bounds.isEmpty(),"visible authorization link");
  Process tap=Runtime.getRuntime().exec(new String[]{"/system/bin/input","tap",String.valueOf(bounds.centerX()),String.valueOf(bounds.centerY())});
  GpuUiProbe.check(tap.waitFor()==0,"authorization tap");
 }
 static void identity(String tenant,String user)throws Exception{
  GpuUiProbe.waitText("cloud-current-tenant",tenant);GpuUiProbe.waitText("cloud-current-user",user);
 }
 static void whoami(String tenant)throws Exception{
  GpuChromeProbe.back();GpuApiProbe.click("api-cloud-functions");
  GpuApiProbe.click("cloud-function-whoami");GpuUiProbe.waitText("cloud-functions-status","whoami succeeded");
  GpuUiProbe.contains("cloud-functions-result",tenant);
  GpuChromeProbe.back();GpuApiProbe.click("api-cloud-auth");
 }
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("cloud-identity-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=automation;GpuApiProbe.a=automation;
  try{
   UiAutomation.class.getMethod("connect").invoke(automation);
   android.accessibilityservice.AccessibilityServiceInfo info=automation.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(info);Thread.sleep(1000);
   GpuApiProbe.click("open-api");GpuApiProbe.click("api-cloud-auth");identity("Tenant A","User A");
   GpuApiProbe.click("cloud-add-identity");authorize("Tenant B");
   identity("Tenant B","User B");whoami("tenant-b");
   GpuApiProbe.click("cloud-activate-tenant-a");identity("Tenant A","User A");whoami("tenant-a");
   GpuApiProbe.click("cloud-activate-tenant-b");identity("Tenant B","User B");
   GpuRadioProbe.screenshot("gpu-cloud-identities");
   GpuApiProbe.click("cloud-logout");
   identity("Tenant A","User A");whoami("tenant-a");
   GpuChromeProbe.back();GpuChromeProbe.back();
   System.out.println("PASS add identity, tenant switching, server identity isolation, logout and remaining identity recovery");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
