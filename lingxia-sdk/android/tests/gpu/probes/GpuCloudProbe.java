import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;

public final class GpuCloudProbe {
 static final String MISSING="This build does not include the cloud provider";
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static void expect(String id,String value)throws Exception{GpuUiProbe.waitText(id,value);}
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("cloud-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=automation;GpuApiProbe.a=automation;
  try{
   UiAutomation.class.getMethod("connect").invoke(automation);
   android.accessibilityservice.AccessibilityServiceInfo info=automation.getServiceInfo();
   info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(info);Thread.sleep(900);
   click("open-api");click("api-cloud-auth");
   expect("cloud-auth-status",MISSING);
   click("cloud-login");expect("cloud-auth-status",MISSING);
   click("cloud-add-identity");expect("cloud-auth-status",MISSING);
   click("cloud-logout");expect("cloud-auth-status",MISSING);
   expect("cloud-current-tenant","No active tenant");expect("cloud-identities-empty","No saved identities");
   GpuRadioProbe.screenshot("gpu-independent-cloud-auth");GpuChromeProbe.back();
   click("api-cloud-mqtt");expect("cloud-mqtt-status",MISSING);expect("cloud-mqtt-active","Inactive");
   click("cloud-mqtt-start");expect("cloud-mqtt-status",MISSING);
   click("cloud-mqtt-stop");expect("cloud-mqtt-status","Demo subscription is inactive");
   expect("cloud-mqtt-count","0");GpuChromeProbe.back();
   click("api-cloud-functions");expect("cloud-auth-status",MISSING);
   for(String name:new String[]{"echo","whoami","fail"}){
    click("cloud-function-"+name);expect("cloud-functions-status",MISSING);expect("cloud-functions-last-call",name);
   }
   GpuRadioProbe.screenshot("gpu-independent-cloud-functions");GpuChromeProbe.back();
   click("api-cloud-mqtt");expect("cloud-mqtt-status",MISSING);expect("cloud-mqtt-count","0");GpuChromeProbe.back();GpuChromeProbe.back();
   System.out.println("PASS cloud Auth/MQTT/functions GPU routes, missing-provider feedback, inactive stop and fresh MQTT reentry");
  }catch(Throwable error){error.printStackTrace();throw error;}
  finally{UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
