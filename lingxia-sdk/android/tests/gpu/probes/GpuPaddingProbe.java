import android.app.UiAutomation;
import android.os.*;
import android.graphics.*;
import android.view.accessibility.AccessibilityNodeInfo;

public class GpuPaddingProbe {
 static float density()throws Exception {
  java.util.regex.Matcher m=java.util.regex.Pattern.compile("density:\\s*(\\d+)").matcher(GpuSearchProbe.shell("/system/bin/wm","density"));
  float result=0;while(m.find())result=Integer.parseInt(m.group(1))/160f;
  GpuApiProbe.check(result>0,"display density");return result;
 }
 static void setText(String value)throws Exception {
  AccessibilityNodeInfo n=GpuApiProbe.node("home-name");Bundle args=new Bundle();args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,value);
  GpuApiProbe.check(n.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,args),"set name");n.recycle();GpuUiProbe.waitText("home-name",value);GpuApiProbe.a.waitForIdle(300,10000);
 }
 static Rect input()throws Exception {
  AccessibilityNodeInfo n=GpuApiProbe.node("home-name");n.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());n.recycle();GpuApiProbe.a.waitForIdle(300,10000);
  n=GpuApiProbe.node("home-name");Rect box=new Rect();n.getBoundsInScreen(box);n.recycle();return box;
 }
 static boolean keyboard()throws Exception {return GpuSearchProbe.shell("/system/bin/dumpsys","input_method").contains("mInputShown=true");}
 static void waitKeyboard(boolean shown)throws Exception {
  for(int i=0;i<30;i++){if(keyboard()==shown){GpuApiProbe.a.waitForIdle(300,10000);return;}Thread.sleep(100);}
  throw new AssertionError("keyboard shown="+shown);
 }
 static void measure(boolean dark)throws Exception {
  GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");setText("MMMM");GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");waitKeyboard(false);Rect box=input();float scale=density();System.out.println("measure dark="+dark+" box="+box+" density="+scale);
  GpuApiProbe.check(Math.abs(box.height()-44*scale)<=2,"44 logical pixel input height");
  Bitmap shot=GpuApiProbe.a.takeScreenshot(),bitmap=shot.copy(Bitmap.Config.ARGB_8888,false);shot.recycle();
  int ink=dark?0xfff2f2f7:0xff2c2c2e,first=box.right,top=box.bottom,bottom=box.top;
  try {
   for(int y=box.top+Math.round(4*scale);y<box.bottom-Math.round(4*scale);y++)for(int x=box.left+Math.round(4*scale);x<box.centerX();x++){
    int pixel=bitmap.getPixel(x,y);
    if(Math.abs(Color.red(pixel)-Color.red(ink))<=3&&Math.abs(Color.green(pixel)-Color.green(ink))<=3&&Math.abs(Color.blue(pixel)-Color.blue(ink))<=3){first=Math.min(first,x);top=Math.min(top,y);bottom=Math.max(bottom,y);}
   }
  }finally{bitmap.recycle();}
  float inset=(first-box.left)/scale;GpuApiProbe.check(inset>=15&&inset<=19,"16 logical pixel text inset, got "+inset);
  GpuApiProbe.check(Math.abs((top+bottom)/2f-box.exactCenterY())/scale<=4,"vertically centered input ink");
  GpuRadioProbe.screenshot(dark?"gpu-padding-dark":"gpu-padding-light");
  GpuSearchProbe.shell("/system/bin/input","tap",String.valueOf(box.left+Math.round(16*scale)),String.valueOf(box.centerY()));GpuApiProbe.a.waitForIdle(300,10000);
  waitKeyboard(true);
  GpuSearchProbe.shell("/system/bin/input","text","Z");GpuUiProbe.waitText("home-name","ZMMMM");GpuRadioProbe.key(4);setText("");
 }
 public static void main(String[] args)throws Exception {
  HandlerThread thread=new HandlerThread("padding-probe");thread.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  String auto=GpuGridProbe.settings("get","system","accelerometer_rotation"),rotation=GpuGridProbe.settings("get","system","user_rotation");
  try {
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(800);
   GpuGridProbe.settings("put","system","accelerometer_rotation","0");GpuGridProbe.settings("put","system","user_rotation","0");Thread.sleep(700);measure(false);measure(true);
   if(args.length==0 || !args[0].equals("--portrait-only")){GpuGridProbe.settings("put","system","user_rotation","1");Thread.sleep(1100);measure(true);}
   GpuGridProbe.settings("put","system","user_rotation","0");Thread.sleep(900);GpuApiProbe.click("home-appearance-light");
   System.out.println("PASS Tailwind input dimensions, centered text, physical caret targeting and text input in light/dark"+(args.length>0?" (portrait only)":" and rotation"));
  }finally{GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
