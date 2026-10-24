import android.app.UiAutomation;
import android.os.*;
import android.view.accessibility.AccessibilityNodeInfo;
public final class GpuFileLayoutProbe {
 static void enabled(String id,boolean value)throws Exception{AccessibilityNodeInfo n=GpuApiProbe.node(id);GpuUiProbe.check(n.isEnabled()==value,id+" enabled="+value);n.recycle();}
 public static void main(String[]args)throws Exception{
  HandlerThread t=new HandlerThread("file-layout");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;GpuFileProbe.a=a;
  try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);Thread.sleep(900);
   for(boolean dark:new boolean[]{false,true}){
    GpuChromeProbe.tab("Home");GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");GpuApiProbe.click("api-choose-file");enabled("file-open-selected",false);GpuUiProbe.waitText("file-selected-path","None");GpuUiProbe.waitText("file-selected-type","Unknown");GpuRadioProbe.screenshot(dark?"gpu-file-choose-dark":"gpu-file-choose-light");
    AccessibilityNodeInfo option=GpuApiProbe.node("file-show-menu");boolean was=option.isChecked();option.recycle();GpuApiProbe.click("file-show-menu");option=GpuApiProbe.node("file-show-menu");GpuUiProbe.check(option.isChecked()!=was,"share option toggled");option.recycle();GpuApiProbe.click("file-show-menu");
    GpuApiProbe.click("file-choose");GpuUiProbe.check(GpuFileProbe.hasText(a.getRootInActiveWindow(),"Choose File"),"actual host chooser");GpuRadioProbe.key(4);GpuUiProbe.waitText("file-choose-status","File selection canceled");GpuChromeProbe.back();GpuApiProbe.click("api-file");GpuApiProbe.node("file-pdf-url").recycle();GpuRadioProbe.screenshot(dark?"gpu-file-open-dark":"gpu-file-open-light");
    GpuFileProbe.set("file-office-url","https://example.com/report.xlsx");GpuUiProbe.waitText("file-office-type","xlsx");GpuRadioProbe.key(4);
    for(String mode:new String[]{"downloading","paused","completed","streaming"}){
     try(java.io.InputStream in=new java.net.URL("http://127.0.0.1:39590/__probe/file/"+mode).openStream()){while(in.read()!=-1){}}Thread.sleep(400);
     GpuUiProbe.waitText("file-pdf-progress",mode.equals("completed")?"100%":mode.equals("streaming")?"Streaming":"42%");enabled("file-pdf-transfer",mode.equals("downloading")||mode.equals("paused")||mode.equals("streaming"));
     GpuApiProbe.node("file-pdf-transfer").performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());Thread.sleep(200);GpuRadioProbe.screenshot("gpu-file-"+mode+(dark?"-dark":"-light"));
    }
    GpuChromeProbe.back();
   }
   GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS file light/dark routes, option toggle, native picker cancel, type detection and transfer presentation fixtures");
  }catch(Throwable e){e.printStackTrace();throw e;}finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}System.exit(0);
 }
}
