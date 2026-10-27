package com.lingxia.ui.tests;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Intent;
import android.graphics.Bitmap;
import android.view.ViewGroup;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;
import androidx.test.runner.lifecycle.ActivityLifecycleMonitorRegistry;
import androidx.test.runner.lifecycle.Stage;
import com.lingxia.ui.GpuView;
import java.io.ByteArrayOutputStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.concurrent.Executors;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.atomic.AtomicReference;
import java.util.concurrent.atomic.AtomicInteger;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import static org.junit.Assert.*;

/** Exercises real presentation with deterministic resource failures in the test APK. */
@RunWith(AndroidJUnit4.class)
public class GpuProductContractTest {
    final Instrumentation instrumentation=InstrumentationRegistry.getInstrumentation();
    interface Work<T> { T get() throws Exception; }
    <T> T main(Work<T> work) throws Exception {
        AtomicReference<T> result=new AtomicReference<>(); AtomicReference<Throwable> error=new AtomicReference<>();
        instrumentation.runOnMainSync(()->{try {result.set(work.get());} catch(Throwable e){error.set(e);}});
        if(error.get()!=null)throw new AssertionError(error.get()); return result.get();
    }
    static boolean hasGpuView(android.view.View view) {
        if(view instanceof GpuView)return true;
        if(view instanceof ViewGroup)for(int i=0;i<((ViewGroup)view).getChildCount();i++)if(hasGpuView(((ViewGroup)view).getChildAt(i)))return true;
        return false;
    }
    void await(Work<Boolean> condition) throws Exception {
        long end=android.os.SystemClock.uptimeMillis()+15000;
        do {if(condition.get())return;Thread.sleep(50);} while(android.os.SystemClock.uptimeMillis()<end);
        fail("GPU product contract deadline");
    }
    JSONObject snapshot(GpuView view) throws Exception {
        Field field=GpuView.class.getDeclaredField("snapshot");field.setAccessible(true);
        return main(()->(JSONObject)field.get(view));
    }
    JSONObject node(GpuView view,String id) throws Exception {
        JSONObject frame=snapshot(view);if(frame==null)return null;
        JSONArray nodes=frame.getJSONArray("nodes");
        for(int i=0;i<nodes.length();i++)if(id.equals(nodes.getJSONObject(i).optString("test_id")))return nodes.getJSONObject(i);
        return null;
    }
    /** A real tap at the node's center, through the host's pointer policy. */
    void click(GpuView view,String id) throws Exception {
        JSONObject target=node(view,id);assertNotNull(target);
        JSONArray visible=target.optJSONArray("visible_bounds");
        JSONArray b=visible!=null?visible:target.getJSONArray("bounds");
        Field scaleField=GpuView.class.getDeclaredField("frameScale");scaleField.setAccessible(true);
        Field hostField=GpuView.class.getDeclaredField("host");hostField.setAccessible(true);
        Method pointer=GpuView.class.getDeclaredMethod("nativePointer",long.class,int.class,float.class,float.class,float.class,float.class,float.class,float.class);pointer.setAccessible(true);
        main(()->{
            float scale=scaleField.getFloat(view);long host=hostField.getLong(view);
            float x=(float)(b.getDouble(0)+b.getDouble(2)/2)*scale,y=(float)(b.getDouble(1)+b.getDouble(3)/2)*scale;
            pointer.invoke(null,host,0,x,y,8f,0f,0f,50f);pointer.invoke(null,host,2,x,y,8f,0f,0f,50f);return null;
        });
    }
    /** Image loads, retries and cancellation run in Rust and are covered by the host tests and the device image checks. */
    @Test public void keyedRowsAndEdgePaddingSurviveRealPresentation() throws Exception {
        Intent launch=instrumentation.getTargetContext().getPackageManager().getLaunchIntentForPackage(instrumentation.getTargetContext().getPackageName());
        launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK|Intent.FLAG_ACTIVITY_CLEAR_TASK);
        instrumentation.getTargetContext().startActivity(launch);
        AtomicReference<Activity> activity=new AtomicReference<>();
        await(()->main(()->{for(Activity a:ActivityLifecycleMonitorRegistry.getInstance().getActivitiesInStage(Stage.RESUMED)){activity.set(a);return true;}return false;}));
        // Wait for the host's own GPU page: native methods are registered only once the host initialises.
        await(()->main(()->{for(Activity a:ActivityLifecycleMonitorRegistry.getInstance().getActivitiesInStage(Stage.RESUMED)){
            if(hasGpuView(a.getWindow().getDecorView())){activity.set(a);return true;}}return false;}));
        String source;
        try(java.io.InputStream stream=instrumentation.getContext().getAssets().open("pages/home/index.js")){
            ByteArrayOutputStream bytes=new ByteArrayOutputStream();byte[] chunk=new byte[8192];int n;
            while((n=stream.read(chunk))!=-1)bytes.write(chunk,0,n);source=bytes.toString("UTF-8");
        }
        AtomicReference<Throwable> error=new AtomicReference<>();
        GpuView view=main(()->{
            GpuView candidate=new GpuView(activity.get(),source,"test:product-contract",new GpuView.Listener(){
                public void onLoaded(){} public void onFrame(String metadata){} public void onError(Throwable e){error.set(e);}
            });
            activity.get().addContentView(candidate,new ViewGroup.LayoutParams(-1,-1));candidate.resume();return candidate;
        });
        try {
            await(()->node(view,"item-a")!=null);
            click(view,"item-a");await(()->"a:1".equals(node(view,"item-a").optString("text")));
            click(view,"reorder");await(()->node(view,"item-b").getJSONArray("bounds").getDouble(1)<node(view,"item-a").getJSONArray("bounds").getDouble(1));
            assertEquals("a:1",node(view,"item-a").getString("text"));assertEquals("b:0",node(view,"item-b").getString("text"));
            JSONObject list=node(view,"padded-list"),last=node(view,"row-99");assertNotNull(last);
            JSONObject scroll=list.getJSONObject("scroll");assertEquals(scroll.getDouble("max_offset"),scroll.getDouble("offset"),0.1);
            JSONArray box=list.getJSONArray("bounds"),row=last.getJSONArray("bounds");
            assertEquals(box.getDouble(1)+box.getDouble(3)-22,row.getDouble(1)+row.getDouble(3),0.2);
            assertNull(error.get());
        } catch(AssertionError failure) {
            throw new AssertionError("error="+error.get()+" list="+node(view,"padded-list")+" last="+node(view,"row-99"),failure);
        } finally {main(()->{view.dispose();((ViewGroup)view.getParent()).removeView(view);return null;});}
    }
}
