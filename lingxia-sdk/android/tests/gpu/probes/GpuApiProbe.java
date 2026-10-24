import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;
public class GpuApiProbe {
 static UiAutomation a;
 static void check(boolean ok,String s){if(!ok)throw new AssertionError(s);}
 static String sectionFor(String id){
  if(!id.startsWith("api-")||id.endsWith("-section")||id.equals("api-location"))return null;
  if(id.startsWith("api-cloud-")||id.equals("api-other-app"))return "cloud";
  if(id.equals("api-stream")||id.equals("api-channel")||id.equals("api-bridge"))return "bridge";
  if(id.equals("api-deepseek"))return "navigation";
  if(id.startsWith("api-ui-")||id.equals("api-refresh")||id.equals("api-surface"))return "interface";
  if(id.startsWith("api-system-")||id.equals("api-exit"))return "system";
  if(id.startsWith("api-media-"))return "media";
  if(id.equals("api-file")||id.equals("api-choose-file"))return "file";
  if(id.startsWith("api-device-")||id.equals("api-wifi")||id.equals("api-share")||id.equals("api-clipboard")||id.equals("api-leaveguard"))return "device";
  return null;
 }
 static AccessibilityNodeInfo node(String id)throws Exception{
  for(int i=0;i<24;i++){
   AccessibilityNodeInfo n=GpuRadioProbe.find(a.getRootInActiveWindow(),id);if(n!=null)return n;
   String section=sectionFor(id);
   if(section!=null){
    AccessibilityNodeInfo header=GpuRadioProbe.find(a.getRootInActiveWindow(),"api-"+section+"-section");
    if(header!=null){
     if(String.valueOf(header.getText()).endsWith("collapsed")){
      header.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());
      header.performAction(AccessibilityNodeInfo.ACTION_CLICK);header.recycle();Thread.sleep(250);continue;
     }
     header.recycle();
    }
   }
   AccessibilityNodeInfo scroll=GpuRadioProbe.find(a.getRootInActiveWindow(),"page-scroll");
   if(scroll==null)scroll=GpuRadioProbe.find(a.getRootInActiveWindow(),"home-scroll");
   if(scroll!=null){scroll.performAction(i<14?AccessibilityNodeInfo.ACTION_SCROLL_FORWARD:AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD);scroll.recycle();}
   Thread.sleep(180);
  }
  throw new AssertionError("missing "+id);
 }
 static void click(String id)throws Exception{AccessibilityNodeInfo n=node(id);n.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());n.recycle();Thread.sleep(200);n=node(id);check(n.performAction(AccessibilityNodeInfo.ACTION_CLICK),"click "+id);n.recycle();Thread.sleep(500);}
 static String text(String id)throws Exception{AccessibilityNodeInfo n=node(id);String s=String.valueOf(n.getText());n.recycle();return s;}
 static void status(String expected)throws Exception{check(text("clipboard-status").equals(expected),"status expected "+expected+" got "+text("clipboard-status"));}
 public static void main(String[] args)throws Exception{
 HandlerThread t=new HandlerThread("api-probe");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);Thread.sleep(800);
 GpuChromeProbe.tab("API");click("api-clipboard");node("clipboard-draft").recycle();GpuRadioProbe.screenshot("gpu-clipboard-layout-light");click("clipboard-write-text");status("Wrote text");click("clipboard-read-text");status("Read text");check(text("clipboard-read-value").equals("Hello from lx.clipboard"),"clipboard text roundtrip");
 click("clipboard-write-typed");status("Wrote typed text item");click("clipboard-types");status("Peeked types");check(text("clipboard-types-value").contains("text"),"text type");
 click("clipboard-write-sample-image");status("Wrote sample PNG");click("clipboard-write-sample-image");status("Wrote sample PNG");click("clipboard-read-all");status("Read image");node("clipboard-image").recycle();GpuRadioProbe.screenshot("gpu-clipboard-image");
 click("clipboard-clear");status("Cleared");click("clipboard-read-text");status("No text on clipboard");GpuRadioProbe.screenshot("gpu-clipboard-empty");GpuChromeProbe.back();
 click("api-share");click("share-text");AccessibilityNodeInfo root=a.getRootInActiveWindow();check(root!=null&&!"com.lingxia.example.gpu.dev".contentEquals(root.getPackageName()),"system share sheet");root.recycle();GpuRadioProbe.screenshot("gpu-share-sheet");GpuRadioProbe.key(4);node("share-text").recycle();
 click("share-choose-file");Thread.sleep(700);root=a.getRootInActiveWindow();check(root!=null&&!"com.lingxia.example.gpu.dev".contentEquals(root.getPackageName()),"system file picker");root.recycle();GpuRadioProbe.key(4);check(text("share-status").equals("File selection canceled"),"file cancel");GpuChromeProbe.back();
 GpuChromeProbe.tab("Home");System.out.println("PASS clipboard text/typed/image roundtrips, types, clear/empty, share sheet and file-picker cancellation");
 }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
