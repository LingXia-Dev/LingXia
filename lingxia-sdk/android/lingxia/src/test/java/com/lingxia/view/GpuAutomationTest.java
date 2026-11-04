package com.lingxia.view;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import org.json.JSONObject;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;

/** Locator queries and actions reach the core through the host, with the core's error text. */
@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 28)
public class GpuAutomationTest {
    @Before public void load() { GpuNative.load(); }

    private static JSONObject automate(long host, String request) throws Exception {
        return new JSONObject(GpuView.nativeAutomate(host, request));
    }

    /** An act names a call; poll `result` until the worker applied it and the frame settled. */
    private static JSONObject act(long host, GpuNativeLoadTest.Fake fake, String request) throws Exception {
        JSONObject reply = automate(host, request);
        assertTrue(reply.toString(), reply.has("value"));
        String result = "{\"op\":\"result\",\"call\":" + reply.getJSONObject("value").getLong("call") + "}";
        JSONObject[] done = {null};
        fake.pump(host, () -> {
            try { JSONObject r = automate(host, result).getJSONObject("value"); if (r.optBoolean("done")) done[0] = r; return done[0] != null; }
            catch (Exception e) { throw new AssertionError(e); }
        });
        return done[0];
    }

    private static String text(GpuNativeLoadTest.Fake fake, String id) throws Exception {
        org.json.JSONArray nodes = new JSONObject(fake.frames.get(fake.frames.size() - 1)).getJSONArray("nodes");
        for (int i = 0; i < nodes.length(); i++)
            if (id.equals(nodes.getJSONObject(i).optString("test_id"))) return nodes.getJSONObject(i).optString("text");
        return null;
    }

    @Test public void queriesAndActsOnThePresentedFrame() throws Exception {
        GpuNativeLoadTest.Fake fake = new GpuNativeLoadTest.Fake();
        long host = GpuView.nativeHostCreate(fake, "test:jvm-automation", false, 0);
        try {
            GpuView.nativeLoad(host, GpuNative.module(
                "function render(){const n=useLocal('n',0),v=useLocal('v','');return h('column',{},"
                + "h('button',{'data-testid':'add',onClick:()=>n.set(n.value+1)},'Add'),"
                + "h('button',{'data-testid':'off',disabled:true},'Off'),"
                + "h('input',{'data-testid':'name',bind:v,'aria-label':'Name'}),"
                + "h('text',{'data-testid':'count'},'Count '+n.value+' '+v.value))}"));
            GpuView.nativeSetVisible(host, true);
            fake.pump(host, () -> fake.presented >= 1);
            JSONObject query = automate(host, "{\"op\":\"query\",\"css\":\"[data-testid=\\\"add\\\"]\"}");
            assertTrue(query.toString(), query.has("value"));
            assertEquals(JSONObject.NULL, act(host, fake, "{\"op\":\"act\",\"css\":\"[data-testid=\\\"add\\\"]\",\"action\":{\"kind\":\"click\"}}").get("value"));
            JSONObject fill = act(host, fake, "{\"op\":\"act\",\"css\":\"[data-testid=\\\"name\\\"]\",\"action\":{\"kind\":\"fill\",\"text\":\"Ada\"}}");
            assertEquals(JSONObject.NULL, fill.get("value"));
            // Fill goes through the platform's text-edit path, as assistive input does.
            assertTrue(fake.events.toString(), fake.events.contains("edit:Ada"));
            fake.pump(host, () -> { try { return "Count 1 ".equals(text(fake, "count")); } catch (Exception e) { throw new AssertionError(e); } });
            JSONObject press = act(host, fake, "{\"op\":\"act\",\"css\":\"[data-testid=\\\"add\\\"]\",\"action\":{\"kind\":\"press\",\"key\":\"Tab\"}}");
            assertTrue(press.toString(), press.has("value"));
            String disabled = automate(host, "{\"op\":\"act\",\"css\":\"[data-testid=\\\"off\\\"]\",\"action\":{\"kind\":\"click\"}}").getString("error");
            assertTrue(disabled, disabled.startsWith("Element not interactable:"));
            String missing = automate(host, "{\"op\":\"act\",\"css\":\"[data-testid=\\\"nope\\\"]\",\"action\":{\"kind\":\"click\"}}").getString("error");
            assertTrue(missing, missing.startsWith("Element not found:"));
            String unsupported = automate(host, "{\"op\":\"query\",\"css\":\"div > p:nth-child(2)\"}").getString("error");
            assertTrue(unsupported, unsupported.startsWith("E_SELECTOR_UNSUPPORTED"));
        } finally {
            GpuView.nativeHostRelease(host);
            fake.release();
        }
    }
}
