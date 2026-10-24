import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.accessibility.AccessibilityNodeInfo;

public final class GpuChartProbe {
 static void tap(float x,float y)throws Exception{
  long time=android.os.SystemClock.uptimeMillis();
  for(int action:new int[]{android.view.MotionEvent.ACTION_DOWN,android.view.MotionEvent.ACTION_UP}){
   android.view.MotionEvent event=android.view.MotionEvent.obtain(time,time+(action==1?60:0),action,x,y,0);
   event.setSource(android.view.InputDevice.SOURCE_TOUCHSCREEN);GpuApiProbe.a.injectInputEvent(event,true);event.recycle();
  }
  Thread.sleep(500);
 }
 static void checkSelection(String kind,String suffix)throws Exception{
  AccessibilityNodeInfo plot=GpuApiProbe.node("chart-plot-"+kind);
  plot.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());plot.recycle();Thread.sleep(300);
  plot=GpuApiProbe.node("chart-plot-"+kind);
  String description=String.valueOf(plot.getContentDescription()),prefix=kind+" chart. ";
  String[] samples=description.substring(description.indexOf(prefix)+prefix.length()).split(", ");
  android.graphics.Rect box=new android.graphics.Rect();plot.getBoundsInScreen(box);plot.recycle();
  float x,y;
  if(kind.equals("pie")){
   tap(box.exactCenterX(),box.exactCenterY());
   GpuUiProbe.waitText("chart-selection","Tap a legend item for details.");
   plot=GpuApiProbe.node("chart-plot-"+kind);
   plot.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());plot.recycle();Thread.sleep(250);
   plot=GpuApiProbe.node("chart-plot-"+kind);plot.getBoundsInScreen(box);plot.recycle();
   double total=0;for(String sample:samples)total+=Double.parseDouble(sample.substring(sample.lastIndexOf(": ")+2));
   double first=Double.parseDouble(samples[0].substring(samples[0].lastIndexOf(": ")+2));
   double angle=-Math.PI/2+Math.PI*first/total,radius=Math.min(box.width(),box.height())*.46*.79;
   x=(float)(box.exactCenterX()+Math.cos(angle)*radius);y=(float)(box.exactCenterY()+Math.sin(angle)*radius);
  }else{
   float padding=4*box.height()/180f;
   x=box.left+padding+(box.width()-2*padding)/samples.length/2;y=box.exactCenterY();
  }
  tap(x,y);
  GpuUiProbe.waitText("chart-selection",samples[0]);
  GpuUiProbe.check(GpuApiProbe.text("chart-tooltip").startsWith(samples[0]),"direct plot tooltip "+kind);
  GpuRadioProbe.screenshot("gpu-chart-selected-"+kind+suffix);
  plot=GpuApiProbe.node("chart-plot-"+kind);
  plot.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());plot.recycle();Thread.sleep(250);
  plot=GpuApiProbe.node("chart-plot-"+kind);
  GpuUiProbe.check(plot.performAction(0x02010000+samples.length-1),"accessible chart item");plot.recycle();
  GpuUiProbe.waitText("chart-selection",samples[samples.length-1]);
  plot=GpuApiProbe.node("chart-plot-"+kind);
  plot.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());plot.recycle();Thread.sleep(250);
  plot=GpuApiProbe.node("chart-plot-"+kind);plot.getBoundsInScreen(box);plot.recycle();
  GpuSearchProbe.shell("/system/bin/input","swipe",String.valueOf(box.centerX()),String.valueOf(box.centerY()),
    String.valueOf(box.centerX()+30),String.valueOf(box.top+20),"350");
  Thread.sleep(400);
  GpuUiProbe.waitText("chart-selection",samples[samples.length-1]);
  GpuApiProbe.click("chart-series-"+(samples.length-1));
  GpuUiProbe.waitText("chart-selection","Tap a legend item for details.");
 }
 static void checkLineAxes()throws Exception{
  AccessibilityNodeInfo summary=GpuApiProbe.node("chart-plot-line");
  String description=String.valueOf(summary.getContentDescription());summary.recycle();
  String[] samples=description.substring(description.indexOf("line chart. ")+"line chart. ".length()).split(", ");
  double high=0,low=0;
  for(String sample:samples){double value=Double.parseDouble(sample.substring(sample.lastIndexOf(": ")+2));high=Math.max(high,value);low=Math.min(low,value);}
  AccessibilityNodeInfo last=GpuApiProbe.node("chart-label-"+(samples.length-1));
  last.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());last.recycle();Thread.sleep(250);
  AccessibilityNodeInfo plot=GpuApiProbe.node("chart-plot-line");android.graphics.Rect bounds=new android.graphics.Rect();plot.getBoundsInScreen(bounds);plot.recycle();
  float scale=bounds.height()/180f,padding=4*scale,step=(bounds.width()-2*padding)/samples.length;
  for(int index=0;index<samples.length;index++){
   AccessibilityNodeInfo label=GpuApiProbe.node("chart-label-"+index);android.graphics.Rect box=new android.graphics.Rect();label.getBoundsInScreen(box);label.recycle();
   GpuUiProbe.check(Math.abs(box.exactCenterX()-(bounds.left+padding+step*(index+.5f)))<3*scale,"category aligned with data point "+index);
  }
  for(int index=0;index<5;index++){
   double expected=high-(high-low)*index/4;
   double actual=Double.parseDouble(GpuApiProbe.text("chart-tick-"+index));
   GpuUiProbe.check(Math.abs(actual-expected)<=Math.max(.001,Math.abs(expected)*.005),"axis tick "+index);
  }
 }
 public static void main(String[] args)throws Exception{
  boolean dark=args.length>0&&args[0].equals("dark");
  String suffix=dark?"-dark":"";
  HandlerThread thread=new HandlerThread("chart-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{
   UiAutomation.class.getMethod("connect").invoke(a);
   android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();
   info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(1100);
   GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");
   GpuApiProbe.click("open-api");GpuApiProbe.click("api-stream");
   for(String kind:new String[]{"bar","line","pie"}){
    GpuApiProbe.click("stream-sample-"+kind);GpuStreamProbe.set("Show a "+kind+" chart");GpuStreamProbe.clickNow("stream-send");
    GpuStreamProbe.waitText("stream-status","Completed");
    AccessibilityNodeInfo plot=GpuApiProbe.node("chart-plot-"+kind);
    GpuUiProbe.check(String.valueOf(plot.getContentDescription()).contains(kind+" chart"),"chart accessible summary");
    plot.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());plot.recycle();Thread.sleep(300);
    plot=GpuApiProbe.node("chart-plot-"+kind);android.graphics.Rect box=new android.graphics.Rect();plot.getBoundsInScreen(box);plot.recycle();
    GpuUiProbe.check(box.width()>300&&box.height()>200,"visible chart bounds");
    android.graphics.Bitmap capture=a.takeScreenshot(),pixels=capture.copy(android.graphics.Bitmap.Config.ARGB_8888,false);capture.recycle();
    int colored=0,pastel=0;
    for(int y=Math.max(0,box.top);y<Math.min(pixels.getHeight(),box.bottom);y+=2)
     for(int x=Math.max(0,box.left);x<Math.min(pixels.getWidth(),box.right);x+=2){
      int p=pixels.getPixel(x,y),r=android.graphics.Color.red(p),g=android.graphics.Color.green(p),b=android.graphics.Color.blue(p);
      if(Math.max(r,Math.max(g,b))-Math.min(r,Math.min(g,b))>60)colored++;
      if(dark ? r<70&&b-r>=45&&b-r<=85&&b>g&&g>r : r>150&&b-r>=8&&b-r<=58&&b>g&&g>r)pastel++;
     }
    pixels.recycle();GpuUiProbe.check(colored>150,"colored plot pixels: "+kind);
    if(kind.equals("line"))GpuUiProbe.check(pastel>box.width()*box.height()/200,"visible gradient area: "+pastel);
    if(kind.equals("line"))checkLineAxes();
    GpuRadioProbe.screenshot("gpu-independent-chart-"+kind+suffix);
    checkSelection(kind,suffix);
    GpuApiProbe.click("chart-series-0");GpuUiProbe.check(!GpuApiProbe.text("chart-selection").startsWith("Tap"),"legend selection");
    if(kind.equals("line")){
     String selected=GpuApiProbe.text("chart-selection");
     String auto=GpuSearchProbe.shell("/system/bin/settings","get","system","accelerometer_rotation").trim();
     String rotation=GpuSearchProbe.shell("/system/bin/settings","get","system","user_rotation").trim();
     try{
      GpuGridProbe.settings("put","system","accelerometer_rotation","0");GpuGridProbe.settings("put","system","user_rotation","1");Thread.sleep(1100);
      GpuUiProbe.waitText("chart-selection",selected);
      AccessibilityNodeInfo landscape=GpuApiProbe.node("chart-plot-line");
      landscape.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());landscape.recycle();Thread.sleep(400);
      checkLineAxes();
      GpuRadioProbe.screenshot("gpu-independent-chart-line-landscape"+suffix);
     }finally{GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);Thread.sleep(900);}
     GpuUiProbe.waitText("chart-selection",selected);
    }
    GpuApiProbe.click("chart-series-0");GpuUiProbe.waitText("chart-selection","Tap a legend item for details.");
    GpuApiProbe.click("stream-clear");GpuStreamProbe.waitText("stream-count","0 messages");
   }
   GpuChromeProbe.back();GpuChromeProbe.back();
   if(dark)GpuApiProbe.click("home-appearance-light");
   System.out.println("PASS "+(dark?"dark":"light")+" streamed bar/smooth area/donut artifacts, native colored pixels, aligned axes, accessible summaries and legend selection");
  }catch(Throwable error){error.printStackTrace();throw error;}
  finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
