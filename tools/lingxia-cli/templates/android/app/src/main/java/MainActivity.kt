package {{PACKAGE_ID}}

import android.content.Intent
import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import com.lingxia.app.Lingxia

class MainActivity : AppCompatActivity() {
    private val TAG = "MainActivity"

    private external fun nativeRegisterHostAddon()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Lingxia.quickStart(this) {
            nativeRegisterHostAddon()
        }

        Log.d(TAG, "LxApp is ready")
    }

    // singleTop (never singleTask, which clears the running app off the task on
    // every launcher tap): a link that arrives while this activity is on top
    // lands here.
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        Lingxia.handleAppLink(intent)
    }
}
