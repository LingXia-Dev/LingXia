import android.app.UiAutomation;
import android.os.*;
public final class GpuHomeErrorProbe {
 static void fixture(String mode)throws Exception{try(java.io.InputStream in=new java.net.URL("http://127.0.0.1:39590/__probe/home-greet/"+mode).openStream()){while(in.read()!=-1){}}Thread.sleep(500);}
 public static void main(String[]args)throws Exception{
 HandlerThread t=new HandlerThread("home-error");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo info=a.getServiceInfo();info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS|android.accessibilityservice.AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS;a.setServiceInfo(info);Thread.sleep(900);
 GpuChromeProbe.tab("Home");GpuApiProbe.click("home-language-en-US");GpuApiProbe.click("home-appearance-light");GpuStreamAutoHeightProbe.set("home-name","Retry Ada");GpuStreamKeyboardProbe.dismissKeyboard();fixture("fail");GpuApiProbe.click("home-greet");
 GpuUiProbe.waitText("home-greeting","Could not send your greeting. Please try again.");GpuUiProbe.check(GpuApiProbe.node("home-greet").isEnabled(),"retry remains enabled");GpuRadioProbe.screenshot("gpu-home-error-light");
 GpuApiProbe.click("home-language-zh-CN");GpuUiProbe.waitText("home-greeting","\u95ee\u5019\u672a\u80fd\u53d1\u9001\uff0c\u8bf7\u91cd\u8bd5\u3002");GpuApiProbe.click("home-appearance-dark");GpuRadioProbe.screenshot("gpu-home-error-dark");
 fixture("restore");GpuApiProbe.click("home-greet");GpuUiProbe.check(GpuApiProbe.text("home-greeting").contains("Retry Ada"),"retry restores successful greeting");GpuApiProbe.click("home-language-en-US");GpuApiProbe.click("home-appearance-light");System.out.println("PASS localized greeting failure, retry enabled and successful recovery");
 }catch(Throwable error){error.printStackTrace();throw error;}finally{fixture("restore");UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}System.exit(0);
 }
}
