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
    void click(GpuView view,String id,ExecutorService worker) throws Exception {
        JSONObject frame=snapshot(view), target=node(view,id);assertNotNull(target);
        Field pageField=GpuView.class.getDeclaredField("nativePage");pageField.setAccessible(true);
        long page=main(()->pageField.getLong(view));
        Method click=GpuView.class.getDeclaredMethod("nativeClick",long.class,long.class,long.class);click.setAccessible(true);
        assertEquals(true,worker.submit(()->click.invoke(null,page,frame.getLong("generation"),target.getLong("on_click"))).get());
        Method redraw=GpuView.class.getDeclaredMethod("requestFrame");redraw.setAccessible(true);
        main(()->{redraw.invoke(view);return null;});
    }
    void verifyHostImageCancellation() throws Exception {
        AtomicReference<GpuView.Listener> host=new AtomicReference<>();
        await(()->main(()->{
            Field pages=com.lingxia.lxapp.GpuPage.class.getDeclaredField("PAGES");pages.setAccessible(true);
            for(Object page:((java.util.Map<?,?>)pages.get(null)).values()) {
                Field content=page.getClass().getDeclaredField("content");content.setAccessible(true);Object view=content.get(page);
                Field ready=GpuView.class.getDeclaredField("bridgeReady");ready.setAccessible(true);if(!ready.getBoolean(view))continue;
                Field listener=GpuView.class.getDeclaredField("listener");listener.setAccessible(true);host.set((GpuView.Listener)listener.get(view));return true;
            }return false;
        }));
        java.net.ServerSocket server=new java.net.ServerSocket(0,4,java.net.InetAddress.getByName("127.0.0.1"));
        ExecutorService serverWork=Executors.newCachedThreadPool();
        java.util.List<java.net.Socket> sockets=new java.util.concurrent.CopyOnWriteArrayList<>();
        AtomicInteger started=new AtomicInteger(),closed=new AtomicInteger();
        AtomicReference<byte[]> fast=new AtomicReference<>();
        AtomicReference<Throwable> failure=new AtomicReference<>();
        serverWork.execute(()->{try {
            for(int i=0;i<3;i++){
                java.net.Socket socket=server.accept();sockets.add(socket);socket.setSoTimeout(15000);
                serverWork.execute(()->{try {
                    java.io.BufferedReader reader=new java.io.BufferedReader(new java.io.InputStreamReader(socket.getInputStream(),java.nio.charset.StandardCharsets.US_ASCII));
                    String request=reader.readLine(),line;while((line=reader.readLine())!=null&&!line.isEmpty()){}
                    if(request.contains("/slow")){started.incrementAndGet();if(reader.read()==-1)closed.incrementAndGet();}
                    else {byte[] body={1,2,3};socket.getOutputStream().write("HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\n".getBytes(java.nio.charset.StandardCharsets.US_ASCII));socket.getOutputStream().write(body);socket.getOutputStream().flush();}
                    socket.close();
                }catch(Throwable e){failure.compareAndSet(null,e);}});
            }
        }catch(Throwable e){if(!server.isClosed())failure.compareAndSet(null,e);}});
        String base="http://127.0.0.1:"+server.getLocalPort();
        Runnable first=main(()->host.get().onImageRequest(base+"/slow-a",bytes->{}));
        Runnable second=main(()->host.get().onImageRequest(base+"/slow-b",bytes->{}));
        Runnable[] quick={null};
        try {
            await(()->started.get()==2);
            long begin=android.os.SystemClock.uptimeMillis();
            main(()->{first.run();second.run();quick[0]=host.get().onImageRequest(base+"/fast",fast::set);return null;});
            try {await(()->fast.get()!=null&&closed.get()==2);}
            catch(AssertionError error){throw new AssertionError("fast="+(fast.get()!=null)+" closed="+closed.get()+" serverFailure="+failure.get(),error);}
            assertTrue("obsolete transfers must not hold the two image workers",android.os.SystemClock.uptimeMillis()-begin<5000);
            assertArrayEquals(new byte[]{1,2,3},fast.get());assertNull(failure.get());
        }finally{
            main(()->{first.run();second.run();if(quick[0]!=null)quick[0].run();return null;});
            server.close();for(java.net.Socket socket:sockets)socket.close();serverWork.shutdownNow();
        }
    }
    @Test public void imageRetryCancellationAndKeyedRowsSurviveRealPresentation() throws Exception {
        Intent launch=instrumentation.getTargetContext().getPackageManager().getLaunchIntentForPackage(instrumentation.getTargetContext().getPackageName());
        launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK|Intent.FLAG_ACTIVITY_CLEAR_TASK);
        instrumentation.getTargetContext().startActivity(launch);
        AtomicReference<Activity> activity=new AtomicReference<>();
        await(()->main(()->{for(Activity a:ActivityLifecycleMonitorRegistry.getInstance().getActivitiesInStage(Stage.RESUMED)){activity.set(a);return true;}return false;}));
        verifyHostImageCancellation();
        // The launcher may hand off to the lxapp activity while the host boots.
        await(()->main(()->{for(Activity a:ActivityLifecycleMonitorRegistry.getInstance().getActivitiesInStage(Stage.RESUMED)){activity.set(a);return true;}return false;}));
        String source;
        try(java.io.InputStream stream=instrumentation.getContext().getAssets().open("pages/home/index.ui.js")){
            ByteArrayOutputStream bytes=new ByteArrayOutputStream();byte[] chunk=new byte[8192];int n;
            while((n=stream.read(chunk))!=-1)bytes.write(chunk,0,n);source=bytes.toString("UTF-8");
        }
        ByteArrayOutputStream png=new ByteArrayOutputStream();Bitmap bitmap=Bitmap.createBitmap(2,2,Bitmap.Config.ARGB_8888);
        bitmap.eraseColor(0xff2388dd);bitmap.compress(Bitmap.CompressFormat.PNG,100,png);bitmap.recycle();
        AtomicInteger attempts=new AtomicInteger(),canceled=new AtomicInteger();AtomicReference<Throwable> error=new AtomicReference<>();
        ExecutorService worker=Executors.newSingleThreadExecutor();
        GpuView view=main(()->{
            GpuView candidate=new GpuView(activity.get(),source,worker,new GpuView.Listener(){
                public void onLoaded(){} public void onFrame(String metadata){} public void onError(Throwable e){error.set(e);}
                public Runnable onImageRequest(String src,java.util.function.Consumer<byte[]> result){
                    if(src.equals("slow.png"))return ()->canceled.incrementAndGet();
                    result.accept(attempts.incrementAndGet()==1?null:png.toByteArray());return ()->{};
                }
            },false);
            activity.get().addContentView(candidate,new ViewGroup.LayoutParams(-1,-1));candidate.resume();return candidate;
        });
        try {
            await(()->node(view,"image-state")!=null && "failed".equals(node(view,"image-state").optString("text")));
            assertEquals(1,attempts.get());click(view,"retry",worker);
            await(()->"ready".equals(node(view,"image-state").optString("text")));assertEquals(2,attempts.get());
            click(view,"remove-slow",worker);await(()->canceled.get()==1);
            click(view,"item-a",worker);await(()->"a:1".equals(node(view,"item-a").optString("text")));
            click(view,"reorder",worker);await(()->node(view,"item-b").getJSONArray("bounds").getDouble(1)<node(view,"item-a").getJSONArray("bounds").getDouble(1));
            assertEquals("a:1",node(view,"item-a").getString("text"));assertEquals("b:0",node(view,"item-b").getString("text"));
            JSONObject list=node(view,"padded-list"),last=node(view,"row-99");assertNotNull(last);
            JSONObject scroll=list.getJSONObject("scroll");assertEquals(scroll.getDouble("max_offset"),scroll.getDouble("offset"),0.1);
            JSONArray box=list.getJSONArray("bounds"),row=last.getJSONArray("bounds");
            assertEquals(box.getDouble(1)+box.getDouble(3)-22,row.getDouble(1)+row.getDouble(3),0.2);
            assertNull(error.get());assertEquals(2,attempts.get());
        } catch(AssertionError failure) {
            throw new AssertionError("attempts="+attempts.get()+" canceled="+canceled.get()+" error="+error.get()+" state="+node(view,"image-state")+" list="+node(view,"padded-list")+" last="+node(view,"row-99"),failure);
        } finally {main(()->{view.dispose();((ViewGroup)view.getParent()).removeView(view);return null;});worker.shutdown();}
    }
}
