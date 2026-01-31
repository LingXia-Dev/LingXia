package {{PACKAGE_ID}}

import android.os.Bundle
import android.util.Log
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.LxAppLaunchActivity

class MainActivity : LxAppLaunchActivity() {
    private val TAG = "MainActivity"

    /**
     * Register custom native extensions.
     * Called once before LxApp initialization.
     */
    override fun registerExtensions() {
        registerNativeExtensions()
    }

    private external fun registerNativeExtensions()

    override fun onCreate(savedInstanceState: Bundle?) {
        // Enable WebView debugging BEFORE calling super.onCreate()
        LxApp.enableWebViewDebugging()

        super.onCreate(savedInstanceState)
    }
}
