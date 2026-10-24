package com.lingxia.ui;

import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 28)
public class GpuEnvironmentTest {
    private JSONObject language(String revision, String language) throws Exception {
        return new JSONObject().put("kind", "language").put("revision", revision).put("language", language);
    }

    @Test public void revisionsAreIndependentAndLossless() throws Exception {
        GpuEnvironment env = new GpuEnvironment();
        assertTrue(env.update(language("18446744073709551614", "zh-CN")));
        assertTrue(env.update(language("18446744073709551615", "en")));
        assertFalse(env.update(language("18446744073709551614", "fr")));
        assertFalse(env.update(language("18446744073709551615", "fr")));
        assertTrue(env.update(new JSONObject("{kind:'surface',revision:'0',context:{sizeClass:'regular',aside:true,width:999}}")));
        JSONObject result = env.apply(new JSONObject().put("viewport", new JSONObject().put("width", 300)));
        assertEquals("en", result.getString("displayLanguage"));
        assertEquals("regular", result.getString("sizeClass"));
        assertTrue(result.getBoolean("aside"));
        assertEquals(300, result.getJSONObject("viewport").getInt("width"));
        assertFalse(result.has("width"));
    }

    @Test public void invalidUpdatesDoNotConsumeRevisionOrPartiallyApply() throws Exception {
        GpuEnvironment env = new GpuEnvironment();
        assertTrue(env.update(new JSONObject("{kind:'surface',revision:'1',context:{sizeClass:'compact',aside:false}}")));
        try {
            env.update(new JSONObject("{kind:'surface',revision:'2',context:{sizeClass:'regular',aside:'true'}}"));
            fail("must reject a string boolean");
        } catch (IllegalArgumentException expected) { }
        assertEquals("compact", env.apply(new JSONObject()).getString("sizeClass"));
        assertTrue(env.update(new JSONObject("{kind:'surface',revision:'2',context:{sizeClass:'regular',aside:true}}")));
        try { env.update(language("18446744073709551616", "fr")); fail("overflow"); }
        catch (IllegalArgumentException expected) { }
        assertTrue(env.update(language("0", "en")));
        assertEquals("en", env.apply(new JSONObject()).getString("displayLanguage"));
    }
}
