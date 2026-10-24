import android.app.UiAutomation;
import android.os.*;
public final class GpuCaptureLayoutProbe {
 static void fixture(String mode)throws Exception{try(java.io.InputStream in=new java.net.URL("http://127.0.0.1:39590/__probe/capture/"+mode).openStream()){while(in.read()!=-1){}}Thread.sleep(400);}
 public static void main(String[] args)throws Exception{
 HandlerThread t=new HandlerThread("capture-layout");t.start();Object c=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();UiAutomation a=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(t.getLooper(),c);GpuRadioProbe.automation=a;GpuApiProbe.a=a;
 try{UiAutomation.class.getMethod("connect").invoke(a);android.accessibilityservice.AccessibilityServiceInfo i=a.getServiceInfo();i.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;a.setServiceInfo(i);Thread.sleep(900);
 for(boolean dark:new boolean[]{false,true}){
 GpuChromeProbe.tab("Home");GpuApiProbe.click(dark?"home-appearance-dark":"home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved",dark?"Dark":"Light");GpuChromeProbe.tab("API");GpuApiProbe.click("api-media-ScanCode");
 GpuUiProbe.waitText("media-scan-result","No result yet");GpuUiProbe.waitText("media-scan-type","--");GpuApiProbe.click("media-openScanSourcePicker");GpuUiProbe.clickNative("Camera & Photo");GpuApiProbe.click("media-openScanTypePicker");GpuUiProbe.clickNative("qrCode");GpuRadioProbe.screenshot("gpu-scan-empty-"+(dark?"dark":"light"));
 GpuScanProbe.open();GpuUiProbe.check(GpuUiProbe.nativeText(a.getRootInActiveWindow(),"Album")!=null,"native scanner open");GpuRadioProbe.key(4);Thread.sleep(600);GpuUiProbe.waitText("media-startScan","Start Scan");GpuUiProbe.waitText("media-scan-result","No result yet");
 fixture("result");GpuUiProbe.waitText("media-scan-result","LingXia GPU scanner verification");GpuUiProbe.waitText("media-scan-type","QR_CODE");GpuRadioProbe.screenshot("gpu-scan-result-"+(dark?"dark":"light"));GpuChromeProbe.back();
 GpuApiProbe.click("api-media-SaveToAlbum");GpuFileLayoutProbe.enabled("media-captureImageForAlbum",true);GpuFileLayoutProbe.enabled("media-captureVideoForAlbum",true);GpuRadioProbe.screenshot("gpu-save-album-"+(dark?"dark":"light"));fixture("busy");GpuUiProbe.waitText("media-captureImageForAlbum","Saving...");GpuFileLayoutProbe.enabled("media-captureImageForAlbum",false);GpuFileLayoutProbe.enabled("media-captureVideoForAlbum",false);fixture("idle");GpuFileLayoutProbe.enabled("media-captureImageForAlbum",true);GpuChromeProbe.back();
 }
 GpuChromeProbe.tab("Home");GpuApiProbe.click("home-appearance-light");GpuUiProbe.waitText("home-appearance-resolved","Light");System.out.println("PASS scan/save light/dark layouts, native scan settings and scanner cancel, result fixture and album busy gates");
 }finally{UiAutomation.class.getMethod("disconnect").invoke(a);t.quitSafely();}System.exit(0);
 }
}
