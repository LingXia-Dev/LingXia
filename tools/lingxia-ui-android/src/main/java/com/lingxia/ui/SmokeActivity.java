package com.lingxia.ui;

import android.app.Activity;
import android.os.Bundle;
import android.graphics.Color;
import android.view.View;
import android.widget.FrameLayout;
import android.widget.Button;
import android.widget.TextView;
import org.json.JSONObject;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Device harness using the SDK's GPU content component. */
public final class SmokeActivity extends Activity {
    static {
        System.loadLibrary("lingxia_ui");
        GpuView.initializeNative();
    }
    private final ExecutorService worker = Executors.newSingleThreadExecutor(r -> new Thread(r, "lingxia-ui-view"));
    private GpuView[] pages;
    private int selected;
    private FrameLayout content;

    private String asset(String path) throws Exception {
        try (java.io.InputStream input = getAssets().open(path)) {
            java.io.ByteArrayOutputStream output = new java.io.ByteArrayOutputStream();
            byte[] buffer = new byte[8192]; int length;
            while ((length = input.read(buffer)) >= 0) output.write(buffer, 0, length);
            return output.toString("UTF-8");
        }
    }

    @Override public void onCreate(Bundle saved) {
        super.onCreate(saved);
        int background = Color.rgb(246,247,251);
        getWindow().getDecorView().setBackgroundColor(background);
        getWindow().setStatusBarColor(background);
        getWindow().setNavigationBarColor(background);
        getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR);
        FrameLayout root = new FrameLayout(this);
        content = new FrameLayout(this);
        boolean multiple = getIntent().getBooleanExtra("multiPage", false);
        FrameLayout.LayoutParams layout = new FrameLayout.LayoutParams(-1, -1);
        if (multiple) layout.topMargin = (int)(48 * getResources().getDisplayMetrics().density);
        root.addView(content, layout);
        if (multiple) {
            Button next = new Button(this); next.setText("Switch page");
            root.addView(next, new FrameLayout.LayoutParams(-1, layout.topMargin));
            next.setOnClickListener(view -> {
                pages[selected].pause(); content.removeAllViews(); selected = 1 - selected;
                content.addView(pages[selected], new FrameLayout.LayoutParams(-1, -1)); pages[selected].resume();
            });
        }
        root.setOnApplyWindowInsetsListener((view, insets) -> {
            view.setPadding(0, insets.getSystemWindowInsetTop(), 0, insets.getSystemWindowInsetBottom());
            return insets.consumeSystemWindowInsets();
        });
        setContentView(root);
        try {
            JSONObject manifest = new JSONObject(asset("lxapp.json"));
            if (manifest.optBoolean("logic", true)) throw new IllegalArgumentException("Harness requires logic: false");
            String path = manifest.getJSONArray("pages").getJSONObject(0).getString("path");
            if (path.endsWith(".ui.js")) path = path.substring(0, path.length() - 6);
            if (!path.matches("pages/[a-zA-Z0-9_/-]+")) throw new IllegalArgumentException("Invalid page path");
            if (!new JSONObject(asset(path + ".json")).getString("renderer").equals("gpu")) throw new IllegalArgumentException("Expected GPU page");
            String source = asset(path + ".ui.js");
            pages = new GpuView[multiple ? 2 : 1];
            for (int index = 0; index < pages.length; index++) {
                final int page = index;
                pages[index] = new GpuView(this, source, worker, new GpuView.Listener() {
                    public void onLoaded() { }
                    public void onError(Throwable error) { android.util.Log.e("LingXia.UI", "Page " + page, error); }
                    public void onFrame(String metadata) {
                        worker.execute(() -> {
                            try {
                                JSONObject snapshot = new JSONObject(metadata); snapshot.put("page", page);
                                java.io.File temporary = new java.io.File(getFilesDir(), "ui-snapshot.tmp");
                                try (java.io.FileOutputStream output = new java.io.FileOutputStream(temporary)) { output.write(snapshot.toString().getBytes(StandardCharsets.UTF_8)); }
                                if (!temporary.renameTo(new java.io.File(getFilesDir(), "ui-snapshot.json"))) throw new java.io.IOException("Cannot publish snapshot");
                            } catch (Exception error) { android.util.Log.w("LingXia.UI", "Snapshot write failed", error); }
                        });
                    }
                });
            }
            content.addView(pages[0], new FrameLayout.LayoutParams(-1, -1));
        } catch (Exception error) {
            TextView message = new TextView(this); message.setText(error.toString()); content.addView(message);
        }
    }
    @Override protected void onResume() { super.onResume(); if (pages != null) pages[selected].resume(); }
    @Override protected void onPause() { if (pages != null) pages[selected].pause(); super.onPause(); }
    @Override protected void onDestroy() {
        if (pages != null) for (GpuView page : pages) if (page != null) page.dispose();
        worker.shutdown(); super.onDestroy();
    }
}
