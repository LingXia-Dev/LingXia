import android.app.UiAutomation;
import android.os.*;
public final class GpuMqttLayoutProbe {
 public static void main(String[] args)throws Exception{
 HandlerThread t=new HandlerThread("mqtt-layout");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);Thread.sleep(900);
 for(boolean dark:new boolean[]{false,true}){
 GpuChromeProbe.tab("Home");GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");GpuApiProbe.click("api-cloud-mqtt");
 GpuUiProbe.waitText("cloud-mqtt-status","Cannot reach the cloud service.");GpuUiProbe.waitText("cloud-mqtt-active","inactive");GpuFileLayoutProbe.enabled("cloud-mqtt-stop",false);GpuApiProbe.click("cloud-mqtt-start");GpuUiProbe.waitText("cloud-mqtt-status","Cannot reach the cloud service.");GpuUiProbe.waitText("cloud-mqtt-count","0");GpuApiProbe.node("cloud-mqtt-empty").recycle();GpuRadioProbe.screenshot("gpu-mqtt-empty-"+(dark?"dark":"light"));
 try(java.io.InputStream in=new java.net.URL("http://127.0.0.1:39590/__probe/mqtt").openStream()){while(in.read()!=-1){}}Thread.sleep(400);
 GpuUiProbe.waitText("cloud-mqtt-state","connected");GpuUiProbe.waitText("cloud-mqtt-count","1");GpuFileLayoutProbe.enabled("cloud-mqtt-start",false);GpuFileLayoutProbe.enabled("cloud-mqtt-stop",true);GpuUiProbe.check(GpuApiProbe.text("cloud-mqtt-payload").contains("Hello from the cloud"),"message fixture");GpuRadioProbe.screenshot("gpu-mqtt-message-"+(dark?"dark":"light"));
 GpuApiProbe.click("cloud-mqtt-stop");GpuUiProbe.waitText("cloud-mqtt-active","inactive");GpuUiProbe.waitText("cloud-mqtt-status","Demo subscription is inactive");GpuFileLayoutProbe.enabled("cloud-mqtt-stop",false);GpuChromeProbe.back();
 GpuApiProbe.click("api-cloud-mqtt");GpuUiProbe.waitText("cloud-mqtt-count","0");GpuChromeProbe.back();
 }
 GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS MQTT light/dark unreachable-service feedback, empty state, connected/message fixture, control gating, stop and fresh reentry");
 }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}System.exit(0);
 }
}
