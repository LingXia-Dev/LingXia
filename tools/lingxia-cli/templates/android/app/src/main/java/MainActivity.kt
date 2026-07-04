package {{PACKAGE_ID}}

import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import com.lingxia.lxapp.Lingxia

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
}
