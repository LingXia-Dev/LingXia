import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import java.net.HttpURLConnection;
import java.net.URL;
import org.json.JSONObject;
public final class GpuCloudMqttProbe {
 static int subscriptionCount()throws Exception{
  String clientId=System.getenv("GPU_MQTT_CLIENT_ID");
  GpuUiProbe.check(clientId!=null&&!clientId.isEmpty(),"set GPU_MQTT_CLIENT_ID from mock broker connection log");
  return post("/broker/subscriptions/count",new JSONObject().put("client_id",clientId).put("topic","lingxia/tenant/tenant-a/demo/mqtt")).getInt("count");
 }
 static JSONObject post(String path,JSONObject body)throws Exception{
  HttpURLConnection connection=(HttpURLConnection)new URL("http://127.0.0.1:39881"+path).openConnection();
  connection.setRequestMethod("POST");connection.setConnectTimeout(5000);connection.setReadTimeout(10000);connection.setDoOutput(true);connection.setRequestProperty("Content-Type","application/json");
  try{try(java.io.OutputStream out=connection.getOutputStream()){out.write(body.toString().getBytes("UTF-8"));}GpuUiProbe.check(connection.getResponseCode()==200,"mock broker response");try(java.io.InputStream in=connection.getInputStream()){java.io.ByteArrayOutputStream out=new java.io.ByteArrayOutputStream();byte[] buffer=new byte[1024];int count;while((count=in.read(buffer))!=-1)out.write(buffer,0,count);return new JSONObject(out.toString("UTF-8"));}}finally{connection.disconnect();}
 }
 static void publish(int count)throws Exception{post("/broker/publish",new JSONObject().put("topic","lingxia/tenant/tenant-a/demo/mqtt").put("payload","GPU verified message").put("properties",new JSONObject().put("payloadFormatIndicator","utf8").put("contentType","text/plain")).put("qos",1).put("count",count).put("interval_ms",100));}
 public static void main(String[] args)throws Exception{
  HandlerThread t=new HandlerThread("cloud-mqtt-probe");t.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),connection);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1000);
   GpuApiProbe.click("open-api");GpuApiProbe.click("api-cloud-mqtt");GpuUiProbe.waitText("cloud-mqtt-active","Active");
   GpuUiProbe.check(subscriptionCount()==1,"broker subscription present");
   publish(3);GpuUiProbe.waitText("cloud-mqtt-count","3");GpuUiProbe.waitText("cloud-mqtt-last-topic","lingxia/tenant/tenant-a/demo/mqtt");GpuUiProbe.contains("cloud-mqtt-payload","GPU verified message");
   GpuApiProbe.click("cloud-mqtt-stop");GpuUiProbe.waitText("cloud-mqtt-active","Inactive");publish(1);Thread.sleep(500);GpuUiProbe.waitText("cloud-mqtt-count","3");
   GpuUiProbe.check(subscriptionCount()==0,"broker unsubscribe");
   GpuApiProbe.click("cloud-mqtt-start");GpuUiProbe.waitText("cloud-mqtt-active","Active");publish(1);GpuUiProbe.waitText("cloud-mqtt-count","4");GpuRadioProbe.screenshot("gpu-cloud-provider-mqtt");
   GpuChromeProbe.back();Thread.sleep(500);GpuUiProbe.check(subscriptionCount()==0,"leave cleanup");GpuChromeProbe.back();
   System.out.println("PASS real MQTT subscribe/messages, unsubscribe silence, restart and broker-side leave cleanup");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
  // Exit only after assertions and UiAutomation cleanup have succeeded.
  System.exit(0);
 }
}
