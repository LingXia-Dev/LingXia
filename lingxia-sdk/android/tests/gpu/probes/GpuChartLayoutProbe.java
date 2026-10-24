import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;
public final class GpuChartLayoutProbe {
 static void legend(boolean wrapped)throws Exception{
  android.graphics.Rect container=GpuThemeProbe.bounds("page-scroll"),plot=GpuThemeProbe.bounds("chart-plot-pie");java.util.ArrayList<android.graphics.Rect> items=new java.util.ArrayList<>();java.util.HashSet<Integer> rows=new java.util.HashSet<>();
  for(int n=0;n<4;n++){android.graphics.Rect box=GpuThemeProbe.bounds("chart-series-"+n);GpuUiProbe.check(container.contains(box)&&box.left>=plot.left&&box.right<=plot.right&&box.top>=plot.top+plot.height()*.78f,"legend item stays below ring and within content");for(android.graphics.Rect other:items)GpuUiProbe.check(!android.graphics.Rect.intersects(box,other),"legend items do not overlap");items.add(box);rows.add(box.top);}
  GpuUiProbe.check(wrapped?rows.size()>1:rows.size()==1,"legend wraps according to available width: "+rows.size());
 }
 static int colorDistance(int a,int b){return Math.max(Math.abs(android.graphics.Color.red(a)-android.graphics.Color.red(b)),Math.max(Math.abs(android.graphics.Color.green(a)-android.graphics.Color.green(b)),Math.abs(android.graphics.Color.blue(a)-android.graphics.Color.blue(b))));}
 static void noTooltip()throws Exception{
  AccessibilityNodeInfo tooltip=GpuRadioProbe.find(GpuApiProbe.a.getRootInActiveWindow(),"chart-tooltip");
  boolean absent=tooltip==null;if(tooltip!=null)tooltip.recycle();GpuUiProbe.check(absent,"cleared plot removes tooltip");
 }
 static android.graphics.Bitmap pixels(){android.graphics.Bitmap raw=GpuApiProbe.a.takeScreenshot(),copy=raw.copy(android.graphics.Bitmap.Config.ARGB_8888,false);raw.recycle();return copy;}
 static void markerGrowth(android.graphics.Rect bounds,android.graphics.Bitmap before)throws Exception{
  android.graphics.Bitmap after=pixels();
  try{
   float scale=bounds.height()/128f,step=(bounds.width()-8*scale)/4;
   float cx=bounds.left+4*scale+step*2.5f,cy=bounds.top+4*scale+(bounds.height()-8*scale)*29/60f;
   int strongest=0;
   // Remove the known category tint from the comparison. The lower circle edge
   // is clear of the rising line, so new blue coverage proves marker expansion.
   for(int y=(int)Math.floor(cy+3.8f*scale);y<=Math.ceil(cy+4.8f*scale);y++)for(int x=Math.round(cx)-1;x<=Math.round(cx)+1;x++){
    int old=before.getPixel(x,y),now=after.getPixel(x,y);
    int expectedRed=Math.round(android.graphics.Color.red(old)*.8f+129*.2f);
    int expectedBlue=Math.round(android.graphics.Color.blue(old)*.8f+136*.2f);
    strongest=Math.max(strongest,android.graphics.Color.blue(now)-android.graphics.Color.red(now)-(expectedBlue-expectedRed));
   }
   GpuUiProbe.check(strongest>=15,"selected line marker expands beyond unchanged circle: "+strongest);
  }finally{after.recycle();}
 }
 static void dashedGrid(android.graphics.Rect bounds)throws Exception{
  android.graphics.Bitmap raw=GpuApiProbe.a.takeScreenshot(),pixels=raw.copy(android.graphics.Bitmap.Config.ARGB_8888,false);raw.recycle();
  try{
   float scale=bounds.height()/128f,start=bounds.left+4*scale;int y=Math.round(bounds.top+4.5f*scale),backgroundY=Math.round(y+4*scale);
   int cycles=(int)((bounds.width()/scale-8)/6);
   for(int cycle:new int[]{0,4,cycles-2}){
    int onX=Math.round(start+(cycle*6+2)*scale),offX=Math.round(start+(cycle*6+5)*scale);
    int background=pixels.getPixel(offX,backgroundY);
    GpuUiProbe.check(colorDistance(pixels.getPixel(onX,y),background)>4,"grid dash must paint");
    GpuUiProbe.check(colorDistance(pixels.getPixel(offX,y),background)<=3,"grid gap must stay transparent");
   }
  }finally{pixels.recycle();}
 }
 static void bubbleCorners(android.graphics.Rect plot)throws Exception{
  float scale=plot.height()/128f;
  android.graphics.Rect user=GpuThemeProbe.bounds("stream-message-fixture-user"),assistant=GpuThemeProbe.bounds("stream-message-fixture-assistant"),viewport=GpuThemeProbe.bounds("page-scroll");
  android.graphics.Bitmap raw=GpuApiProbe.a.takeScreenshot(),pixels=raw.copy(android.graphics.Bitmap.Config.ARGB_8888,false);raw.recycle();
  try{
   int blue=0xff155dfc,last=-1;
   // Read the authored blue from a padding pixel, avoiding theme assumptions.
   blue=pixels.getPixel(user.left-Math.round(5*scale),user.centerY());
   for(int y=user.top;y<assistant.top;y++)if(colorDistance(pixels.getPixel(user.centerX(),y),blue)<=2)last=y;
   GpuUiProbe.check(last>=user.bottom,"user bubble has bottom padding");
   int y=last-Math.round(3*scale);
   GpuUiProbe.check(colorDistance(pixels.getPixel(user.right+Math.round(13*scale),y),blue)<=3,"user bottom-right small corner paints");
   GpuUiProbe.check(colorDistance(pixels.getPixel(user.left-Math.round(13*scale),y),blue)>8,"user bottom-left keeps large corner");
   int surface=pixels.getPixel(assistant.left-Math.round(5*scale),assistant.top+Math.round(10*scale));
   int run=0;last=-1;
   // The bottom padding forms a continuous surface run; text antialiasing and
   // dark shadows can match its color briefly, so neither defines the edge.
   for(int row=plot.bottom;row<viewport.bottom;row++){
    if(colorDistance(pixels.getPixel(assistant.centerX(),row),surface)<=2){run++;if(run>=8*scale)last=row;}
    else run=0;
   }
   GpuUiProbe.check(last>plot.bottom,"assistant bottom padding found");
   y=last-Math.round(4*scale);
   GpuUiProbe.check(colorDistance(pixels.getPixel(assistant.left-Math.round(12*scale),y),surface)<=3,"assistant bottom-left small corner paints");
   GpuUiProbe.check(colorDistance(pixels.getPixel(assistant.right+Math.round(12*scale),y),surface)>4,"assistant bottom-right keeps large corner");
  }finally{pixels.recycle();}
 }
 static void axisSelection(android.graphics.Rect bounds,boolean bar,int index)throws Exception{
  android.graphics.Bitmap raw=GpuApiProbe.a.takeScreenshot(),pixels=raw.copy(android.graphics.Bitmap.Config.ARGB_8888,false);raw.recycle();
  try{
   float scale=bounds.height()/128f,step=(bounds.width()-8*scale)/4;
   // Sample below the tooltip shadow and above the first three fixture values.
   int y=Math.round(bounds.top+30*scale),selectedX=Math.round(bounds.left+4*scale+step*(index+.5f)),otherX=Math.round(bounds.left+4*scale+step*1.5f);
   int background=pixels.getPixel(otherX,y),selected=pixels.getPixel(selectedX,y);
   int expected=android.graphics.Color.rgb(Math.round(android.graphics.Color.red(background)*.8f+129*.2f),Math.round(android.graphics.Color.green(background)*.8f+130*.2f),Math.round(android.graphics.Color.blue(background)*.8f+136*.2f));
   GpuUiProbe.check(colorDistance(selected,expected)<=3,"selected category uses reference shadow tint");
   if(bar){
    GpuThemeProbe.pixel(pixels,Math.round(bounds.left+4*scale+step*3.5f),Math.round(bounds.bottom-8*scale),0xff3b82f6,"other bars retain original color after selection");
    GpuThemeProbe.pixel(pixels,selectedX,Math.round(bounds.bottom-8*scale),0xff4d8ce7,"selected bar emphasis composes under category pointer");
   }
  }finally{pixels.recycle();}
 }
 static void pieGeometry(android.graphics.Rect bounds)throws Exception{
  float unit=Math.min(bounds.width(),bounds.height())/2f,cx=bounds.exactCenterX(),cy=bounds.top+bounds.height()*.44f;
  android.graphics.Bitmap raw=GpuApiProbe.a.takeScreenshot(),pixels=raw.copy(android.graphics.Bitmap.Config.ARGB_8888,false);raw.recycle();
  try{
   int y=Math.round(cy),background=pixels.getPixel(Math.round(cx),y);
   for(float radius:new float[]{.38f,.72f})GpuUiProbe.check(colorDistance(pixels.getPixel(Math.round(cx+unit*radius),y),background)<=3,"pie hole and outside retain background");
   for(float radius:new float[]{.42f,.66f})GpuThemeProbe.pixel(pixels,Math.round(cx+unit*radius),y,0xff8b5cf6,"pie radii match React percentages");
  }finally{pixels.recycle();}
  GpuApiProbe.click("chart-plot-pie");GpuUiProbe.waitText("chart-tooltip","Q1: 16.55%");
  for(float radius:new float[]{.8f,.3f,.54f}){
   java.lang.Process tap=Runtime.getRuntime().exec(new String[]{"/system/bin/input","tap",String.valueOf(Math.round(cx+unit*radius)),String.valueOf(Math.round(cy))});
   GpuUiProbe.check(tap.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&tap.exitValue()==0,"pie touch injection");Thread.sleep(400);
   if(radius==.54f)GpuUiProbe.waitText("chart-tooltip","Q2: 26.21%");
   else{noTooltip();GpuApiProbe.click("chart-plot-pie");GpuUiProbe.waitText("chart-tooltip","Q1: 16.55%");}
  }
  raw=GpuApiProbe.a.takeScreenshot();pixels=raw.copy(android.graphics.Bitmap.Config.ARGB_8888,false);raw.recycle();
  try{
   float scale=bounds.height()/210f;int y=Math.round(cy),background=pixels.getPixel(Math.round(cx),y);
   GpuThemeProbe.pixel(pixels,Math.round(cx+unit*.68f+2*scale),y,0xff9865ff,"selected pie grows five CSS pixels and uses emphasis color");
   for(float x:new float[]{cx+unit*.68f+6*scale,cx-unit*.68f-2*scale,cx+unit*.38f})
    GpuUiProbe.check(colorDistance(pixels.getPixel(Math.round(x),y),background)<=3,"pie emphasis preserves hole, unselected edge and outer limit");
  }finally{pixels.recycle();}
 }
 static void signedAxis(boolean negative,boolean dark)throws Exception{
  String fixture=negative?"bar-negative":"bar-signed";
  try(java.io.InputStream in=new java.net.URL("http://127.0.0.1:39590/__probe/chart/"+fixture).openStream()){while(in.read()!=-1){}}Thread.sleep(700);
  android.graphics.Rect bounds=GpuThemeProbe.bounds("chart-plot-bar");
  android.graphics.Bitmap raw=GpuApiProbe.a.takeScreenshot(),pixels=raw.copy(android.graphics.Bitmap.Config.ARGB_8888,false);raw.recycle();
  try{
   float scale=bounds.height()/128f,start=bounds.left+4*scale;
   // Near the left edge, both samples remain outside the first bar.
   int onX=Math.round(start+2*scale),offX=Math.round(start+5*scale);
   int zeroY=Math.round(bounds.top+(negative?4.5f:76.5f)*scale),bottomY=Math.round(bounds.top+124.5f*scale);
   int background=pixels.getPixel(offX,bottomY-Math.round(4*scale));
   GpuUiProbe.check(colorDistance(pixels.getPixel(onX,zeroY),background)>4,"zero axis paints at zero value");
   GpuUiProbe.check(colorDistance(pixels.getPixel(offX,zeroY),background)>4,"zero axis stays solid across dash gaps");
   GpuUiProbe.check(colorDistance(pixels.getPixel(onX,bottomY),background)>4,"negative bottom grid dash paints");
   GpuUiProbe.check(colorDistance(pixels.getPixel(offX,bottomY),background)<=3,"negative bottom grid remains dashed");
  }finally{pixels.recycle();}
  GpuRadioProbe.screenshot("gpu-chart-"+fixture+(dark?"-dark":"-light"));
  GpuApiProbe.click("stream-clear");
 }
 public static void main(String[]args)throws Exception{
  HandlerThread t=new HandlerThread("chart-layout");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);Thread.sleep(900);
   for(boolean dark:new boolean[]{false,true}){
    GpuChromeProbe.tab("Home");GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");GpuApiProbe.click("api-stream");
    for(String kind:new String[]{"bar","line","pie"}){
     try(java.io.InputStream in=new java.net.URL("http://127.0.0.1:39590/__probe/chart/"+kind).openStream()){while(in.read()!=-1){}}Thread.sleep(700);
     GpuUiProbe.waitText("chart-title","QUARTERLY SALES");AccessibilityNodeInfo plot=GpuApiProbe.node("chart-plot-"+kind);GpuUiProbe.check(plot.getContentDescription().toString().contains("Q4: 52"),"accessible exact series");android.graphics.Rect bounds=new android.graphics.Rect();plot.getBoundsInScreen(bounds);GpuUiProbe.check(bounds.width()>100&&bounds.height()>100,"visible plot bounds");plot.recycle();
     android.graphics.Rect messages=GpuThemeProbe.bounds("page-scroll"),clear=GpuThemeProbe.bounds("stream-clear");GpuUiProbe.check(clear.top>=messages.top&&clear.bottom<=messages.bottom,"clear overlays message viewport instead of reserving a row");GpuUiProbe.check(clear.bottom<=GpuThemeProbe.bounds("stream-message-fixture-user").top,"clear must not cover first message");
     if(!kind.equals("pie")){
      dashedGrid(bounds);
      for(int tick=0;tick<7;tick++)GpuUiProbe.waitText("chart-tick-"+tick,String.valueOf(60-tick*10));
      if(kind.equals("bar")){
       bubbleCorners(bounds);
       android.graphics.Bitmap raw=a.takeScreenshot(),pixels=raw.copy(android.graphics.Bitmap.Config.ARGB_8888,false);raw.recycle();
       try{
        float padding=bounds.height()/32f;
        int x=Math.round(bounds.left+padding+(bounds.width()-2*padding)*0.875f),first=-1;
        for(int y=bounds.top;y<bounds.bottom;y++){int color=pixels.getPixel(x,y);if(Math.abs(android.graphics.Color.red(color)-59)<4&&Math.abs(android.graphics.Color.green(color)-130)<4&&Math.abs(android.graphics.Color.blue(color)-246)<4){first=y;break;}}
        float expected=bounds.top+padding+(bounds.height()-2*padding)*8/60f;
        GpuUiProbe.check(first>=0&&Math.abs(first-expected)<4,"bar height agrees with 0..60 scale: "+first+" expected "+expected);
        float scale=bounds.height()/128f,step=(bounds.width()-2*padding)/4,barWidth=Math.min(step*0.65f,36*scale);
        float left=bounds.left+padding+step*0.5f-barWidth/2,top=bounds.top+padding+(bounds.height()-2*padding)*36/60f;
        int corner=pixels.getPixel(Math.round(left+scale),Math.round(top+scale));
        int background=pixels.getPixel(Math.round(left-3*scale),Math.round(top+scale));
        GpuUiProbe.check(Math.abs(android.graphics.Color.blue(corner)-android.graphics.Color.blue(background))<20&&Math.abs(android.graphics.Color.red(corner)-android.graphics.Color.red(background))<20,"bar top corner follows four-pixel radius");
        int bottom=(int)Math.floor(bounds.bottom-padding)-2;
        GpuThemeProbe.pixel(pixels,(int)Math.ceil(left)+1,bottom,0xff3b82f6,"bar bottom-left corner stays square");
        GpuThemeProbe.pixel(pixels,(int)Math.floor(left+barWidth)-2,bottom,0xff3b82f6,"bar bottom-right corner stays square");
       }finally{pixels.recycle();}
      }
     }
     if(kind.equals("line")){
      android.graphics.Bitmap raw=a.takeScreenshot(),pixels=raw.copy(android.graphics.Bitmap.Config.ARGB_8888,false);raw.recycle();
      try{
       float scale=bounds.height()/128f,padding=4*scale;
       int x=Math.round(bounds.left+padding+(bounds.width()-2*padding)*0.875f),y=Math.round(bounds.top+padding+(bounds.height()-2*padding)*8/60f);
       GpuThemeProbe.pixel(pixels,x,y,0xffffffff,"line marker white center");
       GpuThemeProbe.pixel(pixels,Math.round(x+3*scale),y,0xff3b82f6,"line marker blue ring");
      }finally{pixels.recycle();}
     }
     GpuRadioProbe.screenshot("gpu-chart-"+kind+(dark?"-dark":"-light"));
     if(kind.equals("pie")){pieGeometry(bounds);legend(false);GpuApiProbe.click("chart-series-1");AccessibilityNodeInfo hidden=GpuApiProbe.node("chart-plot-pie");GpuUiProbe.check(hidden.getContentDescription().toString().contains("Q2: 38 (hidden)"),"Q2 hidden");hidden.recycle();GpuRadioProbe.screenshot("gpu-chart-pie-hidden"+(dark?"-dark":"-light"));
      GpuApiProbe.click("chart-plot-pie");GpuUiProbe.waitText("chart-tooltip","Q1: 22.43%");
      for(int n:new int[]{0,2,3})GpuApiProbe.click("chart-series-"+n);
      GpuRadioProbe.screenshot("gpu-chart-pie-all-hidden"+(dark?"-dark":"-light"));
      for(int n:new int[]{0,1,2,3})GpuApiProbe.click("chart-series-"+n);
      AccessibilityNodeInfo restored=GpuApiProbe.node("chart-plot-pie");GpuUiProbe.check(!restored.getContentDescription().toString().contains("hidden"),"all series restored");restored.recycle();GpuApiProbe.click("chart-plot-pie");GpuUiProbe.waitText("chart-tooltip","Q1: 16.55%");}else{GpuApiProbe.click("chart-plot-"+kind);GpuApiProbe.node("chart-tooltip").recycle();}
     if(!kind.equals("pie")){
      axisSelection(bounds,kind.equals("bar"),0);
      android.graphics.Bitmap beforeMarker=kind.equals("line")?pixels():null;
      try{
      float pointerScale=bounds.height()/128f,pointerStep=(bounds.width()-8*pointerScale)/4;
      java.lang.Process tap=Runtime.getRuntime().exec(new String[]{"/system/bin/input","tap",String.valueOf(Math.round(bounds.left+4*pointerScale+pointerStep*2.5f)),String.valueOf(bounds.centerY())});
      GpuUiProbe.check(tap.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&tap.exitValue()==0,"touch passes through category indicator");Thread.sleep(400);
      GpuUiProbe.waitText("chart-tooltip","Q3: 31");axisSelection(bounds,kind.equals("bar"),2);
      if(beforeMarker!=null)markerGrowth(bounds,beforeMarker);
      }finally{if(beforeMarker!=null)beforeMarker.recycle();}
     }
     AccessibilityNodeInfo selectedPlot=GpuApiProbe.node("chart-plot-"+kind);android.graphics.Rect after=new android.graphics.Rect();selectedPlot.getBoundsInScreen(after);selectedPlot.recycle();GpuUiProbe.check(bounds.equals(after),"selection must not move plot: "+bounds+" -> "+after);
     android.graphics.Rect tooltip=GpuThemeProbe.bounds("chart-tooltip");GpuUiProbe.check(tooltip.height()<after.height()/2,"tooltip must keep content height: "+tooltip);
     GpuRadioProbe.screenshot("gpu-chart-tooltip-"+kind+(dark?"-dark":"-light"));
     AccessibilityNodeInfo clearable=GpuApiProbe.node("chart-plot-"+kind);
     GpuUiProbe.check(clearable.performAction(AccessibilityNodeInfo.ACTION_CLEAR_SELECTION),"accessible clear selection");clearable.recycle();Thread.sleep(400);noTooltip();
     clearable=GpuApiProbe.node("chart-plot-"+kind);
     GpuUiProbe.check(!clearable.getActionList().contains(AccessibilityNodeInfo.AccessibilityAction.ACTION_CLEAR_SELECTION),"clear action disappears when unselected");clearable.recycle();
     GpuApiProbe.click("chart-plot-"+kind);GpuApiProbe.node("chart-tooltip").recycle();
     android.graphics.Rect title=GpuThemeProbe.bounds("chart-title");
     java.lang.Process outsideTap=Runtime.getRuntime().exec(new String[]{"/system/bin/input","tap",String.valueOf(title.centerX()),String.valueOf(title.centerY())});
     GpuUiProbe.check(outsideTap.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&outsideTap.exitValue()==0,"tap outside plot");Thread.sleep(400);noTooltip();
     GpuApiProbe.click("chart-plot-"+kind);GpuApiProbe.node("chart-tooltip").recycle();
     if(kind.equals("pie")){
      try(java.io.InputStream in=new java.net.URL("http://127.0.0.1:39590/__probe/chart/pie-long").openStream()){while(in.read()!=-1){}}Thread.sleep(700);
      legend(true);GpuApiProbe.click("chart-series-1");AccessibilityNodeInfo longPlot=GpuApiProbe.node("chart-plot-pie");GpuUiProbe.check(longPlot.getContentDescription().toString().contains("South America: 38 (hidden)"),"wrapped legend remains interactive");longPlot.recycle();
      GpuRadioProbe.screenshot("gpu-chart-legend-wrapped"+(dark?"-dark":"-light"));
     }
     GpuApiProbe.click("stream-clear");GpuApiProbe.node("stream-empty").recycle();
    }
    signedAxis(false,dark);signedAxis(true,dark);
    GpuChromeProbe.back();
   }
   GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS bar/line/pie fixture layout, exact accessible data, plot selection and pie legend in light/dark");
  }catch(Throwable e){e.printStackTrace();throw e;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
  System.exit(0);
 }
}
