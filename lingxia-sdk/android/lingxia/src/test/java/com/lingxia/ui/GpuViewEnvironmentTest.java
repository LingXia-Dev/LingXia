package com.lingxia.ui;

import android.content.res.Configuration;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.annotation.Config;
import java.lang.reflect.Method;
import java.util.Collections;
import java.util.List;
import java.util.concurrent.AbstractExecutorService;
import java.util.concurrent.TimeUnit;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 28)
public class GpuViewEnvironmentTest {
    @Test public void retainedPageResamplesFontsWithoutAViewConfigurationCallback() throws Exception {
        android.content.Context base = RuntimeEnvironment.getApplication();
        Configuration normal = new Configuration(base.getResources().getConfiguration());
        normal.fontScale = 1.0f;
        Configuration large = new Configuration(normal); large.fontScale = 2.0f;
        android.content.Context context = base.createConfigurationContext(normal);
        GpuView view = new GpuView(context, "", new DeferredWorker(), null);
        Method measure = GpuView.class.getDeclaredMethod("measureEnvironment", float.class);
        measure.setAccessible(true);
        JSONObject first = new JSONObject((String)measure.invoke(view, 1.0f));
        android.util.DisplayMetrics metrics = new android.util.DisplayMetrics();
        metrics.setTo(context.getResources().getDisplayMetrics());
        metrics.scaledDensity = metrics.density * large.fontScale;
        context.getResources().updateConfiguration(large, metrics);
        JSONObject enlarged = new JSONObject((String)measure.invoke(view, 1.0f));
        assertEquals(14.0, first.getJSONArray("textScale").getJSONArray(14).getDouble(1), 0.01);
        assertEquals(28.0, enlarged.getJSONArray("textScale").getJSONArray(14).getDouble(1), 0.01);
        view.dispose();
    }

