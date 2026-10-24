import android.app.UiAutomation;
import android.os.Bundle;
import android.os.HandlerThread;
import android.os.Looper;
import android.graphics.Bitmap;
import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.FileOutputStream;

public final class GpuCacheEmojiProbe {
    static UiAutomation automation;
    static void require(boolean ok,String message){if(!ok)throw new AssertionError(message);}
    static AccessibilityNodeInfo find(AccessibilityNodeInfo node){
        if(node==null)return null;
        if("com.lingxia.example.gpu.dev:id/greeting-input".equals(node.getViewIdResourceName()))return node;
        for(int i=0;i<node.getChildCount();i++){AccessibilityNodeInfo found=find(node.getChild(i));if(found!=null){node.recycle();return found;}}
        node.recycle();return null;
    }
    static AccessibilityNodeInfo input(){return find(automation.getRootInActiveWindow());}
    static AccessibilityNodeInfo await(String value)throws Exception{
        for(int i=0;i<60;i++){
            AccessibilityNodeInfo node=input();
            if(node!=null&&value.contentEquals(node.getText()==null?"":node.getText()))return node;
            if(node!=null)node.recycle();Thread.sleep(100);
        }
        throw new AssertionError("input text timeout");
    }
    public static void main(String[] args)throws Exception{
        HandlerThread thread=new HandlerThread("cache-emoji");thread.start();
        Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
        automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
        try{
            UiAutomation.class.getMethod("connect").invoke(automation);
            android.accessibilityservice.AccessibilityServiceInfo info=automation.getServiceInfo();
            info.flags|=android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;automation.setServiceInfo(info);
            Thread.sleep(700);long previous=0;
            String[] values={"A\ud83d\ude00B","A\ud83d\udc4d\ud83c\udffdB"};
            for(int variant=0;variant<values.length;variant++){
                AccessibilityNodeInfo node=input();require(node!=null,"input missing");
                Bundle argsText=new Bundle();argsText.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,values[variant]);
                require(node.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,argsText),"set text");node.recycle();
                node=await(values[variant]);node.recycle();Thread.sleep(1600);
                long expected=0;
                for(int frame=0;frame<6;frame++){
                    node=await(values[variant]);Rect box=new Rect();node.getBoundsInScreen(box);node.recycle();
                    Bitmap hardware=automation.takeScreenshot();require(hardware!=null,"screenshot missing");
                    Bitmap screenshot=hardware.copy(Bitmap.Config.ARGB_8888,false);hardware.recycle();require(screenshot!=null,"pixel copy missing");
                    long hash=1;int colored=0;
                    for(int y=box.top;y<box.bottom;y++)for(int x=box.left;x<box.right;x++){
                        int pixel=screenshot.getPixel(x,y),r=(pixel>>16)&255,g=(pixel>>8)&255,b=pixel&255;
                        if(r>160&&g>80&&b<120){colored++;hash=31*hash+pixel+x+4096L*y;}
                    }
                    require(colored>30,"bitmap emoji missing in phase "+frame);
                    if(frame==0)expected=hash;else require(hash==expected,"emoji pixels changed across caret phases");
                    try(FileOutputStream output=new FileOutputStream("/data/local/tmp/gpu-cache-emoji-"+variant+"-"+frame+".png")){screenshot.compress(Bitmap.CompressFormat.PNG,100,output);}
                    screenshot.recycle();Thread.sleep(300);
                }
                if(variant>0)require(expected!=previous,"new emoji reused old pixels");previous=expected;
                System.out.println("PASS cached bitmap glyph phases and replacement: "+variant);
            }
        }finally{UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely();}
    }
}
