import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;

public class GpuUiProbe {
 static void check(boolean value,String message){if(!value)throw new AssertionError(message);}
 static void click(String id)throws Exception{GpuApiProbe.click(id);}
 static String text(String id)throws Exception{return GpuApiProbe.text(id);}
 static void waitText(String id,String expected)throws Exception{
  String value="";for(int i=0;i<35;i++){value=text(id);if(value.equals(expected))return;Thread.sleep(150);}
  throw new AssertionError(id+" expected "+expected+" got "+value);
 }
 static void contains(String id,String expected)throws Exception{
  String value="";for(int i=0;i<35;i++){value=text(id);if(value.contains(expected))return;Thread.sleep(150);}
  throw new AssertionError(id+" expected substring "+expected+" got "+value);
 }
 static void set(String id,String value)throws Exception{
  AccessibilityNodeInfo n=GpuApiProbe.node(id);Bundle args=new Bundle();
  args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,value);
  check(n.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,args),"set "+id);n.recycle();Thread.sleep(180);
 }
 static AccessibilityNodeInfo nativeText(AccessibilityNodeInfo n,String value){
  if(n==null)return null;
  String cls=String.valueOf(n.getClassName());
  boolean match=value.contentEquals(n.getText()==null?"":n.getText())
    ||(cls.equals("android.widget.Button")&&value.equalsIgnoreCase(String.valueOf(n.getText())));
  if(match&&!cls.equals("android.widget.EditText"))return n;
  for(int i=0;i<n.getChildCount();i++){AccessibilityNodeInfo found=nativeText(n.getChild(i),value);if(found!=null){n.recycle();return found;}}
  n.recycle();return null;
 }
 static AccessibilityNodeInfo waitNative(String value)throws Exception{
  for(int i=0;i<30;i++){AccessibilityNodeInfo n=nativeText(GpuApiProbe.a.getRootInActiveWindow(),value);if(n!=null)return n;Thread.sleep(150);}
  throw new AssertionError("native text missing: "+value);
 }
 static void clickNative(String value)throws Exception{
  AccessibilityNodeInfo n=waitNative(value);
  while(n!=null&&!n.isClickable()){AccessibilityNodeInfo parent=n.getParent();n.recycle();n=parent;}
  check(n!=null,"clickable native ancestor "+value);check(n.performAction(AccessibilityNodeInfo.ACTION_CLICK),"native click "+value);n.recycle();Thread.sleep(400);
 }
 static void absentNative(String value)throws Exception{
  Thread.sleep(300);AccessibilityNodeInfo n=nativeText(GpuApiProbe.a.getRootInActiveWindow(),value);
  if(n!=null){n.recycle();throw new AssertionError("native text still visible: "+value);}
 }
 static void tapAboveTabBar(String id)throws Exception {
  AccessibilityNodeInfo n=GpuApiProbe.node(id);
  n.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());n.recycle();Thread.sleep(500);
  n=GpuApiProbe.node(id);android.graphics.Rect button=new android.graphics.Rect();n.getBoundsInScreen(button);n.recycle();
  n=waitNative("GPU API");
  while(n!=null&&!n.isClickable()){AccessibilityNodeInfo parent=n.getParent();n.recycle();n=parent;}
  check(n!=null,"native tab item bounds");android.graphics.Rect bar=new android.graphics.Rect();n.getBoundsInScreen(bar);n.recycle();
  check(button.height()>0&&button.bottom<=bar.top,"GPU button obscured by tab bar: "+button+" / "+bar);
  java.lang.Process tap=Runtime.getRuntime().exec(new String[]{"/system/bin/input","tap",String.valueOf(button.centerX()),String.valueOf(button.centerY())});
  check(tap.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&tap.exitValue()==0,"physical tab-bar-adjacent tap");Thread.sleep(400);
  System.out.println("PASS physical "+id+" bounds "+button+" above native tab "+bar);
 }
 static void enter(String type)throws Exception{click("api-ui-"+type);}
 static void back()throws Exception{GpuChromeProbe.back();}
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("ui-probe");thread.start();
  Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  UiAutomation automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  GpuRadioProbe.automation=automation;GpuApiProbe.a=automation;
  try{
   UiAutomation.class.getMethod("connect").invoke(automation);
   android.accessibilityservice.AccessibilityServiceInfo info=automation.getServiceInfo();
   info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(info);Thread.sleep(700);
   click("open-api");
   if(args.length==0||!args[0].equals("controls")){
   enter("navigation");
   String first=text("lifecycle-instance-tag");check(!first.isEmpty(),"instance tag");
   click("lifecycle-bump-logic");click("lifecycle-bump-view");click("lifecycle-bump-module");
   waitText("lifecycle-logic-counter","1");waitText("lifecycle-view-counter","1");waitText("lifecycle-module-counter","1");
   click("ui-navigate-to");String second=text("lifecycle-instance-tag");check(!first.equals(second),"push creates instance");
   waitText("lifecycle-logic-counter","0");waitText("lifecycle-view-counter","0");waitText("lifecycle-module-counter","1");waitText("lifecycle-previous-tag",first);
   click("ui-navigate-back");waitText("lifecycle-instance-tag",first);waitText("lifecycle-logic-counter","1");waitText("lifecycle-view-counter","1");
   click("ui-redirect-to");check(!first.equals(text("lifecycle-instance-tag")),"redirect reruns onLoad");
   waitText("lifecycle-view-counter","1");waitText("lifecycle-logic-counter","1");
   GpuRadioProbe.screenshot("gpu-independent-ui-before-popup");
   click("lifecycle-open-popup");GpuApiProbe.node("lifecycle-popup-close").recycle();GpuRadioProbe.screenshot("gpu-independent-ui-popup");click("lifecycle-popup-close");
   click("lifecycle-open-popup");GpuRadioProbe.key(4);enter("navigation");
   AccessibilityNodeInfo popup=GpuRadioProbe.find(automation.getRootInActiveWindow(),"lifecycle-popup-close");check(popup==null,"popup resets after exit");
   waitText("lifecycle-view-counter","0");waitText("lifecycle-logic-counter","0");waitText("lifecycle-module-counter","1");
   click("ui-switch-tab");GpuApiProbe.node("home-greet").recycle();click("open-api");
   System.out.println("PASS navigation instances, local/Logic/module state, redirect, popup lifetime and switchTab");
   }
   if(args.length>0&&args[0].equals("navigation"))return;

   enter("actionsheet");click("actionsheet-show");clickNative("\u67e5\u770b\u65e5\u5fd7");contains("actionsheet-result","\"id\":\"1\"");
   click("actionsheet-show");GpuRadioProbe.key(4);contains("actionsheet-result","canceled");back();
   enter("modal");set("modal-title","GPU confirmation");set("modal-content","A native dialog from a GPU page");
   click("modal-show");waitNative("GPU confirmation").recycle();GpuRadioProbe.screenshot("gpu-independent-ui-modal");clickNative("Cancel");contains("modal-result","canceled");
   click("modal-show");clickNative("OK");contains("modal-result","\"status\":\"ok\"");click("modal-clear");waitText("modal-result","No result yet");
   click("modal-show-cancel");click("modal-show");absentNative("Cancel");clickNative("OK");contains("modal-result","\"status\":\"ok\"");back();
   System.out.println("PASS native action sheet selection/cancel and modal confirm/cancel/no-cancel");

   enter("toast");set("toast-title","GPU toast");set("toast-duration","10000");
   click("toast-icon");clickNative("None");contains("toast-icon","None");
   click("toast-position");clickNative("Top");contains("toast-position","Top");
   click("toast-show");waitNative("GPU toast").recycle();GpuRadioProbe.screenshot("gpu-independent-ui-toast");click("toast-hide");absentNative("GPU toast");back();
   enter("navbar");set("navbar-title-input","GPU native chrome");click("navbar-set-title");waitNative("GPU native chrome").recycle();
   click("navbar-preset-blue");GpuRadioProbe.screenshot("gpu-independent-ui-navbar");
   set("navbar-bg-input","not-a-color");click("navbar-set-colors");contains("ui-chrome-error","Navigation bar update");
   click("navbar-preset-blue");click("navbar-home-hide");click("navbar-home-auto");click("navbar-reset");back();
   enter("appearance");click("ui-appearance-dark");waitText("ui-appearance-preference","dark");waitText("ui-appearance-resolved","dark");GpuRadioProbe.screenshot("gpu-independent-ui-dark");
   click("ui-appearance-light");waitText("ui-appearance-resolved","light");back();
   enter("tabbar");click("tabbar-show");set("tabbar-item-text","GPU API");click("tabbar-item-update");waitNative("GPU API").recycle();
   set("tabbar-badge-input","42");click("tabbar-badge-set");waitNative("42").recycle();GpuRadioProbe.screenshot("gpu-independent-ui-tabbar");tapAboveTabBar("tabbar-badge-clear");absentNative("42");
   click("tabbar-reddot-show");click("tabbar-reddot-hide");click("tabbar-hide");absentNative("GPU API");click("tabbar-show");waitNative("GPU API").recycle();
   set("tabbar-item-text","API");click("tabbar-item-update");back();back();
   System.out.println("PASS toast show/hide, native title, appearance, tab label/badge and tab visibility");
  }catch(Throwable error){error.printStackTrace();throw error;}
  finally{UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
