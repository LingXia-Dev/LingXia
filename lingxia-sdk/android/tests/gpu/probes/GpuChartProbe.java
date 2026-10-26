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
 static android.graphics.Rect showPlot(String kind)throws Exception{
  AccessibilityNodeInfo plot=GpuApiProbe.node("chart-plot-"+kind);
  plot.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());plot.recycle();Thread.sleep(300);
  plot=GpuApiProbe.node("chart-plot-"+kind);android.graphics.Rect box=new android.graphics.Rect();plot.getBoundsInScreen(box);plot.recycle();
  return box;
 }
 static String[] samples(String kind)throws Exception{
  AccessibilityNodeInfo plot=GpuApiProbe.node("chart-plot-"+kind);
  String description=String.valueOf(plot.getContentDescription()),prefix=kind+" chart. ";plot.recycle();
  return description.substring(description.indexOf(prefix)+prefix.length()).split(", ");
 }
 static String description(String kind)throws Exception{
  AccessibilityNodeInfo plot=GpuApiProbe.node("chart-plot-"+kind);String d=String.valueOf(plot.getContentDescription());plot.recycle();return d;
 }
 /** Total chroma of the plot: a selection dims the other bars and slices to 60%. */
 static long vivid(android.graphics.Rect box){
  android.graphics.Bitmap capture=GpuApiProbe.a.takeScreenshot(),pixels=capture.copy(android.graphics.Bitmap.Config.ARGB_8888,false);capture.recycle();
  long count=0;
  try{
   for(int y=Math.max(0,box.top);y<Math.min(pixels.getHeight(),box.bottom);y+=2)
    for(int x=Math.max(0,box.left);x<Math.min(pixels.getWidth(),box.right);x+=2){
     int p=pixels.getPixel(x,y),r=android.graphics.Color.red(p),g=android.graphics.Color.green(p),b=android.graphics.Color.blue(p);
     count+=Math.max(r,Math.max(g,b))-Math.min(r,Math.min(g,b));
    }
  }finally{pixels.recycle();}
  return count;
 }
 static String label(String sample){return sample.substring(0,sample.lastIndexOf(": "));}
 static void waitTooltip(String label)throws Exception{
  String value="";
  for(int i=0;i<35;i++){if(GpuStreamProbe.present("chart-tooltip")){value=GpuApiProbe.text("chart-tooltip");if(value.startsWith(label+": "))return;}Thread.sleep(150);}
  throw new AssertionError("chart-tooltip expected "+label+" got "+value);
 }
 static void checkSelection(String kind,String suffix)throws Exception{
  String[] samples=samples(kind);
  android.graphics.Rect box=showPlot(kind);
  GpuUiProbe.check(!GpuStreamProbe.present("chart-tooltip"),"no tooltip before a selection");
  long before=vivid(box);
  float x,y;
  if(kind.equals("pie")){
   double total=0;for(String sample:samples)total+=Double.parseDouble(sample.substring(sample.lastIndexOf(": ")+2));
   double first=Double.parseDouble(samples[0].substring(samples[0].lastIndexOf(": ")+2));
   // pieLayout: center [0.5, 0.44], radii 0.4..0.68 of the shorter side's half.
   double angle=-Math.PI/2+Math.PI*first/total,radius=Math.min(box.width(),box.height())*.5*.54;
   x=(float)(box.left+box.width()*.5+Math.cos(angle)*radius);y=(float)(box.top+box.height()*.44+Math.sin(angle)*radius);
  }else{
   float padding=4*box.height()/180f;
   x=box.left+padding+(box.width()-2*padding)/samples.length/2;y=box.exactCenterY();
  }
  tap(x,y);
  waitTooltip(label(samples[0]));
  box=showPlot(kind);Thread.sleep(300);
  long after=vivid(box);
  System.out.println("chart "+kind+" vivid before="+before+" selected="+after);
  // Lines keep their colour; bars and slices dim the series that are not chosen.
  if(!kind.equals("line"))GpuUiProbe.check(samples.length<2||after<before*0.95,"other series dimmed: "+kind);
  GpuRadioProbe.screenshot("gpu-chart-selected-"+kind+suffix);
  AccessibilityNodeInfo plot=GpuApiProbe.node("chart-plot-"+kind);
  GpuUiProbe.check(plot.performAction(0x02010000+samples.length-1),"accessible chart item");plot.recycle();
  waitTooltip(label(samples[samples.length-1]));
 }
 /** A pie legend entry hides its slice and clears the selection; a second press shows it. */
 static void checkLegend(String suffix)throws Exception{
  String first=samples("pie")[0];
  GpuApiProbe.click("chart-series-0");
  GpuUiProbe.check(description("pie").contains(first+" (hidden)"),"legend hides the first slice");
  GpuUiProbe.check(!GpuStreamProbe.present("chart-tooltip"),"hiding a slice clears the selection");
  GpuRadioProbe.screenshot("gpu-chart-pie-hidden"+suffix);
  GpuApiProbe.click("chart-series-0");
  GpuUiProbe.check(!description("pie").contains("(hidden)"),"legend shows the slice again");
 }
 static void checkLineAxes()throws Exception{
  AccessibilityNodeInfo summary=GpuApiProbe.node("chart-plot-line");
  String description=String.valueOf(summary.getContentDescription());summary.recycle();
  String[] samples=description.substring(description.indexOf("line chart. ")+"line chart. ".length()).split(", ");
  double high=0,low=0;
  for(String sample:samples){double value=Double.parseDouble(sample.substring(sample.lastIndexOf(": ")+2));high=Math.max(high,value);low=Math.min(low,value);}
  // A short landscape page cannot hold the chart's title, plot and labels at once: scroll the labels in.
  String lastLabel="chart-label-"+(samples.length-1);
  for(int step=0;step<8&&!GpuStreamProbe.present(lastLabel);step++){
   AccessibilityNodeInfo plot=GpuApiProbe.node("chart-plot-line");android.graphics.Rect at=new android.graphics.Rect();plot.getBoundsInScreen(at);plot.recycle();
   GpuSearchProbe.shell("/system/bin/input","swipe",String.valueOf(at.centerX()),String.valueOf(at.centerY()),String.valueOf(at.centerX()),String.valueOf(at.centerY()-300),"400");Thread.sleep(600);
  }
  AccessibilityNodeInfo last=GpuApiProbe.node(lastLabel);
  last.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());last.recycle();Thread.sleep(250);
  AccessibilityNodeInfo plot=GpuApiProbe.node("chart-plot-line");android.graphics.Rect bounds=new android.graphics.Rect();plot.getBoundsInScreen(bounds);plot.recycle();
  float scale=bounds.height()/180f,padding=4*scale,step=(bounds.width()-2*padding)/samples.length;
  for(int index=0;index<samples.length;index++){
   AccessibilityNodeInfo label=GpuApiProbe.node("chart-label-"+index);android.graphics.Rect box=new android.graphics.Rect();label.getBoundsInScreen(box);label.recycle();
   GpuUiProbe.check(Math.abs(box.exactCenterX()-(bounds.left+padding+step*(index+.5f)))<3*scale,"category aligned with data point "+index);
  }
  // Ticks are rounded steps from the axis top to its bottom; together they cover every value and zero.
  java.util.ArrayList<Double> ticks=new java.util.ArrayList<>();
  for(int index=0;GpuStreamProbe.present("chart-tick-"+index);index++)ticks.add(Double.parseDouble(GpuApiProbe.text("chart-tick-"+index)));
  GpuUiProbe.check(ticks.size()>=3,"axis ticks: "+ticks);
  GpuUiProbe.check(ticks.get(0)>=high-1e-9&&ticks.get(ticks.size()-1)<=Math.min(0,low)+1e-9,"axis covers the values: "+ticks);
  for(int index=1;index<ticks.size();index++)GpuUiProbe.check(ticks.get(index)<ticks.get(index-1),"axis ticks descend: "+ticks);
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
   GpuChromeProbe.tab("API");GpuApiProbe.click("api-stream");
   for(String kind:new String[]{"bar","line","pie"}){
    // The stream answers with a random scenario; ask again until it draws this kind.
    boolean drawn=false;
    for(int attempt=0;attempt<15&&!drawn;attempt++){
     GpuStreamProbe.set("Show me some data");GpuStreamProbe.clickNow("stream-send");
     GpuStreamProbe.waitSettled(2);
     drawn=GpuTodoParityProbe.exists("chart-plot-"+kind);
     if(!drawn){GpuApiProbe.click("stream-clear");GpuStreamProbe.waitEmpty();}
    }
    GpuUiProbe.check(drawn,"stream drew a "+kind+" chart");
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
      // The area tint follows the accent role, whatever its hue.
      int lo=Math.min(r,Math.min(g,b)),spread=Math.max(r,Math.max(g,b))-lo;
      if(dark ? lo<90&&spread>=15&&spread<=85 : lo>150&&spread>=8&&spread<=58)pastel++;
     }
    pixels.recycle();GpuUiProbe.check(colored>150,"colored plot pixels: "+kind);
    if(kind.equals("line"))GpuUiProbe.check(pastel>box.width()*box.height()/200,"visible gradient area: "+pastel);
    if(kind.equals("line"))checkLineAxes();
    GpuRadioProbe.screenshot("gpu-independent-chart-"+kind+suffix);
    checkSelection(kind,suffix);
    if(kind.equals("pie"))checkLegend(suffix);
    if(kind.equals("line")){
     String selected=GpuApiProbe.text("chart-tooltip");
     String auto=GpuSearchProbe.shell("/system/bin/settings","get","system","accelerometer_rotation").trim();
     String rotation=GpuSearchProbe.shell("/system/bin/settings","get","system","user_rotation").trim();
     try{
      GpuGridProbe.settings("put","system","accelerometer_rotation","0");GpuGridProbe.settings("put","system","user_rotation","1");Thread.sleep(1100);
      GpuUiProbe.waitText("chart-tooltip",selected);
      AccessibilityNodeInfo landscape=GpuApiProbe.node("chart-plot-line");
      landscape.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());landscape.recycle();Thread.sleep(400);
      checkLineAxes();
      GpuRadioProbe.screenshot("gpu-independent-chart-line-landscape"+suffix);
     }finally{GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);Thread.sleep(900);}
     GpuUiProbe.waitText("chart-tooltip",selected);
    }
    GpuApiProbe.click("stream-clear");GpuStreamProbe.waitEmpty();
   }
   GpuChromeProbe.back();
   if(dark){GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");}
   System.out.println("PASS "+(dark?"dark":"light")+" streamed bar/smooth area/donut artifacts, coloured pixels, aligned axes, accessible summaries, tooltip selection with dimming and pie legend toggles");
  }catch(Throwable error){error.printStackTrace();throw error;}
  finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
