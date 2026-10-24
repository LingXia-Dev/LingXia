import android.app.UiAutomation;
import android.os.*;
public final class GpuFunctionsLayoutProbe {
 static void fixture(String mode)throws Exception{try(java.io.InputStream in=new java.net.URL("http://127.0.0.1:39590/__probe/functions/"+mode).openStream()){while(in.read()!=-1){}}Thread.sleep(400);}
 public static void main(String[] args)throws Exception{
 HandlerThread t=new HandlerThread("functions-layout");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);Thread.sleep(900);
 for(boolean dark:new boolean[]{false,true}){
 GpuChromeProbe.tab("Home");GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");GpuApiProbe.click("api-cloud-functions");
 GpuUiProbe.waitText("cloud-functions-result","(no result yet)");GpuRadioProbe.screenshot("gpu-functions-initial-"+(dark?"dark":"light"));
 for(String name:new String[]{"echo","whoami","fail"}){GpuApiProbe.click("cloud-function-"+name);GpuUiProbe.waitText("cloud-functions-status","Cannot reach the cloud service.");GpuUiProbe.waitText("cloud-functions-last-call",name);}
 fixture("result");GpuUiProbe.waitText("cloud-functions-status","echo succeeded");GpuUiProbe.check(GpuApiProbe.text("cloud-functions-result").contains("world"),"result fixture");GpuRadioProbe.screenshot("gpu-functions-result-"+(dark?"dark":"light"));
 fixture("empty");GpuApiProbe.node("cloud-functions-empty").recycle();GpuUiProbe.waitText("cloud-functions-last-call","-");GpuRadioProbe.screenshot("gpu-functions-empty-"+(dark?"dark":"light"));GpuChromeProbe.back();
 GpuApiProbe.click("api-cloud-functions");GpuApiProbe.node("cloud-function-echo").recycle();GpuUiProbe.waitText("cloud-functions-result","(no result yet)");GpuChromeProbe.back();
 }
 GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS functions light/dark calls with unreachable-service feedback, result/empty fixtures and fresh reentry");
 }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}System.exit(0);
 }
}
