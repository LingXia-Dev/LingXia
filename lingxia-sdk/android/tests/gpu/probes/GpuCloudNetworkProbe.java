public final class GpuCloudNetworkProbe {
 public static void main(String[] args)throws Exception {
  org.json.JSONObject ready=GpuCloudMqttProbe.post("/broker/wait-ready",new org.json.JSONObject().put("timeout_ms",5000));
  GpuUiProbe.check(ready.getBoolean("ok"),"broker ready from device");
  System.out.println("PASS physical Android reaches local HTTP control plane and broker is ready");
  // app_process has no Activity lifecycle to stop HTTP worker threads.
  System.exit(0);
 }
}
