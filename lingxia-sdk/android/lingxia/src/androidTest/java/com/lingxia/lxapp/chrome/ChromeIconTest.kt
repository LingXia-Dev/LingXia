package com.lingxia.lxapp.chrome

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

@RunWith(AndroidJUnit4::class)
class ChromeIconTest {
    @Test fun explicitSvgDimensionsScaleToDensitySizedBoundsAndPreserveHoles() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val file = File.createTempFile("chrome-icon", ".svg", context.cacheDir)
        try {
            file.writeText("""<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><rect x="3" y="3" width="18" height="18" fill="none" stroke="black" stroke-width="2"/></svg>""")
            val drawable = ChromeIcon.load(file.path, Color.BLUE) { ColorDrawable(Color.RED) }
            for (size in listOf(24, 48, 72)) {
                val bitmap = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
                drawable.setBounds(0, 0, size, size)
                drawable.draw(Canvas(bitmap))
                assertEquals("stroke at density-scaled edge ($size)", Color.BLUE,
                    bitmap.getPixel(size / 8, size / 2))
                assertEquals("center stays transparent", 0, Color.alpha(bitmap.getPixel(size / 2, size / 2)))
                assertEquals("outer margin stays transparent", 0, Color.alpha(bitmap.getPixel(0, 0)))
                bitmap.recycle()
            }
        } finally {
            file.delete()
        }
    }
}
