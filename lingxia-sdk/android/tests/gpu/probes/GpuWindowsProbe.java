import android.app.UiAutomation;
import android.os.HandlerThread;
import android.os.Looper;
import android.accessibilityservice.AccessibilityServiceInfo;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import android.graphics.Rect;
import org.json.JSONArray;
import org.json.JSONObject;
public final class GpuWindowsProbe {
    static String targetPackage="com.lingxia.example.gpu.dev";
    static void visit(AccessibilityNodeInfo node,JSONArray result) throws Exception {
        if(node==null)return;
        if(node.getPackageName()!=null && !targetPackage.contentEquals(node.getPackageName()))return;
        Rect rect=new Rect();node.getBoundsInScreen(rect);
        result.put(new JSONObject().put("text",node.getText()==null?"":node.getText().toString())
            .put("description",node.getContentDescription()==null?"":node.getContentDescription().toString())
            .put("bounds",new JSONArray(new int[]{rect.left,rect.top,rect.right,rect.bottom})));
        for(int i=0;i<node.getChildCount();i++){AccessibilityNodeInfo child=node.getChild(i);visit(child,result);if(child!=null)child.recycle();}
    }
    public static void main(String[] args) throws Exception {
        if(args.length>0)targetPackage=args[0];
        HandlerThread thread=new HandlerThread("windows");thread.start();
        Object connection=Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
        UiAutomation automation=(UiAutomation)UiAutomation.class.getConstructor(Looper.class,Class.forName("android.app.IUiAutomationConnection")).newInstance(thread.getLooper(),connection);
        try {
            UiAutomation.class.getMethod("connect").invoke(automation);
            AccessibilityServiceInfo info=automation.getServiceInfo();info.flags|=AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS;automation.setServiceInfo(info);
            Thread.sleep(300);
            JSONArray result=new JSONArray();
            for(AccessibilityWindowInfo window:automation.getWindows()){AccessibilityNodeInfo root=window.getRoot();visit(root,result);if(root!=null)root.recycle();window.recycle();}
            System.out.println(result.toString());
        } finally { UiAutomation.class.getMethod("disconnect").invoke(automation);thread.quitSafely(); }
    }
}
