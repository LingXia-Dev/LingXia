package com.lingxia.ui;

import android.content.res.Configuration;
import android.content.res.Resources;
import org.json.JSONArray;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.annotation.Config;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 28)
public class GpuTextScaleTest {
    @Test @Config(sdk = 34)
    public void android14PreservesNonlinearPlatformScaling() throws Exception {
        android.content.Context context = RuntimeEnvironment.getApplication();
        Configuration configuration = new Configuration(context.getResources().getConfiguration());
        configuration.fontScale = 2.0f;
        JSONArray points = GpuTextScale.measure(context.createConfigurationContext(configuration).getResources());
        double bodyRatio = points.getJSONArray(14).getDouble(1) / 14.0;
        double displayRatio = points.getJSONArray(100).getDouble(1) / 100.0;
        assertTrue(bodyRatio > 1.0);
        assertTrue(displayRatio < bodyRatio);
    }

    @Test public void systemTextPreferenceIsIndependentOfDisplayDensity() throws Exception {
        android.content.Context context = RuntimeEnvironment.getApplication();
        Configuration configuration = new Configuration(context.getResources().getConfiguration());
        configuration.fontScale = 1.5f;
        configuration.densityDpi = 320;
        Resources resources = context.createConfigurationContext(configuration).getResources();
        JSONArray points = GpuTextScale.measure(resources);
        assertEquals(263, points.length());
        assertEquals(21.0, points.getJSONArray(14).getDouble(1), 0.01);
        assertEquals(16384, points.getJSONArray(262).getInt(0));
        configuration.densityDpi = 480;
        JSONArray denser = GpuTextScale.measure(context.createConfigurationContext(configuration).getResources());
        assertEquals(points.getJSONArray(14).getDouble(1), denser.getJSONArray(14).getDouble(1), 0.01);
    }
}
