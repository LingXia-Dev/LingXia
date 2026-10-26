package com.lingxia.ui;

import android.app.Activity;
import android.graphics.Rect;
import android.view.View;
import java.util.ArrayList;
import java.util.function.Consumer;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.Robolectric;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;
import static org.junit.Assert.*;

/** The `<select>` picker shows the frame's options and answers with one `{op:"select"}`. */
@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 28)
public class GpuPickerTest {
    static final class Host implements GpuPicker.Host {
        final ArrayList<JSONObject> requests = new ArrayList<>();
        final ArrayList<Consumer<JSONObject>> replies = new ArrayList<>();
        final ArrayList<String> opened = new ArrayList<>();
        final ArrayList<Boolean> renders = new ArrayList<>();
        public void request(JSONObject request, Consumer<JSONObject> reply) { requests.add(request); replies.add(reply); }
        public void render(boolean rebuild) { renders.add(rebuild); }
        public void open(String identity) { opened.add(identity); }
    }

    private static JSONObject select(int count, int disabled, int selected) throws Exception {
        JSONArray options = new JSONArray();
        for (int i = 0; i < count; i++)
            options.put(new JSONObject().put("value", "v" + i).put("label", "Plan " + i).put("enabled", i != disabled));
        return new JSONObject().put("identity", "plan").put("tag", "select")
            .put("select", new JSONObject().put("options", options).put("selected", selected));
    }

    private static View anchor() {
        Activity activity = Robolectric.buildActivity(Activity.class).setup().get();
        View view = new View(activity);
        activity.setContentView(view);
        return view;
    }

    @Test public void aShortListDropsDownAndAChoiceAnswersOnce() throws Exception {
        Host host = new Host();
        GpuPicker picker = new GpuPicker(anchor(), host);
        picker.show(select(3, 1, 0), 5, new Rect(10, 20, 210, 70));
        assertNotNull("up to eight options drop down", picker.dropdown());
        assertEquals("plan", host.opened.get(0));
        android.widget.ListAdapter adapter = picker.dropdown().getListView().getAdapter();
        assertFalse("a disabled option cannot be chosen", adapter.isEnabled(1));
        picker.dropdown().performItemClick(2);
        assertFalse(picker.showing());
        assertEquals(1, host.requests.size());
        JSONObject request = host.requests.get(0);
        assertEquals("select", request.getString("op"));
        assertEquals("plan", request.getString("identity"));
        assertEquals(5, request.getLong("generation"));
        assertEquals(2, request.getInt("index"));
        assertNull("closed", host.opened.get(host.opened.size() - 1));
        host.replies.get(0).accept(new JSONObject().put("needs_view_render", true));
        assertEquals(java.util.Collections.singletonList(true), host.renders);
    }

    @Test public void aLongListOpensADialogAndDismissingSendsNothing() throws Exception {
        Host host = new Host();
        GpuPicker picker = new GpuPicker(anchor(), host);
        picker.show(select(GpuPicker.DROPDOWN_OPTIONS + 2, -1, 4), 7, new Rect(0, 0, 100, 40));
        assertNull(picker.dropdown());
        assertNotNull("more options open a dialog", picker.list());
        assertEquals(4, picker.list().getListView().getCheckedItemPosition());
        picker.dismiss();
        assertTrue(host.requests.isEmpty());
        assertFalse(picker.showing());
        assertNull(host.opened.get(host.opened.size() - 1));
    }
}
