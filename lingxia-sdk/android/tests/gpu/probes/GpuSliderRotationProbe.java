import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;
public final class GpuSliderRotationProbe {
 static String settings(String... args)throws Exception{
  String[] command=new String[args.length+1];command[0]="/system/bin/settings";System.arraycopy(args,0,command,1,args.length);
  Process p=Runtime.getRuntime().exec(command);GpuRadioProbe.require(p.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&p.exitValue()==0,"settings command");
  String value=new java.io.BufferedReader(new java.io.InputStreamReader(p.getInputStream())).readLine();return value==null?"":value;
 }
 public static void main(String[] args)throws Exception{
  String auto=settings("get","system","accelerometer_rotation"),rotation=settings("get","system","user_rotation");
  HandlerThread thread=new HandlerThread("slider-rotation");thread.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);GpuRadioProbe.automation=automation;
  try{
   UiAutomation.class.getMethod("connect").invoke(automation);android.accessibilityservice.AccessibilityServiceInfo service=automation.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(service);
   settings("put","system","accelerometer_rotation","0");settings("put","system","user_rotation","0");Thread.sleep(700);
   GpuRadioProbe.click("tab-explore");GpuSliderProbe.box();GpuSliderProbe.progress(9);GpuSliderProbe.value(9);AccessibilityNodeInfo original=GpuRadioProbe.node("pause-length");
   settings("put","system","user_rotation","1");Thread.sleep(1400);GpuSliderProbe.box();GpuSliderProbe.value(9);GpuRadioProbe.require(original.refresh()&&original.getRangeInfo().getCurrent()==9,"stable range identity after rotation");GpuRadioProbe.screenshot("gpu-slider-landscape");
   settings("put","system","user_rotation","0");Thread.sleep(1400);GpuSliderProbe.box();GpuSliderProbe.value(9);GpuRadioProbe.require(original.refresh()&&original.getRangeInfo().getCurrent()==9,"stable range identity after return");original.recycle();
   System.out.println("PASS physical portrait/landscape, retained numeric value, stable range identity and reveal after resize");
  }finally{settings("put","system","user_rotation",rotation);settings("put","system","accelerometer_rotation",auto);UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
