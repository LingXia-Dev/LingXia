import android.app.UiAutomation;
import android.os.*;
import android.graphics.*;
import android.view.accessibility.AccessibilityNodeInfo;

public class GpuBorderProbe {
 static float scale;
 static void check(boolean value,String message){GpuApiProbe.check(value,message);}
 static Rect box(String id)throws Exception{AccessibilityNodeInfo n=GpuApiProbe.node(id);Rect r=new Rect();n.getBoundsInScreen(r);n.recycle();return r;}
 static void tap(String id,float x,float y)throws Exception{Rect r=box(id);GpuSearchProbe.shell("/system/bin/input","tap",String.valueOf(r.left+Math.round(x*scale)),String.valueOf(r.top+Math.round(y*scale)));Thread.sleep(350);}
 static void pixel(String id,int x,int y,int expected)throws Exception{
  Rect r=box(id);Bitmap shot=GpuApiProbe.a.takeScreenshot(),b=shot.copy(Bitmap.Config.ARGB_8888,false);shot.recycle();
  int actual=b.getPixel(r.left+Math.round(x*scale),r.top+Math.round(y*scale));b.recycle();
  check(Math.abs(Color.red(actual)-Color.red(expected))<=3&&Math.abs(Color.green(actual)-Color.green(expected))<=3&&Math.abs(Color.blue(actual)-Color.blue(expected))<=3,id+" pixel "+x+","+y+" got "+Integer.toHexString(actual));
 }
 static void status(String value)throws Exception{check(GpuApiProbe.text("border-status").equals(value),"status expected "+value+" got "+GpuApiProbe.text("border-status"));}
 public static void main(String[] args)throws Exception{
  HandlerThread t=new HandlerThread("border-probe");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo service=a.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(service);Thread.sleep(500);scale=GpuPaddingProbe.density();
   pixel("border-ring",4,32,0xffff7f7f);pixel("border-ring",9,9,0xffff7f7f);pixel("border-ring",80,14,0xffffffff);
   pixel("border-scroll",5,40,0xffff0000);pixel("border-scroll",10,5,0xffff0000);pixel("border-scroll",50,40,0xff0000ff);
   tap("border-scroll",10,5);status("0/0/-1");tap("border-scroll",50,40);status("1/0/-1");
   AccessibilityNodeInfo scroll=GpuApiProbe.node("border-scroll");for(int i=0;i<5;i++){scroll.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);Thread.sleep(180);}scroll.recycle();
   pixel("border-scroll",5,40,0xffff0000);pixel("border-scroll",50,40,0xff00ff00);
   tap("border-slider",130,30);status("1/5/-1");pixel("border-slider",10,30,0xffff0000);
   tap("border-plot",5,40);status("1/5/-1");tap("border-plot",9,3);status("1/5/-1");tap("border-plot",130,40);status("1/5/1");
   pixel("border-image",4,32,0xffff0000);
   AccessibilityNodeInfo input=GpuApiProbe.node("border-input");Bundle text=new Bundle();text.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,"abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz");check(input.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,text),"replace bordered input");input.recycle();Thread.sleep(300);
   pixel("border-input",2,28,0xffff0000);pixel("border-input",278,28,0xffff0000);pixel("border-input",140,2,0xffff0000);
   input=GpuApiProbe.node("border-input");text.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,"MMMM");check(input.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,text),"reset bordered input");input.recycle();GpuPaddingProbe.waitKeyboard(true);
   tap("border-input",14,28);GpuSearchProbe.shell("/system/bin/input","text","Z");GpuUiProbe.waitText("border-input","ZMMMM");GpuRadioProbe.key(4);GpuPaddingProbe.waitKeyboard(false);
   GpuRadioProbe.screenshot("gpu-border-fixture");
   System.out.println("PASS translucent border corners, currentColor, asymmetric scroll pixels/hit clipping, slider and plot geometry, image border and long editor foreground");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
