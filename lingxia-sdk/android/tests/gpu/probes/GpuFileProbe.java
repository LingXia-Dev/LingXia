import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;

public class GpuFileProbe {
 static UiAutomation a;
 static void check(boolean v,String s){if(!v)throw new AssertionError(s);}
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static String text(String id)throws Exception{return GpuApiProbe.text(id);}
 static void waitText(String id,String expected)throws Exception{
  String actual="";for(int i=0;i<30;i++){actual=text(id);if(actual.contains(expected))return;Thread.sleep(300);}
  throw new AssertionError(id+" expected "+expected+" got "+actual);
 }
 static void set(String id,String value)throws Exception{
  AccessibilityNodeInfo n=GpuApiProbe.node(id);n.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());n.recycle();Thread.sleep(200);
  n=GpuApiProbe.node(id);Bundle b=new Bundle();b.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,value);check(n.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,b),"set "+id);n.recycle();Thread.sleep(500);
 }
 static boolean hasText(AccessibilityNodeInfo n,String s){
  if(n==null)return false;boolean found=s.equals(String.valueOf(n.getText()))||s.equals(String.valueOf(n.getContentDescription()));
  for(int i=0;!found&&i<n.getChildCount();i++)found=hasText(n.getChild(i),s);n.recycle();return found;
 }
 static void waitPreview()throws Exception{
  for(int i=0;i<60;i++){if(hasText(a.getRootInActiveWindow(),"1 / 1"))return;Thread.sleep(400);}
  throw new AssertionError("PDF viewer page count");
 }
 static AccessibilityNodeInfo findText(AccessibilityNodeInfo n,String s){
  if(n==null)return null;if(s.equals(String.valueOf(n.getText())))return n;
  for(int i=0;i<n.getChildCount();i++){AccessibilityNodeInfo f=findText(n.getChild(i),s);if(f!=null){n.recycle();return f;}}n.recycle();return null;
 }
 static void clickText(String s)throws Exception{
  AccessibilityNodeInfo n=findText(a.getRootInActiveWindow(),s);check(n!=null,"native file row "+s);
  while(!n.isClickable()){AccessibilityNodeInfo parent=n.getParent();n.recycle();n=parent;check(n!=null,"clickable row "+s);}
  check(n.performAction(AccessibilityNodeInfo.ACTION_CLICK),"native row "+s);n.recycle();Thread.sleep(500);
 }
 public static void main(String[] args)throws Exception{
  HandlerThread t=new HandlerThread("file-probe");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
  try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);Thread.sleep(800);
   GpuChromeProbe.tab("API");click("api-choose-file");AccessibilityNodeInfo n=GpuApiProbe.node("file-open-selected");check(!n.isEnabled(),"no selected file disabled");n.recycle();
   click("file-choose");Thread.sleep(500);check(hasText(a.getRootInActiveWindow(),"Choose File"),"managed file picker");GpuRadioProbe.key(4);check(text("file-choose-status").equals("File selection canceled"),"file cancellation");
   GpuChromeProbe.back();click("api-file");set("file-pdf-url","http://127.0.0.1:39871/missing.pdf");click("file-pdf-open");Thread.sleep(900);check(!text("file-pdf-status").equals("Ready"),"download failure visible");
   set("file-office-url","http://127.0.0.1:39871/missing.docx");click("file-office-open");waitText("file-office-status","404");
   set("file-pdf-url","http://127.0.0.1:39871/slow.pdf");click("file-pdf-open");click("file-pdf-transfer");waitText("file-pdf-open","Download Paused");GpuRadioProbe.screenshot("gpu-independent-file-paused");
   GpuChromeProbe.back();click("api-file");waitText("file-pdf-open","Download Paused");check(text("file-pdf-url").equals("http://127.0.0.1:39871/slow.pdf"),"recreated page retains transfer URL");click("file-pdf-transfer");waitPreview();GpuRadioProbe.screenshot("gpu-independent-file-preview");GpuRadioProbe.key(4);waitText("file-pdf-progress","100%");check(text("file-pdf-progress").equals("100%"),"completed progress");
   set("file-office-url","http://127.0.0.1:39871/sample.pdf");check(text("file-office-type").equals("pdf"),"extension detected");click("file-office-open");waitPreview();GpuRadioProbe.key(4);check(text("file-office-status").equals("Opened lx://userdata/sample.pdf"),"fetch writes durable file and previews");
   GpuChromeProbe.back();click("api-choose-file");click("file-choose");clickText("sample.pdf");check(text("file-choose-status").equals("File selected"),"managed selection");check(text("file-selected-type").equals("pdf"),"selected type");click("file-open-selected");waitPreview();GpuRadioProbe.key(4);GpuChromeProbe.back();click("api-file");check(text("file-pdf-open").equals("Download and Preview PDF"),"fresh open-file route");
   GpuChromeProbe.back();GpuChromeProbe.tab("Home");System.out.println("PASS file picker cancel/selection, download/fetch errors, pause/resume, durable fetch/write, native PDF preview and return to GPU");
  }catch(Throwable error){error.printStackTrace();throw error;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}
 }
}
