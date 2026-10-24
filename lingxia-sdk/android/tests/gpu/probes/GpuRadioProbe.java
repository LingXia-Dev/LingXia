import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.graphics.Rect;

public final class GpuRadioProbe {
 static UiAutomation automation;
 static void require(boolean ok,String message){if(!ok)throw new AssertionError(message);}
 static AccessibilityNodeInfo find(AccessibilityNodeInfo n,String id){
  if(n==null)return null;if(("com.lingxia.example.gpu.dev:id/"+id).equals(n.getViewIdResourceName()))return n;
  for(int i=0;i<n.getChildCount();i++){AccessibilityNodeInfo found=find(n.getChild(i),id);if(found!=null){n.recycle();return found;}}n.recycle();return null;
 }
 static AccessibilityNodeInfo node(String id)throws Exception{
  for(int i=0;i<18;i++){
   AccessibilityNodeInfo n=find(automation.getRootInActiveWindow(),id);if(n!=null)return n;
   if(i>1){AccessibilityNodeInfo scroll=find(automation.getRootInActiveWindow(),"explore-scroll");if(scroll==null)scroll=find(automation.getRootInActiveWindow(),"home-scroll");if(scroll!=null){scroll.performAction(i<11?AccessibilityNodeInfo.ACTION_SCROLL_FORWARD:AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD);scroll.recycle();}}
   Thread.sleep(250);
  }throw new AssertionError("missing "+id);
 }
 static void show(String id)throws Exception{
  AccessibilityNodeInfo n=node(id);require(n.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId()),"show "+id);n.recycle();Thread.sleep(450);
  n=node(id);Rect r=new Rect();n.getBoundsInScreen(r);require(n.isVisibleToUser()&&r.height()>=120,"44dp visible target "+id+" "+r);n.recycle();
 }
 static void click(String id)throws Exception{AccessibilityNodeInfo n=node(id);require(n.performAction(AccessibilityNodeInfo.ACTION_CLICK),"click "+id);n.recycle();Thread.sleep(450);}
 static void key(int code)throws Exception{Process p=Runtime.getRuntime().exec(new String[]{"/system/bin/input","keyevent",String.valueOf(code)});require(p.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&p.exitValue()==0,"key injection");Thread.sleep(400);}
 static void selection(String expected)throws Exception{
  int checked=0;
  for(String id:new String[]{"daily","weekly","monthly","quiet"}){
   AccessibilityNodeInfo n=node("pace-"+id);require("android.widget.RadioButton".contentEquals(n.getClassName())&&n.isCheckable(),"radio role");
   if(n.isChecked())checked++;require(n.isChecked()==id.equals(expected),"selection "+id+" expected "+expected);n.recycle();
  }require(checked==1,"exclusive selection");
 }
 static void focused(String id)throws Exception{
  AccessibilityNodeInfo n=automation.findFocus(AccessibilityNodeInfo.FOCUS_INPUT);require(n!=null&&("com.lingxia.example.gpu.dev:id/"+id).equals(n.getViewIdResourceName()),"focus "+id+" actual "+n);n.recycle();
 }
 static void screenshot(String name)throws Exception{android.graphics.Bitmap b=automation.takeScreenshot();try(java.io.FileOutputStream out=new java.io.FileOutputStream("/data/local/tmp/"+name+".png")){b.compress(android.graphics.Bitmap.CompressFormat.PNG,100,out);}b.recycle();}
 public static void main(String[] args)throws Exception{
  HandlerThread thread=new HandlerThread("radio-probe");thread.start();Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
  automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
  try{
   UiAutomation.class.getMethod("connect").invoke(automation);android.accessibilityservice.AccessibilityServiceInfo service=automation.getServiceInfo();service.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(service);
   Thread.sleep(600);click("tab-explore");AccessibilityNodeInfo group=node("pace-group");
   require("android.widget.RadioGroup".contentEquals(group.getClassName()),"group role");require("Your rhythm".contentEquals(group.getContentDescription()),"group name");require(group.getChildCount()==4&&group.getCollectionInfo().getRowCount()==4&&group.getCollectionInfo().getSelectionMode()==1,"group collection");
   AccessibilityNodeInfo option=node("pace-weekly"),parent=option.getParent();require(parent!=null&&parent.equals(group),"virtual parent");parent.recycle();option.recycle();group.recycle();selection("weekly");
   show("pace-quiet");show("pace-daily");
   AccessibilityNodeInfo touch=node("pace-daily");Rect box=new Rect();touch.getBoundsInScreen(box);touch.recycle();
   Process tap=Runtime.getRuntime().exec(new String[]{"/system/bin/input","tap",String.valueOf(box.centerX()),String.valueOf(box.centerY())});require(tap.waitFor(10,java.util.concurrent.TimeUnit.SECONDS)&&tap.exitValue()==0,"touch injection");Thread.sleep(400);selection("daily");show("pace-weekly");click("pace-weekly");selection("weekly");
   System.out.println("PASS group hierarchy, exclusive state, show-on-screen reveal");
   boolean entered=false;
   for(int i=0;i<16;i++){key(KeyEvent.KEYCODE_TAB);AccessibilityNodeInfo focus=automation.findFocus(AccessibilityNodeInfo.FOCUS_INPUT);String id=focus==null?"":focus.getViewIdResourceName();if(focus!=null)focus.recycle();if(id==null)id="";require(!id.endsWith("pace-daily")&&!id.endsWith("pace-quiet")&&!id.endsWith("pace-monthly"),"Tab must use selected option");if(id.endsWith("pace-weekly")){entered=true;break;}}
   require(entered,"Tab group entry");selection("weekly");
   key(KeyEvent.KEYCODE_DPAD_DOWN);selection("quiet");focused("pace-quiet");show("pace-quiet");
   key(KeyEvent.KEYCODE_DPAD_UP);selection("weekly");focused("pace-weekly");
   key(KeyEvent.KEYCODE_DPAD_LEFT);selection("daily");focused("pace-daily");
   key(KeyEvent.KEYCODE_DPAD_LEFT);selection("quiet");focused("pace-quiet");
   key(KeyEvent.KEYCODE_DPAD_RIGHT);selection("daily");focused("pace-daily");
   key(KeyEvent.KEYCODE_SPACE);selection("daily");
   key(KeyEvent.KEYCODE_TAB);focused("pace-toggle");System.out.println("PASS roving Tab, arrows, disabled skip, wrap, scroll reveal and Space no-op");
   click("pace-toggle");group=node("pace-group");require(!group.isEnabled(),"disabled group");group.recycle();
   for(String id:new String[]{"daily","weekly","monthly","quiet"}){AccessibilityNodeInfo n=node("pace-"+id);require(!n.isEnabled()&&!n.performAction(AccessibilityNodeInfo.ACTION_CLICK),"disabled activation");n.recycle();}
   selection("daily");click("pace-toggle");show("pace-weekly");click("pace-weekly");selection("weekly");
   AccessibilityNodeInfo stale=node("pace-weekly");click("tab-hello");require(!stale.performAction(AccessibilityNodeInfo.ACTION_CLICK)&&!stale.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId()),"stale option activation");stale.recycle();click("tab-explore");selection("weekly");show("pace-daily");show("pace-quiet");screenshot("gpu-radio");
   click("theme");selection("weekly");show("pace-daily");show("pace-quiet");screenshot("gpu-radio-alternate-theme");click("theme");selection("weekly");
   System.out.println("PASS disabled group, accessibility selection, stale option rejection and state across tabs");
  }finally{UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
 }
}
