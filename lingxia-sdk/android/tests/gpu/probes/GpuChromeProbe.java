import android.app.UiAutomation;
import android.os.*;
import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;

public class GpuChromeProbe {
 static void awaitHomeReady()throws Exception {
  for(int attempt=0;attempt<80;attempt++) {
   AccessibilityNodeInfo home=GpuRadioProbe.find(GpuApiProbe.a.getRootInActiveWindow(),"home-name");
   if(home!=null){home.recycle();GpuApiProbe.a.waitForIdle(500,10000);return;}
   Thread.sleep(250);
  }
  throw new AssertionError("cold Home content did not become ready");
 }
 static AccessibilityNodeInfo chromeButton(AccessibilityNodeInfo node,String label) {
  if(node==null)return null;
  if("android.widget.ImageView".contentEquals(node.getClassName()) && label.contentEquals(node.getContentDescription()==null?"":node.getContentDescription()) && node.isClickable() && node.isVisibleToUser())return node;
  for(int i=0;i<node.getChildCount();i++){AccessibilityNodeInfo found=chromeButton(node.getChild(i),label);if(found!=null){node.recycle();return found;}}
  node.recycle();return null;
 }
 static void back()throws Exception {
  AccessibilityNodeInfo button=null;
  for(int i=0;i<20 && button==null;i++){
   button=chromeButton(GpuApiProbe.a.getRootInActiveWindow(),"Back");
   if(button==null)button=chromeButton(GpuApiProbe.a.getRootInActiveWindow(),"Home");
   if(button==null)Thread.sleep(100);
  }
  GpuApiProbe.check(button!=null,"native navigation Back/Home control");
  Rect box=new Rect();button.getBoundsInScreen(box);button.recycle();
  GpuSearchProbe.shell("/system/bin/input","tap",String.valueOf(box.centerX()),String.valueOf(box.centerY()));Thread.sleep(500);
 }
 static boolean titleAbove(AccessibilityNodeInfo node,String title,int top) {
  if(node==null)return false;
  Rect bounds=new Rect();node.getBoundsInScreen(bounds);
  if(title.contentEquals(node.getText()==null?"":node.getText()) && node.isVisibleToUser() && bounds.bottom<=top && bounds.height()>0){node.recycle();return true;}
  for(int i=0;i<node.getChildCount();i++)if(titleAbove(node.getChild(i),title,top)){node.recycle();return true;}
  node.recycle();return false;
 }
 static AccessibilityNodeInfo tabText(AccessibilityNodeInfo node,String label) {
  if(node==null)return null;
  if(node.isVisibleToUser() && "android.widget.TextView".contentEquals(node.getClassName()) && label.contentEquals(node.getText()==null?"":node.getText()))return node;
  for(int i=0;i<node.getChildCount();i++){AccessibilityNodeInfo found=tabText(node.getChild(i),label);if(found!=null){node.recycle();return found;}}
  node.recycle();return null;
 }
 static void tab(String label)throws Exception {
  AccessibilityNodeInfo node=tabText(GpuApiProbe.a.getRootInActiveWindow(),label);
  GpuApiProbe.check(node!=null,"visible native tab "+label);
  while(node!=null&&!node.isClickable()){AccessibilityNodeInfo parent=node.getParent();node.recycle();node=parent;}
  GpuApiProbe.check(node!=null && node.isVisibleToUser(),"clickable visible tab "+label);
  Rect box=new Rect();node.getBoundsInScreen(box);node.recycle();
  GpuSearchProbe.shell("/system/bin/input","tap",String.valueOf(box.centerX()),String.valueOf(box.centerY()));Thread.sleep(600);
 }
 static void homePixels()throws Exception {
  GpuApiProbe.click("home-appearance-light");
  AccessibilityNodeInfo node=GpuApiProbe.node("home-appearance-light");Rect box=new Rect();node.getBoundsInScreen(box);node.recycle();
  android.graphics.Bitmap shot=GpuApiProbe.a.takeScreenshot(),bitmap=shot.copy(android.graphics.Bitmap.Config.ARGB_8888,false);shot.recycle();
  try{GpuThemeProbe.pixel(bitmap,box.left+12,box.centerY(),0xffffffff,"Home pixels after native tab switch");}finally{bitmap.recycle();}
 }
 public static void main(String[]args)throws Exception {
  HandlerThread thread=new HandlerThread("chrome-probe");thread.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);GpuApiProbe.a=a;GpuRadioProbe.automation=a;
  try {
   UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(info);Thread.sleep(900);
   GpuApiProbe.click("home-language-en-US");tab("API");
   AccessibilityNodeInfo first=GpuApiProbe.node("api-ui-navigation");Rect box=new Rect();first.getBoundsInScreen(box);first.recycle();
   GpuApiProbe.check(titleAbove(a.getRootInActiveWindow(),"API",box.top),"native API title above GPU page content");
   GpuApiProbe.check(GpuRadioProbe.find(a.getRootInActiveWindow(),"page-back")==null,"no GPU replacement navigation bar");
   GpuRadioProbe.screenshot("gpu-native-navbar");
   GpuApiProbe.click("api-clipboard");GpuApiProbe.node("clipboard-write-text").recycle();back();GpuApiProbe.node("api-clipboard").recycle();tab("Home");GpuApiProbe.node("home-name").recycle();
   for(int pass=0;pass<3;pass++){
    tab("ToDo");GpuApiProbe.node("todo-input").recycle();tab("Home");homePixels();
    tab("Media");GpuApiProbe.node("media-selection-count").recycle();tab("Home");homePixels();
   }
   GpuRadioProbe.screenshot("gpu-native-tabbar");
   System.out.println("PASS host navbar title, native Back, host tab navigation and no duplicate GPU navbar");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(a);thread.quitSafely();}
 }
}
