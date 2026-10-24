import android.app.UiAutomation;
import android.os.*;

public class GpuHomeFontProbe {
 public static void main(String[] args)throws Exception {
  HandlerThread t=new HandlerThread("home-font");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  String original=null;
  try {
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);
   original=GpuApiProbe.text("home-name");GpuTodoProbe.touch("home-name");
   GpuPaddingProbe.setText("");GpuSearchProbe.shell("/system/bin/input","text","bbb");
   GpuRadioProbe.screenshot("gpu-home-font-typed");Thread.sleep(1000);GpuRadioProbe.screenshot("gpu-home-font-settled");
   GpuPaddingProbe.setText("Hamburgefonts 0123 bbb");Thread.sleep(500);GpuRadioProbe.screenshot("gpu-home-font-focused");
   GpuRadioProbe.key(61);Thread.sleep(300);GpuRadioProbe.key(4);Thread.sleep(500);GpuRadioProbe.screenshot("gpu-home-font-blurred");
   GpuTodoProbe.touch("home-name");Thread.sleep(500);GpuRadioProbe.screenshot("gpu-home-font-refocused");
   System.out.println("CAPTURED Home font focus/blur/refocus; inspect screenshots for glyph distortion");
  }finally {
   if(original!=null){GpuPaddingProbe.setText(original);GpuRadioProbe.key(61);GpuRadioProbe.key(4);}
   UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();
  }
 }
}
