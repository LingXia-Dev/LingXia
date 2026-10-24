import android.app.UiAutomation;
import android.os.*;
public final class GpuAuthLayoutProbe {
 static void fixture(String mode)throws Exception{try(java.io.InputStream in=new java.net.URL("http://127.0.0.1:39590/__probe/auth"+mode).openStream()){while(in.read()!=-1){}}Thread.sleep(500);}
 public static void main(String[] args)throws Exception{
 HandlerThread t=new HandlerThread("auth-layout");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);Thread.sleep(900);
 for(boolean dark:new boolean[]{false,true}){
 GpuChromeProbe.tab("Home");GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");GpuApiProbe.click("api-cloud-auth");String original=GpuApiProbe.text("cloud-current-tenant");fixture("/empty");
 GpuUiProbe.waitText("cloud-current-tenant","No active tenant");GpuUiProbe.waitText("cloud-current-user","Not signed in");GpuApiProbe.node("cloud-login").recycle();GpuUiProbe.waitText("cloud-tenants-count","0");GpuApiProbe.node("cloud-identities-empty").recycle();GpuRadioProbe.screenshot("gpu-auth-empty-"+(dark?"dark":"light"));
 try(java.io.InputStream in=new java.net.URL("http://127.0.0.1:39590/__probe/auth").openStream()){while(in.read()!=-1){}}Thread.sleep(500);
 GpuUiProbe.waitText("cloud-current-tenant","Design Studio");GpuUiProbe.waitText("cloud-current-user","Alex Chen");GpuUiProbe.waitText("cloud-user-id","designer-42");GpuRadioProbe.screenshot("gpu-auth-current-"+(dark?"dark":"light"));GpuUiProbe.waitText("cloud-tenants-count","3");GpuFileLayoutProbe.enabled("cloud-activate-fixture-active",false);GpuFileLayoutProbe.enabled("cloud-activate-fixture-other",true);GpuFileLayoutProbe.enabled("cloud-activate-2",false);GpuRadioProbe.screenshot("gpu-auth-tenants-"+(dark?"dark":"light"));
 GpuApiProbe.click("cloud-activate-fixture-other");GpuUiProbe.waitText("cloud-auth-status","Identity fixture-other not found");GpuChromeProbe.back();
 GpuApiProbe.click("api-cloud-auth");GpuUiProbe.waitText("cloud-current-tenant",original);GpuChromeProbe.back();
 }
 GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS auth light/dark empty and identity/image fixtures, displayName, tenant gating, actual missing-identity activation and fresh reentry");
 }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}System.exit(0);
 }
}
