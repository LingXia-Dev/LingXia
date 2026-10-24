import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;

public class GpuChannelProbe {
 static void check(boolean v,String s){if(!v)throw new AssertionError(s);}
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static String text(String id)throws Exception{return GpuApiProbe.text(id);}
 static void waitText(String id,String expected)throws Exception{
  String value="";for(int j=0;j<40;j++){value=text(id);if(value.equals(expected))return;Thread.sleep(250);}
  throw new AssertionError(id+" expected "+expected+" got "+value);
 }
 static void price()throws Exception{for(int j=0;j<40;j++){if(text("channel-price").matches("\\$[0-9]+\\.[0-9]{2}"))return;Thread.sleep(150);}throw new AssertionError("numeric ticker price");}
 static void historyFilled()throws Exception {
  float scale=GpuPaddingProbe.density();
  for(int j=0;j<60;j++) {
   if(GpuThemeProbe.bounds("channel-history").height()/scale>=255)return;
   Thread.sleep(150);
  }
  throw new AssertionError("ticker history reaches its bounded viewport before clicking the moving action row");
 }
 static void tabs()throws Exception {
  float scale=GpuPaddingProbe.density();
  android.graphics.Rect first=GpuThemeProbe.bounds("channel-symbol-AAPL"),middle=GpuThemeProbe.bounds("channel-symbol-GOOGL"),third=GpuThemeProbe.bounds("channel-symbol-MSFT"),last=GpuThemeProbe.bounds("channel-symbol-TSLA"),viewport=GpuThemeProbe.bounds("page-scroll");
  check(first.width()/scale<90&&middle.width()/scale<90&&last.width()/scale<90,"symbol tabs retain content widths");
  check(first.left>=viewport.left+15*scale&&last.right<viewport.right-20*scale,"tabs occupy a leading group, not full width");
  check(Math.abs((middle.left-first.right)/scale-8)<1&&Math.abs((third.left-middle.right)/scale-8)<1&&Math.abs((last.left-third.right)/scale-8)<1,"symbol tab gaps match reference");
  check(Math.abs(first.top-last.top)<2,"symbol tabs share a row");
  android.graphics.Rect price=GpuThemeProbe.bounds("channel-price"),change=GpuThemeProbe.bounds("channel-change");
  check(change.left>price.right&&change.top-price.top>8*scale,"smaller change label aligns to the price baseline rather than its center");
 }
 static int frames()throws Exception{
  java.lang.Process pid=Runtime.getRuntime().exec(new String[]{"/system/bin/pidof","com.lingxia.example.gpu.dev"});
  String appPid=new java.io.BufferedReader(new java.io.InputStreamReader(pid.getInputStream())).readLine();
  check(pid.waitFor()==0&&appPid!=null,"GPU process exists");
  java.lang.Process log=Runtime.getRuntime().exec(new String[]{"/system/bin/logcat","-d","--pid="+appPid.trim(),"-s","LingXia.UI:I","*:S"});
  int count=0;String line;try(java.io.BufferedReader reader=new java.io.BufferedReader(new java.io.InputStreamReader(log.getInputStream()))){
   while((line=reader.readLine())!=null)if(line.contains("present generation="))count++;
  }
  check(log.waitFor()==0,"read GPU frame log");return count;
 }
 public static void main(String[] args)throws Exception{
  String auto=GpuGridProbe.settings("get","system","accelerometer_rotation"),rotation=GpuGridProbe.settings("get","system","user_rotation");
  HandlerThread t=new HandlerThread("channel-probe");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);Thread.sleep(800);
   for(boolean dark:new boolean[]{false,true}) {
   GpuChromeProbe.tab("Home");click(dark?"home-appearance-dark":"home-appearance-light");waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");click("api-channel");waitText("channel-status","Connected");waitText("channel-active-symbol","AAPL");price();
   click("channel-symbol-MSFT");waitText("channel-active-symbol","MSFT");waitText("channel-active-symbol","MSFT");price();tabs();GpuRadioProbe.screenshot(dark?"gpu-channel-layout-dark":"gpu-channel-layout-light");
   GpuGridProbe.settings("put","system","accelerometer_rotation","0");GpuGridProbe.settings("put","system","user_rotation","1");Thread.sleep(1200);
   android.graphics.Bitmap landscape=a.takeScreenshot();check(landscape.getWidth()>landscape.getHeight(),"actual channel landscape");landscape.recycle();
   waitText("channel-status","Connected");waitText("channel-active-symbol","MSFT");price();GpuRadioProbe.screenshot("gpu-independent-channel-landscape");
   GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);Thread.sleep(1000);
   waitText("channel-active-symbol","MSFT");
   historyFilled();GpuTodoProbe.touch("channel-disconnect");waitText("channel-status","Disconnected");waitText("channel-status","Disconnected");
   String last=text("channel-price");Thread.sleep(400);int idleFrames=frames();Thread.sleep(1900);check(last.equals(text("channel-price")),"closed channel stops delivery");
   check(frames()==idleFrames,"disconnected channel does not create an idle frame loop");
   AccessibilityNodeInfo n=GpuApiProbe.node("channel-symbol-TSLA");check(!n.isEnabled(),"disconnected subscription disabled");n.recycle();
   click("channel-reconnect");waitText("channel-status","Connected");waitText("channel-active-symbol","AAPL");click("channel-symbol-TSLA");waitText("channel-active-symbol","TSLA");price();
   GpuChromeProbe.back();click("api-channel");waitText("channel-status","Connected");waitText("channel-active-symbol","AAPL");price();click("channel-disconnect");waitText("channel-status","Disconnected");GpuChromeProbe.back();
   }
   GpuChromeProbe.tab("Home");click("home-appearance-light");waitText("home-appearance-resolved","Light");
   System.out.println("PASS automatic channel connection, ack/init/ticks, subscription across rotation, close delivery and idle-frame stop, reconnect and page teardown");
  }catch(Throwable error){error.printStackTrace();throw error;}finally{GpuGridProbe.settings("put","system","user_rotation",rotation);GpuGridProbe.settings("put","system","accelerometer_rotation",auto);UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