    @Test public void imageDescriptionsAreProjectedAndDecorativeImagesAreExcluded() throws Exception {
        GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "", new DeferredWorker(), null);
        activate(view);
        publish(view, new JSONObject("{nodes:["
                + "{identity:'photo',tag:'image',accessibility_label:'A mountain lake',bounds:[0,0,64,48]},"
                + "{identity:'decoration',tag:'image',accessibility_hidden:true,bounds:[0,50,64,48]}]}"));
        android.view.View surface = view.getChildAt(0);
        java.lang.reflect.Field field = GpuView.class.getDeclaredField("accessibilityIds"); field.setAccessible(true);
        GpuAccessibilityIds ids = (GpuAccessibilityIds)field.get(view);
        android.view.accessibility.AccessibilityNodeProvider provider = surface.getAccessibilityNodeProvider();
        android.view.accessibility.AccessibilityNodeInfo root = provider.createAccessibilityNodeInfo(android.view.View.NO_ID);
        assertEquals(1, root.getChildCount());
        android.view.accessibility.AccessibilityNodeInfo photo = provider.createAccessibilityNodeInfo(ids.id(0));
        assertEquals("android.widget.ImageView", photo.getClassName());
        assertEquals("A mountain lake", photo.getContentDescription());
        assertFalse(photo.isClickable());
        assertNull(provider.createAccessibilityNodeInfo(ids.id(1)));
        photo.recycle(); root.recycle(); view.dispose();
    }
    // Environment sampling must not depend on loading a native renderer in the JVM.
    private static class DeferredWorker extends AbstractExecutorService {
        public void execute(Runnable command) { }
        public void shutdown() { }
        public List<Runnable> shutdownNow() { return Collections.emptyList(); }
        public boolean isShutdown() { return false; }
        public boolean isTerminated() { return false; }
        public boolean awaitTermination(long timeout, TimeUnit unit) { return false; }
    }

    @Test public void interruptedRefreshGestureIsCanceledOnce() throws Exception {
        for (int interruption = 0; interruption < 3; interruption++) {
            GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "", new DeferredWorker(), null);
            android.view.View surface = view.getChildAt(0);
            java.util.ArrayList<Integer> events = new java.util.ArrayList<>();
            view.setPullToRefresh(event -> {
                events.add(event.getActionMasked());
                return true;
            });
            android.view.MotionEvent move = android.view.MotionEvent.obtain(0, 1, android.view.MotionEvent.ACTION_MOVE, 10, 200, 0);
            try { assertTrue(surface.onTouchEvent(move)); } finally { move.recycle(); }
            if (interruption == 0) view.onWindowFocusChanged(false);
            else if (interruption == 1) view.pause();
            else view.surfaceDestroyed(((android.view.SurfaceView)surface).getHolder());
            view.onWindowFocusChanged(false);
            assertEquals(java.util.Arrays.asList(android.view.MotionEvent.ACTION_MOVE, android.view.MotionEvent.ACTION_CANCEL), events);
        }
    }

    @Test public void retainedPageVisibilityHidesItsSurfaceAndResumeRestoresIt() throws Exception {
        android.app.Activity activity=org.robolectric.Robolectric.buildActivity(android.app.Activity.class).setup().get();
        android.widget.FrameLayout parent=new android.widget.FrameLayout(activity);
        GpuView view=new GpuView(activity,"",new DeferredWorker(),null);
        parent.addView(view);activity.setContentView(parent);
        android.view.View surface=view.getChildAt(0);
        view.resume();assertEquals(android.view.View.VISIBLE,surface.getVisibility());
        parent.setVisibility(android.view.View.GONE);
        assertEquals(android.view.View.INVISIBLE,surface.getVisibility());
        parent.setVisibility(android.view.View.VISIBLE);
        assertEquals(android.view.View.VISIBLE,surface.getVisibility());
        view.pause();assertEquals(android.view.View.INVISIBLE,surface.getVisibility());
        parent.setVisibility(android.view.View.GONE);parent.setVisibility(android.view.View.VISIBLE);
        assertEquals(android.view.View.INVISIBLE,surface.getVisibility());
        view.resume();assertEquals(android.view.View.VISIBLE,surface.getVisibility());
    }

    @Test public void accessibilityIdsPreserveFocusAcrossReorderAndRejectRemovedNodes() throws Exception {
        GpuView view=new GpuView(RuntimeEnvironment.getApplication(),"",new DeferredWorker(),null);
        android.view.View surface=view.getChildAt(0);
        android.widget.FrameLayout parent=new android.widget.FrameLayout(view.getContext()); parent.addView(view);
        java.lang.reflect.Field active=GpuView.class.getDeclaredField("active"); active.setAccessible(true); active.setBoolean(view,true);
        Method accept=surface.getClass().getDeclaredMethod("acceptFrame",JSONObject.class,float.class); accept.setAccessible(true);
        java.lang.reflect.Field mapping=GpuView.class.getDeclaredField("accessibilityIds"); mapping.setAccessible(true);
        GpuAccessibilityIds ids=(GpuAccessibilityIds)mapping.get(view);
        JSONObject a=new JSONObject("{identity:'a',tag:'button',text:'Alpha',on_click:1,bounds:[0,0,100,40]}"),
            b=new JSONObject("{identity:'b',tag:'button',text:'Beta',on_click:2,bounds:[0,50,100,40]}");
        JSONObject first=new JSONObject().put("nodes",new org.json.JSONArray().put(a).put(b));
        accept.invoke(surface,first,1f); int alpha=ids.id(0),beta=ids.id(1);
        android.view.accessibility.AccessibilityNodeProvider provider=surface.getAccessibilityNodeProvider();
        int focus=android.view.accessibility.AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS;
        int clear=android.view.accessibility.AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS;
        assertTrue(provider.performAction(alpha,focus,null));
        JSONObject reordered=new JSONObject().put("nodes",new org.json.JSONArray().put(b).put(a));
        accept.invoke(surface,reordered,1f);
        assertEquals(alpha,ids.id(1)); assertEquals(beta,ids.id(0));
        assertEquals("Alpha",provider.findFocus(android.view.accessibility.AccessibilityNodeInfo.FOCUS_ACCESSIBILITY).getText().toString());
        assertFalse(provider.performAction(beta,clear,null));
        assertNotNull(provider.findFocus(android.view.accessibility.AccessibilityNodeInfo.FOCUS_ACCESSIBILITY));
        accept.invoke(surface,new JSONObject().put("nodes",new org.json.JSONArray().put(b)),1f);
        assertNull(provider.createAccessibilityNodeInfo(alpha)); assertFalse(provider.performAction(alpha,focus,null));
        assertNull(provider.findFocus(android.view.accessibility.AccessibilityNodeInfo.FOCUS_ACCESSIBILITY));
        accept.invoke(surface,first,1f); assertNotEquals(alpha,ids.id(0));
        Method reset=surface.getClass().getDeclaredMethod("resetFocus"); reset.setAccessible(true); reset.invoke(surface);
        assertEquals(-1,ids.index(beta));
    }

    @Test public void touchExplorationUsesVisibleSemanticNodesAndDoesNotClick() throws Exception {
        java.util.ArrayList<Integer> events=new java.util.ArrayList<>();
        java.util.ArrayList<Runnable> tasks=new java.util.ArrayList<>();
        GpuView view=new GpuView(RuntimeEnvironment.getApplication(),"",new DeferredWorker() {
            @Override public void execute(Runnable task) { tasks.add(task); }
        },null);
        android.widget.FrameLayout parent=new android.widget.FrameLayout(view.getContext()) {
            @Override public boolean requestSendAccessibilityEvent(android.view.View child,android.view.accessibility.AccessibilityEvent event) {
                events.add(event.getEventType()); return true;
            }
        };
        parent.addView(view); android.view.View surface=view.getChildAt(0);
        java.lang.reflect.Field active=GpuView.class.getDeclaredField("active"); active.setAccessible(true); active.setBoolean(view,true);
        android.view.accessibility.AccessibilityManager manager=(android.view.accessibility.AccessibilityManager)view.getContext().getSystemService(android.content.Context.ACCESSIBILITY_SERVICE);
        org.robolectric.Shadows.shadowOf(manager).setEnabled(true);
        org.robolectric.Shadows.shadowOf(manager).setTouchExplorationEnabled(true);
        JSONObject a=new JSONObject("{identity:'a',tag:'button',text:'Alpha',on_click:1,bounds:[0,0,100,40]}"),
            b=new JSONObject("{identity:'b',tag:'text',text:'Beta',bounds:[0,50,100,40],clips:[{bounds:[0,50,100,40],radius:20}]}");
        publish(view,new JSONObject().put("nodes",new org.json.JSONArray().put(a).put(b))); tasks.clear();
        int enter=android.view.accessibility.AccessibilityEvent.TYPE_VIEW_HOVER_ENTER, exit=android.view.accessibility.AccessibilityEvent.TYPE_VIEW_HOVER_EXIT;
        assertTrue(hover(surface,android.view.MotionEvent.ACTION_HOVER_ENTER,30,20));
        assertTrue(hover(surface,android.view.MotionEvent.ACTION_HOVER_MOVE,40,20));
        assertEquals(java.util.Arrays.asList(enter),events);
        assertFalse(hover(surface,android.view.MotionEvent.ACTION_HOVER_MOVE,1,51));
        assertEquals(java.util.Arrays.asList(enter,exit),events);
        assertTrue(hover(surface,android.view.MotionEvent.ACTION_HOVER_MOVE,50,70));
        assertTrue(tasks.isEmpty());
        publish(view,new JSONObject().put("nodes",new org.json.JSONArray().put(b).put(a)));
        assertEquals(java.util.Arrays.asList(enter,exit,enter),events);
        publish(view,new JSONObject().put("nodes",new org.json.JSONArray().put(a)));
        assertEquals(java.util.Arrays.asList(enter,exit,enter,exit),events);
        assertTrue(hover(surface,android.view.MotionEvent.ACTION_HOVER_MOVE,30,20));
        view.pause(); assertEquals(java.util.Arrays.asList(enter,exit,enter,exit,enter,exit),events);
        events.clear(); org.robolectric.Shadows.shadowOf(manager).setTouchExplorationEnabled(false);
        active.setBoolean(view,true); hover(surface,android.view.MotionEvent.ACTION_HOVER_ENTER,30,20);
        assertTrue(events.isEmpty());
    }
    private boolean hover(android.view.View surface,int action,float x,float y) throws Exception {
        android.view.MotionEvent event=android.view.MotionEvent.obtain(0,0,action,x,y,0);
        Method dispatch=surface.getClass().getDeclaredMethod("dispatchHoverEvent",android.view.MotionEvent.class); dispatch.setAccessible(true);
        try { return (boolean)dispatch.invoke(surface,event); } finally { event.recycle(); }
    }

    private void publish(GpuView view,JSONObject frame) throws Exception {
        org.json.JSONArray nodes=frame.getJSONArray("nodes");
        for(int i=0;i<nodes.length();i++) if(!nodes.getJSONObject(i).has("identity")) nodes.getJSONObject(i).put("identity","fixture-"+i);
        android.view.View surface=view.getChildAt(0);
        Method accept=surface.getClass().getDeclaredMethod("acceptFrame",JSONObject.class,float.class); accept.setAccessible(true);
        accept.invoke(surface,frame,1f);
    }

    private void activate(GpuView view) throws Exception {
        java.lang.reflect.Field active=GpuView.class.getDeclaredField("active");active.setAccessible(true);active.setBoolean(view,true);
    }

    private JSONObject sample(GpuView view) throws Exception {
        Method method = GpuView.class.getDeclaredMethod("measureEnvironment", float.class);
        method.setAccessible(true);
        return new JSONObject((String) method.invoke(view, 1.0f));
    }

    @Test public void unchangedPhaseFramesPreservePressAndDeferClicksUntilPresentation() throws Exception {
        java.util.ArrayList<Runnable> tasks=new java.util.ArrayList<>();
        DeferredWorker worker=new DeferredWorker() { @Override public void execute(Runnable task) { tasks.add(task); } };
        GpuView view=new GpuView(RuntimeEnvironment.getApplication(),"",worker,null);
        android.view.View surface=view.getChildAt(0);
        int spec=android.view.View.MeasureSpec.makeMeasureSpec(200,android.view.View.MeasureSpec.EXACTLY);
        view.measure(spec,spec); view.layout(0,0,200,200);
        Method accept=surface.getClass().getDeclaredMethod("acceptFrame",JSONObject.class,float.class); accept.setAccessible(true);
        JSONObject first=new JSONObject("{generation:1,nodes:[{identity:'button',tag:'button',on_click:1,bounds:[0,0,100,100],visible_bounds:[0,0,100,100]}]}");
        accept.invoke(surface,first,1f);
        android.view.MotionEvent down=android.view.MotionEvent.obtain(0,0,android.view.MotionEvent.ACTION_DOWN,50,50,0);
        view.dispatchTouchEvent(down); down.recycle();
        java.lang.reflect.Field pressed=surface.getClass().getDeclaredField("pressed"), generation=surface.getClass().getDeclaredField("pressedGeneration");
        pressed.setAccessible(true); generation.setAccessible(true);
        JSONObject next=new JSONObject(first.toString()).put("generation",2);
        accept.invoke(surface,next,1f);
        assertEquals(0,pressed.getInt(surface)); assertEquals(2,generation.getLong(surface));
        for (String name : new String[]{"active","ready","busy"}) {
            java.lang.reflect.Field flag=GpuView.class.getDeclaredField(name); flag.setAccessible(true); flag.setBoolean(view,true);
        }
        Method click=GpuView.class.getDeclaredMethod("click",int.class), drain=GpuView.class.getDeclaredMethod("drainClick",boolean.class);
        click.setAccessible(true); drain.setAccessible(true);
        tasks.clear(); click.invoke(view,0); assertTrue(tasks.isEmpty());
        java.lang.reflect.Field busy=GpuView.class.getDeclaredField("busy"); busy.setAccessible(true); busy.setBoolean(view,false);
        drain.invoke(view,true); assertEquals(1,tasks.size());
        tasks.clear(); busy.setBoolean(view,true); click.invoke(view,0);
        JSONObject changed=new JSONObject(next.toString()).put("generation",3);
        changed.getJSONArray("nodes").getJSONObject(0).put("identity","replacement");
        accept.invoke(surface,changed,1f); assertEquals(-1,pressed.getInt(surface));
        busy.setBoolean(view,false); drain.invoke(view,true); assertTrue(tasks.isEmpty());
        view.dispose();
    }

    @Test public void feedbackLayerPassesTouchesAndReturnsAfterLoadErrors() throws Exception {
        GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "", new DeferredWorker(), null);
        android.view.View surface = view.getChildAt(0), feedback = view.getChildAt(1);
        assertFalse(feedback.isClickable());
        assertFalse(feedback.isFocusable());
        assertEquals(android.view.View.IMPORTANT_FOR_ACCESSIBILITY_NO, feedback.getImportantForAccessibility());
        java.lang.reflect.Field snapshot = GpuView.class.getDeclaredField("snapshot");
        snapshot.setAccessible(true);
        publish(view, new JSONObject("{generation:1,nodes:[{tag:'button',on_click:1,bounds:[0,0,100,100],visible_bounds:[0,0,100,100]}]}"));
        java.lang.reflect.Field scale = GpuView.class.getDeclaredField("frameScale");
        scale.setAccessible(true); scale.setFloat(view, 1);
        int spec = android.view.View.MeasureSpec.makeMeasureSpec(200, android.view.View.MeasureSpec.EXACTLY);
        view.measure(spec,spec); view.layout(0,0,200,200);
        android.view.MotionEvent down = android.view.MotionEvent.obtain(0,0,android.view.MotionEvent.ACTION_DOWN,50,50,0);
        assertTrue(view.dispatchTouchEvent(down)); down.recycle();
        java.lang.reflect.Field pressed = surface.getClass().getDeclaredField("pressed");
        pressed.setAccessible(true); assertEquals(0,pressed.getInt(surface));
        android.view.MotionEvent cancel = android.view.MotionEvent.obtain(0,1,android.view.MotionEvent.ACTION_CANCEL,50,50,0);
        view.dispatchTouchEvent(cancel); cancel.recycle();
        assertEquals(-1,pressed.getInt(surface));
        snapshot.set(view,null);
        Method error = GpuView.class.getDeclaredMethod("showError",Throwable.class);
        error.setAccessible(true); error.invoke(view,new IllegalStateException("test load failure"));
        assertEquals(3,view.getChildCount());
        assertSame(surface,view.getChildAt(0)); assertSame(feedback,view.getChildAt(1));
        view.reload();
        assertEquals(2,view.getChildCount());
        assertSame(surface,view.getChildAt(0)); assertSame(feedback,view.getChildAt(1));
        view.dispose();
    }

    @Test public void keyboardEntryAndReverseTraversalSkipUnavailableButtons() throws Exception {
        GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "", new DeferredWorker(), null);
        activate(view);
        android.view.View surface = view.getChildAt(0);
        assertFalse(surface.getDefaultFocusHighlightEnabled());
        java.lang.reflect.Field snapshot = GpuView.class.getDeclaredField("snapshot");
        snapshot.setAccessible(true);
        publish(view,new JSONObject("{nodes:["
            + "{on_click:1,bounds:[0,0,100,40]},"
            + "{on_click:2,bounds:[0,40,100,40],visible_bounds:[0,40,100,0]},"
            + "{on_click:0,bounds:[0,80,100,40]},"
            + "{on_click:3,bounds:[0,120,100,40]}]}"));
        java.lang.reflect.Field scale = GpuView.class.getDeclaredField("frameScale");
        scale.setAccessible(true); scale.setFloat(view,1);
        java.lang.reflect.Field focus = surface.getClass().getDeclaredField("keyboardFocus");
        focus.setAccessible(true);
        Method enter = surface.getClass().getDeclaredMethod("onFocusChanged",boolean.class,int.class,android.graphics.Rect.class);
        enter.setAccessible(true);
        enter.invoke(surface,true,android.view.View.FOCUS_FORWARD,null);
        assertEquals(0,focus.getInt(surface));
        assertTrue(surface.getAccessibilityNodeProvider().createAccessibilityNodeInfo(1).isFocused());
        Method move = surface.getClass().getDeclaredMethod("moveKeyboardFocus",int.class);
        move.setAccessible(true);
        assertEquals(true,move.invoke(surface,1)); assertEquals(3,focus.getInt(surface));
        assertEquals(true,move.invoke(surface,1)); assertEquals(0,focus.getInt(surface));
        enter.invoke(surface,false,android.view.View.FOCUS_FORWARD,null);
        assertEquals(-1,focus.getInt(surface));
        enter.invoke(surface,true,android.view.View.FOCUS_BACKWARD,null);
        assertEquals(3,focus.getInt(surface));
        view.dispose();
    }

    @Test public void accessibilityRevealsButtonsAndTextWithoutActivatingThem() throws Exception {
        for(String kind:new String[]{"button","text"}) {
            GpuView view=new GpuView(RuntimeEnvironment.getApplication(),"",new DeferredWorker(),null);
            activate(view);android.view.View surface=view.getChildAt(0);
            JSONObject frame=new JSONObject("{nodes:["
                +"{identity:'scroll',bounds:[0,0,100,100],visible_bounds:[0,0,100,100],scroll:{id:'s',parent:null,offset:0,max_offset:200}},"
                +"{identity:'target',tag:'"+kind+"',text:'Target',on_click:0,bounds:[0,160,80,40],visible_bounds:[0,160,0,0],scroll_parent:'s'}]}");
            publish(view,frame);
            java.lang.reflect.Field mapping=GpuView.class.getDeclaredField("accessibilityIds");mapping.setAccessible(true);
            int id=((GpuAccessibilityIds)mapping.get(view)).id(1);
            android.view.accessibility.AccessibilityNodeProvider provider=surface.getAccessibilityNodeProvider();
            android.view.accessibility.AccessibilityNodeInfo info=provider.createAccessibilityNodeInfo(id);
            android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction show=android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN;
            assertTrue(info.getActionList().contains(show));info.recycle();
            assertTrue(provider.performAction(id,show.getId(),null));
            java.lang.reflect.Field scroll=GpuView.class.getDeclaredField("scrolling");scroll.setAccessible(true);
            assertEquals(100,((GpuScroll)scroll.get(view)).offset("s"),0);
            assertNull(provider.findFocus(android.view.accessibility.AccessibilityNodeInfo.FOCUS_INPUT));
            assertFalse(provider.performAction(id,android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK,null));
            publish(view,new JSONObject("{nodes:[]}"));assertFalse(provider.performAction(id,show.getId(),null));
            view.dispose();
        }
    }

    @Test public void keyboardTraversalSkipsHorizontallyUnreachableScrollChildren() throws Exception {
        GpuView view=new GpuView(RuntimeEnvironment.getApplication(),"",new DeferredWorker(),null);
        activate(view);android.view.View surface=view.getChildAt(0);
        publish(view,new JSONObject("{nodes:["
            +"{identity:'scroll',bounds:[0,0,100,100],visible_bounds:[0,0,100,100],scroll:{id:'s',parent:null,offset:0,max_offset:200}},"
            +"{identity:'first',on_click:1,bounds:[0,0,80,40]},"
            +"{identity:'outside',on_click:2,bounds:[120,160,80,40],visible_bounds:[120,160,0,0],scroll_parent:'s'},"
            +"{identity:'last',on_click:3,bounds:[0,50,80,40]}]}"));
        Method move=surface.getClass().getDeclaredMethod("moveKeyboardFocus",int.class);move.setAccessible(true);
        java.lang.reflect.Field focus=surface.getClass().getDeclaredField("keyboardFocus");focus.setAccessible(true);
        assertEquals(true,move.invoke(surface,1));assertEquals(1,focus.getInt(surface));
        assertEquals(true,move.invoke(surface,1));assertEquals(3,focus.getInt(surface));
        assertEquals(true,move.invoke(surface,-1));assertEquals(1,focus.getInt(surface));
        view.dispose();
    }

    @Test public void presentedFramesRetainIdentityAndClearUnavailableTargets() throws Exception {
        GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "", new DeferredWorker(), null);
        activate(view);
        android.view.View surface = view.getChildAt(0);
        Method accept = surface.getClass().getDeclaredMethod("acceptFrame",JSONObject.class,float.class);
        accept.setAccessible(true);
        Method move = surface.getClass().getDeclaredMethod("moveKeyboardFocus",int.class);
        move.setAccessible(true);
        java.lang.reflect.Field focus = surface.getClass().getDeclaredField("keyboardFocus");
        focus.setAccessible(true);
        accept.invoke(surface,new JSONObject("{nodes:[{identity:'a',on_click:1,bounds:[0,0,100,40]}]}"),1f);
        move.invoke(surface,1);
        accept.invoke(surface,new JSONObject("{nodes:[{identity:'b',on_click:2,bounds:[0,0,100,40]},{identity:'a',on_click:3,bounds:[0,40,100,40]}]}"),2f);
        assertEquals(1,focus.getInt(surface));
        view.onConfigurationChanged(new Configuration(view.getResources().getConfiguration()));
        assertEquals(1,focus.getInt(surface));
        java.lang.reflect.Field overlay = surface.getClass().getDeclaredField("focusOverlay");
        overlay.setAccessible(true);
        assertEquals(new android.graphics.Rect(0,80,200,160),((GpuClip.Highlight)overlay.get(surface)).getBounds());
        accept.invoke(surface,new JSONObject("{nodes:[{identity:'a',on_click:0,bounds:[0,0,100,40]}]}"),1f);
        assertEquals(-1,focus.getInt(surface));
        assertFalse(surface.onKeyDown(android.view.KeyEvent.KEYCODE_ENTER,new android.view.KeyEvent(0,android.view.KeyEvent.KEYCODE_ENTER)));
        accept.invoke(surface,new JSONObject("{nodes:[{identity:'a',on_click:4,bounds:[0,0,100,40]}]}"),1f);
        assertEquals(-1,focus.getInt(surface));
        move.invoke(surface,1);
        accept.invoke(surface,new JSONObject("{nodes:[{identity:'a',on_click:4,bounds:[0,0,100,40],visible_bounds:[0,0,100,0]}]}"),1f);
        assertEquals(-1,focus.getInt(surface));
        accept.invoke(surface,new JSONObject("{nodes:[{identity:'a',on_click:4,bounds:[0,0,100,40]}]}"),1f);
        move.invoke(surface,1);
        accept.invoke(surface,new JSONObject("{nodes:[{identity:'b',on_click:4,bounds:[0,0,100,40]}]}"),1f);
        assertEquals(-1,focus.getInt(surface));
        view.dispose();
    }

    @Test public void sliderQueueCoalescesAndPausedWorkerCannotDeliverStaleValues() throws Exception {
        java.util.ArrayList<Runnable> tasks=new java.util.ArrayList<>();
        GpuView view=new GpuView(RuntimeEnvironment.getApplication(),"",new DeferredWorker(){@Override public void execute(Runnable task){tasks.add(task);}},null);
        activate(view);java.lang.reflect.Field ready=GpuView.class.getDeclaredField("ready");ready.setAccessible(true);ready.setBoolean(view,true);
        JSONObject node=GpuSliderTest.node();publish(view,GpuSliderTest.frame(node));tasks.clear();
        Method propose=GpuView.class.getDeclaredMethod("proposeNumber",JSONObject.class,double.class);propose.setAccessible(true);
        java.lang.reflect.Field state=GpuView.class.getDeclaredField("sliders");state.setAccessible(true);GpuSlider slider=(GpuSlider)state.get(view);
        assertEquals(true,propose.invoke(view,node,6.0));assertEquals(1,tasks.size());Runnable queued=tasks.get(0);
        assertEquals(true,propose.invoke(view,node,9.0));assertEquals(true,propose.invoke(view,node,10.0));assertEquals(1,tasks.size());assertEquals(10,slider.pending.value,0);
        view.pause();assertNull(slider.pending);queued.run();org.robolectric.Shadows.shadowOf(android.os.Looper.getMainLooper()).idle();
        java.lang.reflect.Field busy=GpuView.class.getDeclaredField("numberBusy");busy.setAccessible(true);assertFalse(busy.getBoolean(view));
        assertFalse((boolean)propose.invoke(view,node,9.0));view.dispose();
    }

    @Test public void pendingKeyboardSelectionCannotActivateRemovedDisabledOrPausedTargets() throws Exception {
        java.util.ArrayList<Runnable> tasks=new java.util.ArrayList<>();
        GpuView view=new GpuView(RuntimeEnvironment.getApplication(),"",new DeferredWorker(){@Override public void execute(Runnable task){tasks.add(task);}},null);
        activate(view);android.view.View surface=view.getChildAt(0);
        java.lang.reflect.Field busy=GpuView.class.getDeclaredField("busy"),pending=GpuView.class.getDeclaredField("pending");busy.setAccessible(true);pending.setAccessible(true);
        java.lang.reflect.Field identity=surface.getClass().getDeclaredField("pendingKeyboardIdentity");identity.setAccessible(true);
        Method navigate=surface.getClass().getDeclaredMethod("navigateKeyboard",int.class,boolean.class),finish=surface.getClass().getDeclaredMethod("finishKeyboardNavigation");navigate.setAccessible(true);finish.setAccessible(true);
        JSONObject frame=new JSONObject("{nodes:[{identity:'option',tag:'radio-option',radio_group:'g',on_click:1,bounds:[0,0,100,44]}]}");
        publish(view,frame);tasks.clear();busy.setBoolean(view,true);navigate.invoke(surface,0,true);assertEquals("option",identity.get(surface));
        publish(view,new JSONObject("{nodes:[]}"));busy.setBoolean(view,false);pending.setBoolean(view,false);finish.invoke(surface);assertNull(identity.get(surface));assertTrue(tasks.isEmpty());
        publish(view,frame);busy.setBoolean(view,true);navigate.invoke(surface,0,true);publish(view,new JSONObject("{nodes:[{identity:'option',tag:'radio-option',radio_group:'g',bounds:[0,0,100,44]}]}"));busy.setBoolean(view,false);pending.setBoolean(view,false);finish.invoke(surface);assertNull(identity.get(surface));assertTrue(tasks.isEmpty());
        publish(view,frame);busy.setBoolean(view,true);navigate.invoke(surface,0,true);
        java.lang.reflect.Field page=GpuView.class.getDeclaredField("inputPageGeneration");page.setAccessible(true);page.setLong(view,page.getLong(view)+1);busy.setBoolean(view,false);pending.setBoolean(view,false);finish.invoke(surface);assertNull(identity.get(surface));assertTrue(tasks.isEmpty());
        busy.setBoolean(view,true);navigate.invoke(surface,0,true);view.onWindowFocusChanged(false);assertNull(identity.get(surface));assertTrue(tasks.isEmpty());
        navigate.invoke(surface,0,true);view.pause();assertNull(identity.get(surface));
        assertEquals("pause enqueues only timeline parking",1,tasks.size());tasks.remove(0).run();assertTrue(tasks.isEmpty());view.dispose();
    }

    @Test public void activityConfigurationAndHostOverridesSurviveModuleReload() throws Exception {
        GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "", new DeferredWorker(), null);
        Configuration configuration = new Configuration(view.getResources().getConfiguration());
        configuration.uiMode = Configuration.UI_MODE_NIGHT_YES;
        view.onConfigurationChanged(configuration);
        configuration.uiMode = Configuration.UI_MODE_NIGHT_NO;
        assertEquals("dark", sample(view).getString("theme"));
        view.updateEnvironment("{kind:'language',revision:'1',language:'zh-CN'}");
        view.updateEnvironment("{kind:'surface',revision:'1',context:{sizeClass:'regular',aside:true}}");
        view.reload("new module");
        JSONObject retained = sample(view);
        assertEquals("dark", retained.getString("theme"));
        assertEquals("zh-CN", retained.getString("displayLanguage"));
        assertEquals("regular", retained.getString("sizeClass"));
        assertTrue(retained.getBoolean("aside"));
        view.onConfigurationChanged(configuration);
        assertEquals("light", sample(view).getString("theme"));
        view.dispose();
    }

    @Test public void queuedInputCannotTargetAReplacementPage() throws Exception {
        java.util.ArrayList<Runnable> tasks=new java.util.ArrayList<>();
        DeferredWorker worker=new DeferredWorker() { @Override public void execute(Runnable task) { tasks.add(task); } };
        int[] errors={0};
        GpuView view=new GpuView(RuntimeEnvironment.getApplication(),"",worker,new GpuView.Listener() {
            public void onLoaded() { }
            public void onFrame(String metadata) { }
            public void onError(Throwable error) { errors[0]++; }
        });
        java.lang.reflect.Field input=GpuView.class.getDeclaredField("ime"); input.setAccessible(true);
        ((GpuIme)input.get(view)).focus(new JSONObject("{identity:'editor',tag:'input',input:{enabled:true}}"),1);
        Runnable stale=tasks.get(1);
        view.reload("replacement");
        java.lang.reflect.Field page=GpuView.class.getDeclaredField("nativePage"); page.setAccessible(true); page.setLong(view,99);
        stale.run();
        org.robolectric.Shadows.shadowOf(android.os.Looper.getMainLooper()).idle();
        assertEquals(0,errors[0]);
        ((GpuIme)input.get(view)).focus(new JSONObject("{identity:'editor',tag:'input',input:{enabled:true}}"),1);
        tasks.get(tasks.size()-1).run(); // JNI is absent in this JVM; delivery is queued on main.
        view.reload("third page");
        org.robolectric.Shadows.shadowOf(android.os.Looper.getMainLooper()).idle();
        assertEquals(0,errors[0]);
        page.setLong(view,0); view.dispose();
    }

    @Test public void accessibilityFocusQueryTracksPresentedEditorAndInactiveActionsFail() throws Exception {
        GpuView view=new GpuView(RuntimeEnvironment.getApplication(),"",new DeferredWorker(),null);
        java.lang.reflect.Field snapshot=GpuView.class.getDeclaredField("snapshot"); snapshot.setAccessible(true);
        publish(view,new JSONObject("{generation:12,nodes:[{identity:'editor',tag:'input',text:'',input:{enabled:true,edit_token:7,edit:{text:'hello',selection:[1,4]}},bounds:[0,0,100,40],visible_bounds:[0,0,100,40]}]}"));
        java.lang.reflect.Field scale=GpuView.class.getDeclaredField("frameScale"); scale.setAccessible(true); scale.setFloat(view,1);
        android.view.accessibility.AccessibilityNodeProvider provider=view.getChildAt(0).getAccessibilityNodeProvider();
        assertNull(provider.findFocus(android.view.accessibility.AccessibilityNodeInfo.FOCUS_INPUT));
        assertFalse(provider.performAction(1,android.view.accessibility.AccessibilityNodeInfo.ACTION_SET_TEXT,null));
        java.lang.reflect.Field active=GpuView.class.getDeclaredField("active"); active.setAccessible(true); active.setBoolean(view,true);
        android.view.accessibility.AccessibilityNodeInfo info=provider.findFocus(android.view.accessibility.AccessibilityNodeInfo.FOCUS_INPUT);
        assertEquals("hello",info.getText().toString()); assertEquals(1,info.getTextSelectionStart()); assertEquals(4,info.getTextSelectionEnd());
        if (android.os.Build.VERSION.SDK_INT >= 24) assertTrue(info.isImportantForAccessibility());
        view.dispose();
    }

    @Test public void emptyInputsExposeFocusTargetsAndDisabledInputsCannotBeHit() throws Exception {
        GpuView view=new GpuView(RuntimeEnvironment.getApplication(),"",new DeferredWorker(),null);
        java.lang.reflect.Field snapshot=GpuView.class.getDeclaredField("snapshot"); snapshot.setAccessible(true);
        publish(view,new JSONObject("{nodes:[{tag:'input',text:'',input:{enabled:true,secure:true},bounds:[0,0,100,40],visible_bounds:[0,0,100,40]},"
            + "{tag:'input',text:'',input:{enabled:false},bounds:[0,50,100,40],visible_bounds:[0,50,100,40]}]}"));
        java.lang.reflect.Field scale=GpuView.class.getDeclaredField("frameScale"); scale.setAccessible(true); scale.setFloat(view,1);
        android.view.View surface=view.getChildAt(0);
        Method hit=surface.getClass().getDeclaredMethod("hit",float.class,float.class); hit.setAccessible(true);
        assertEquals(0,hit.invoke(surface,20f,20f)); assertEquals(-1,hit.invoke(surface,20f,70f));
        android.view.accessibility.AccessibilityNodeProvider provider=surface.getAccessibilityNodeProvider();
        assertEquals(2,provider.createAccessibilityNodeInfo(android.view.View.NO_ID).getChildCount());
        assertEquals("android.widget.EditText",provider.createAccessibilityNodeInfo(1).getClassName());
        assertTrue(provider.createAccessibilityNodeInfo(1).isPassword());
        assertTrue(provider.createAccessibilityNodeInfo(1).isClickable());
        assertFalse(provider.createAccessibilityNodeInfo(2).isEnabled());
        view.dispose();
    }

    @Test public void clippedButtonsCannotBeHitOrExposedAsVisibleAccessibilityChildren() throws Exception {
        GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "", new DeferredWorker(), null);
        java.lang.reflect.Field snapshot = GpuView.class.getDeclaredField("snapshot");
        snapshot.setAccessible(true);
        publish(view, new JSONObject("{nodes:["
            + "{tag:'button',text:'partial',on_click:1,bounds:[0,0,100,100],visible_bounds:[0,50,100,50],clips:[{bounds:[0,50,100,50],radius:25}]},"
            + "{tag:'button',text:'hidden',on_click:2,bounds:[0,100,100,100],visible_bounds:[0,100,100,0]},"
            + "{tag:'image',text:'',test_id:'avatar',bounds:[0,0,40,40],visible_bounds:[0,0,40,40]}]}"));
        java.lang.reflect.Field scale = GpuView.class.getDeclaredField("frameScale");
        scale.setAccessible(true);
        scale.setFloat(view, 1.0f);
        android.view.View surface = view.getChildAt(0);
        Method hit = surface.getClass().getDeclaredMethod("hit", float.class, float.class);
        hit.setAccessible(true);
        assertEquals(-1, hit.invoke(surface, 20.0f, 20.0f));
        assertEquals(0, hit.invoke(surface, 20.0f, 75.0f));
        assertEquals(-1, hit.invoke(surface, 1.0f, 51.0f));
        assertEquals(-1, hit.invoke(surface, 20.0f, 150.0f));
        android.view.accessibility.AccessibilityNodeProvider provider = surface.getAccessibilityNodeProvider();
        assertEquals(2, provider.createAccessibilityNodeInfo(android.view.View.NO_ID).getChildCount());
        assertEquals("android.widget.ImageView", provider.createAccessibilityNodeInfo(3).getClassName());
        assertTrue(provider.createAccessibilityNodeInfo(3).getViewIdResourceName().endsWith(":id/avatar"));
        assertFalse(provider.createAccessibilityNodeInfo(2).isVisibleToUser());
        assertFalse(provider.performAction(2, android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK, null));
        view.dispose();
    }
    @Test public void composedButtonHitsAllContentButExposesOnlyOneAccessibleControl() throws Exception {
        GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "", new DeferredWorker(), null);
        publish(view, new JSONObject("{nodes:["
            + "{tag:'button',identity:'action',text:'Open inbox',accessibility_label:'Open inbox',on_click:1,bounds:[0,0,200,100],visible_bounds:[0,0,200,100]},"
            + "{tag:'text',identity:'title',text:'Inbox',accessibility_hidden:true,bounds:[16,16,120,24],visible_bounds:[16,16,120,24]},"
            + "{tag:'image',identity:'icon',accessibility_hidden:true,bounds:[160,20,24,24],visible_bounds:[160,20,24,24]},"
            + "{tag:'text',identity:'description',text:'3 unread',accessibility_hidden:true,bounds:[16,50,160,24],visible_bounds:[16,50,160,24]}]}"));
        java.lang.reflect.Field scale = GpuView.class.getDeclaredField("frameScale"); scale.setAccessible(true); scale.setFloat(view,1f);
        android.view.View surface=view.getChildAt(0);
        Method hit=surface.getClass().getDeclaredMethod("hit",float.class,float.class);hit.setAccessible(true);
        assertEquals(0,hit.invoke(surface,24f,60f));
        assertEquals(0,hit.invoke(surface,170f,30f));
        assertEquals(0,hit.invoke(surface,195f,95f));
        android.view.accessibility.AccessibilityNodeProvider provider=surface.getAccessibilityNodeProvider();
        assertEquals(1,provider.createAccessibilityNodeInfo(android.view.View.NO_ID).getChildCount());
        assertEquals("Open inbox",provider.createAccessibilityNodeInfo(1).getText());
        assertEquals("android.widget.Button",provider.createAccessibilityNodeInfo(1).getClassName());
        for(int id:new int[]{2,3,4}){
            assertNull(provider.createAccessibilityNodeInfo(id));
            assertFalse(provider.performAction(id,android.view.accessibility.AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS,null));
        }
        view.dispose();
    }

}
