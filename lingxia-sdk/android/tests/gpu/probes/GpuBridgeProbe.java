import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;

public final class GpuBridgeProbe {
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("bridge-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);
   android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();
   info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1100);
   for(boolean dark:new boolean[]{false,true}) {
   GpuChromeProbe.tab("Home");GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");GpuApiProbe.click("api-bridge");
   GpuUiProbe.waitText("bootstrap-verdict","Bootstrap: PASS");
   GpuUiProbe.waitText("bootstrap-snapshot","initial snapshot: yes");
   GpuUiProbe.waitText("bootstrap-ready","bridge ready: yes");
   GpuApiProbe.click("btn-echo");GpuUiProbe.contains("stat-echo","echo #1 ok");
   GpuApiProbe.click("btn-echo");GpuUiProbe.contains("stat-echo","echo #2 ok");
   GpuRadioProbe.screenshot(dark?"gpu-bridge-layout-dark":"gpu-bridge-layout-light");
   GpuApiProbe.click("btn-reconnect");GpuUiProbe.waitText("stat-echo","reconnect hook unavailable (GPU transport)");GpuUiProbe.waitText("stat-reconnects","reconnects: 0");
   GpuApiProbe.click("btn-restart");Thread.sleep(1400);
   GpuUiProbe.waitText("stream-verdict","Stream: PASS");
   GpuApiProbe.click("btn-stop");Thread.sleep(300);Thread.sleep(300);
   String stopped=GpuApiProbe.text("stat-received");
   int received=Integer.parseInt(stopped.substring("received: ".length()));
   GpuUiProbe.check(received>=10,"received at least ten ticks");
   GpuUiProbe.waitText("stat-last","last seq: "+received);
   GpuUiProbe.waitText("stat-gaps","gaps: none");GpuUiProbe.waitText("stat-error","stream error: none");
   Thread.sleep(500);GpuUiProbe.waitText("stat-received",stopped);
   int frames=GpuChannelProbe.frames();Thread.sleep(500);GpuUiProbe.check(GpuChannelProbe.frames()==frames,"stopped stream produces no idle frames");
   GpuApiProbe.click("btn-restart");Thread.sleep(400);GpuUiProbe.waitText("stream-verdict","Stream: PASS");
   android.view.accessibility.AccessibilityNodeInfo stop=GpuApiProbe.node("btn-stop");
   android.graphics.Rect bounds=new android.graphics.Rect();stop.getBoundsInScreen(bounds);stop.recycle();
   String x=String.valueOf(bounds.centerX()),y=String.valueOf(bounds.centerY());
   Process touch=Runtime.getRuntime().exec(new String[]{"/system/bin/input","swipe",x,y,x,y,"300"});
   GpuUiProbe.check(touch.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&touch.exitValue()==0,"held touch injection");
   Thread.sleep(300);
   GpuApiProbe.click("btn-restart");Thread.sleep(400);
   GpuRadioProbe.screenshot("gpu-independent-bridge-bootstrap");
   GpuChromeProbe.back();GpuApiProbe.click("api-bridge");
   GpuUiProbe.waitText("bootstrap-verdict","Bootstrap: PASS");GpuUiProbe.waitText("stat-echo","echo: -");
   GpuUiProbe.waitText("stat-received","received: 0");GpuUiProbe.waitText("stream-verdict","Stream: WAITING");
   GpuChromeProbe.back();
   }
   GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");
   System.out.println("PASS light/dark layout, bootstrap/snapshot/echo, gap-free ticks, accessibility/held-touch stop across redraw, idle, restart and active-stream exit/fresh entry");
  }catch(Throwable e){e.printStackTrace();throw e;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
