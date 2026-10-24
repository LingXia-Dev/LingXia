import android.app.UiAutomation;
import android.os.*;
import android.graphics.*;
import android.view.accessibility.AccessibilityNodeInfo;
public class GpuStrikeProbe {
 static int bands(String id)throws Exception{
  AccessibilityNodeInfo n=GpuApiProbe.node(id);Rect r=new Rect();n.getBoundsInScreen(r);n.recycle();
  Bitmap shot=GpuApiProbe.a.takeScreenshot(),b=shot.copy(Bitmap.Config.ARGB_8888,false);shot.recycle();
  try{int x=r.left+Math.round(128*GpuPaddingProbe.density()),bands=0;boolean ink=false;
   for(int y=r.top;y<r.bottom;y++){int c=b.getPixel(x,y);boolean next=Color.red(c)<100&&Color.green(c)<100&&Color.blue(c)<100;if(next&&!ink)bands++;ink=next;}return bands;
  }finally{b.recycle();}
 }
 public static void main(String[] args)throws Exception{
  HandlerThread t=new HandlerThread("strike-probe");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(700);
   GpuApiProbe.check(bands("strike-lines")==0,"spaces contain no ink before decoration");GpuApiProbe.click("strike-toggle");GpuApiProbe.check(bands("strike-lines")==2,"one metric-positioned strike per line through spaces");GpuApiProbe.check(bands("strike-scroll")==1,"viewport clips second strike");
   AccessibilityNodeInfo scroll=GpuApiProbe.node("strike-scroll");scroll.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);scroll.recycle();Thread.sleep(500);GpuApiProbe.check(bands("strike-scroll")==2,"retained scrolling reveals both strikes");GpuRadioProbe.screenshot("gpu-strike-fixture");
   GpuApiProbe.click("strike-toggle");GpuApiProbe.check(bands("strike-lines")==0&&bands("strike-scroll")==0,"removing decoration restores cached text pixels");System.out.println("PASS multiline strike, descendant none propagation, clipped and retained scroll, cached toggle pixels");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
