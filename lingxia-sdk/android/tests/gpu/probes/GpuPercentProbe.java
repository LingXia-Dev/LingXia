import android.app.UiAutomation;
import android.os.*;
import android.graphics.Rect;

public class GpuPercentProbe {
 static void check(boolean value,String message){GpuApiProbe.check(value,message);}
 static void near(double actual,double expected,String message){check(Math.abs(actual-expected)<=2,message+": "+actual+" expected "+expected);}
 static void layout(int width)throws Exception{
  float density=GpuPaddingProbe.density();
  Rect full=GpuBorderProbe.box("percent-full"),half=GpuBorderProbe.box("percent-half"),min=GpuBorderProbe.box("percent-min"),ip=GpuBorderProbe.box("percent-ip");
  near(full.width(),(width-36)*density,"percentage uses padded bordered content box");
  near(half.width(),full.width()/2.0,"Tailwind fraction width");
  near(min.width(),full.width()/2.0,"percentage min width overrides width");
  near(GpuBorderProbe.box("percent-max").width(),full.width()*.72,"percentage max width overrides full width");
  check(ip.width()<=full.width()*.72+2,"long IP respects max width");
  near(ip.right,full.right,"value remains at right edge");
  check(GpuApiProbe.text("percent-ip").equals("2001:db8:1234:5678:90ab:cdef:1234:5678"),"full IP retained");
  check(ip.height()>24*density,"long IP wraps");
  near(GpuBorderProbe.box("percent-height").height(),40*density,"fraction height");
  near(GpuBorderProbe.box("percent-max-height").height(),40*density,"percentage max height");
 }
 public static void main(String[] args)throws Exception{
  HandlerThread t=new HandlerThread("percent-probe");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo service=a.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(service);Thread.sleep(500);
   layout(320);GpuRadioProbe.screenshot("gpu-percent-wide");GpuApiProbe.click("percent-toggle");layout(240);GpuRadioProbe.screenshot("gpu-percent-narrow");GpuApiProbe.click("percent-toggle");layout(320);
   System.out.println("PASS percentage content-box sizing, fractions, min/max constraints, right-aligned full IPv6 and live parent resize");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
