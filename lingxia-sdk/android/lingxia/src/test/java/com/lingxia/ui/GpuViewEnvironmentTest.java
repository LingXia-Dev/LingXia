package com.lingxia.ui;

import android.content.res.Configuration;
import org.json.JSONObject;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.annotation.Config;
import java.lang.reflect.Method;
import static org.junit.Assert.*;

/** Android mechanics around the shared core: environment sampling, surfaces, gestures and teardown. */
@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 28)
public class GpuViewEnvironmentTest {
    @Before public void loadCore() { GpuNative.load(); }

    private JSONObject sample(GpuView view) throws Exception {
        Method method = GpuView.class.getDeclaredMethod("measureEnvironment", float.class);
        method.setAccessible(true);
        return new JSONObject((String) method.invoke(view, 1.0f));
    }

    @Test public void retainedPageResamplesFontsWithoutAViewConfigurationCallback() throws Exception {
        android.content.Context base = RuntimeEnvironment.getApplication();
        Configuration normal = new Configuration(base.getResources().getConfiguration());
        normal.fontScale = 1.0f;
        Configuration large = new Configuration(normal); large.fontScale = 2.0f;
        android.content.Context context = base.createConfigurationContext(normal);
        GpuView view = new GpuView(context, "", "test:environment", null);
        JSONObject first = sample(view);
        android.util.DisplayMetrics metrics = new android.util.DisplayMetrics();
        metrics.setTo(context.getResources().getDisplayMetrics());
        metrics.scaledDensity = metrics.density * large.fontScale;
        context.getResources().updateConfiguration(large, metrics);
        JSONObject enlarged = sample(view);
        assertEquals(14.0, first.getJSONArray("textScale").getJSONArray(14).getDouble(1), 0.01);
        assertEquals(28.0, enlarged.getJSONArray("textScale").getJSONArray(14).getDouble(1), 0.01);
        view.dispose();
    }

    @Test public void interruptedRefreshGestureIsCanceledOnce() throws Exception {
        for (int interruption = 0; interruption < 3; interruption++) {
            GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "", "test:environment", null);
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
            view.dispose();
        }
    }

    @Test public void retainedPageVisibilityHidesItsSurfaceAndResumeRestoresIt() throws Exception {
        android.app.Activity activity = org.robolectric.Robolectric.buildActivity(android.app.Activity.class).setup().get();
        android.widget.FrameLayout parent = new android.widget.FrameLayout(activity);
        GpuView view = new GpuView(activity, "", "test:environment", null);
        parent.addView(view); activity.setContentView(parent);
        android.view.View surface = view.getChildAt(0);
        view.resume(); assertEquals(android.view.View.VISIBLE, surface.getVisibility());
        parent.setVisibility(android.view.View.GONE);
        assertEquals(android.view.View.INVISIBLE, surface.getVisibility());
        parent.setVisibility(android.view.View.VISIBLE);
        assertEquals(android.view.View.VISIBLE, surface.getVisibility());
        view.pause(); assertEquals(android.view.View.INVISIBLE, surface.getVisibility());
        parent.setVisibility(android.view.View.GONE); parent.setVisibility(android.view.View.VISIBLE);
        assertEquals(android.view.View.INVISIBLE, surface.getVisibility());
        view.resume(); assertEquals(android.view.View.VISIBLE, surface.getVisibility());
        view.dispose();
    }

    @Test public void touchesWithoutAPresentedFrameReachTheHostAndTheFeedbackLayerStaysInert() throws Exception {
        GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "", "test:environment", null);
        android.view.View surface = view.getChildAt(0), feedback = view.getChildAt(1);
        assertFalse(feedback.isClickable());
        assertFalse(feedback.isFocusable());
        assertEquals(android.view.View.IMPORTANT_FOR_ACCESSIBILITY_NO, feedback.getImportantForAccessibility());
        int spec = android.view.View.MeasureSpec.makeMeasureSpec(200, android.view.View.MeasureSpec.EXACTLY);
        view.measure(spec, spec); view.layout(0, 0, 200, 200);
        android.view.MotionEvent down = android.view.MotionEvent.obtain(0, 0, android.view.MotionEvent.ACTION_DOWN, 50, 50, 0);
        assertFalse("nothing is presented, so the gesture is the host's", surface.onTouchEvent(down));
        down.recycle();
        view.dispose();
        assertFalse("disposal leaves the core inert", view.canScrollVertically(1));
    }

    @Test public void activityConfigurationAndHostOverridesSurviveModuleReload() throws Exception {
        GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "", "test:environment", null);
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

    @Test public void inputRepliesFromAReplacedPageNeverReachTheEditor() throws Exception {
        int[] errors = {0};
        GpuView view = new GpuView(RuntimeEnvironment.getApplication(), GpuNative.module("function render(){return h('text',{},'a')}"),
            "test:environment", new GpuView.Listener() {
                public void onLoaded() { }
                public void onFrame(String metadata) { }
                public void onError(Throwable error) { errors[0]++; }
            });
        GpuNative.settle();
        java.lang.reflect.Field input = GpuView.class.getDeclaredField("ime"); input.setAccessible(true);
        GpuIme ime = (GpuIme)input.get(view);
        ime.focus(new JSONObject("{identity:'editor',tag:'input',input:{enabled:true}}"), 1);
        view.reload(GpuNative.module("function render(){return h('text',{},'b')}"));
        GpuNative.settle();
        assertFalse("the replaced page's focus reply was dropped", ime.hasFocus());
        java.lang.reflect.Field replies = GpuView.class.getDeclaredField("inputReplies"); replies.setAccessible(true);
        assertTrue(((java.util.Map<?, ?>)replies.get(view)).isEmpty());
        assertEquals(0, errors[0]);
        view.dispose();
    }
}
