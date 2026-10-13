package com.lingxia.lxapp.NativeComponents

import android.content.Context
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import com.lingxia.app.NativeApi
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.Implementation
import org.robolectric.annotation.Implements

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [28], shadows = [InlineNativeIslandTest.MediaPolicy::class],
    instrumentedPackages = ["com.lingxia.app"])
class InlineNativeIslandTest {
    // Only the Rust policy boundary is replaced; commits, native views and
    // layout requests run through the production island.
    @Implements(NativeApi::class)
    class MediaPolicy {
        companion object {
            @JvmStatic @Implementation fun ensureLoaded() = true
            @JvmStatic @Implementation fun validateInlineNativeMediaUrls(appId: String, urlsJson: String): String? = null
        }
    }

    private class Host(context: Context) : FrameLayout(context) {
        var layoutRequests = 0
        override fun requestLayout() {
            layoutRequests++
            super.requestLayout()
        }
        fun settle() {
            val size = View.MeasureSpec.makeMeasureSpec(400, View.MeasureSpec.EXACTLY)
            measure(size, size)
            layout(0, 0, 400, 400)
            layoutRequests = 0
        }
    }

    private val root = mapOf<String, Any?>("rootKey" to "root", "rootEpoch" to 1,
        "surfaceInstanceId" to "surface", "pageInstanceId" to "page", "documentInstanceId" to "doc")
    private fun ref(key: String) = root + ("nodeKey" to key)
    private fun mount(key: String, order: Int, parent: String? = null) = mapOf(
        "op" to "mount", "node" to mapOf("ref" to ref(key), "kind" to "view", "authorId" to key,
            "parent" to parent?.let(::ref), "order" to order, "props" to mapOf("aria-label" to key)))
    private fun commit(island: InlineNativeIsland, base: Int, vararg ops: Map<String, Any?>) {
        island.handle(mapOf("action" to "root.commit", "root" to root,
            "baseRevision" to base, "revision" to base + 1, "operations" to ops.toList()))
    }
    private fun geometry(island: InlineNativeIsland, vararg keys: String, y: Int = 20) {
        island.handle(mapOf("action" to "geometry.snapshot",
            "roots" to listOf(mapOf("ref" to root, "rootOrder" to 0)),
            "nodes" to keys.map { key -> mapOf("ref" to ref(key), "visible" to true,
                "contentRect" to mapOf("x" to 10, "y" to y, "width" to 100, "height" to 60)) }))
    }
    private fun order(container: ViewGroup) = (0 until container.childCount).map {
        ((container.getChildAt(it) as ViewGroup).getChildAt(0)).contentDescription.toString()
    }

    @Test fun repeatedGeometryDoesNotRequestLayoutOrReplaceParams() {
        val host = Host(RuntimeEnvironment.getApplication())
        val island = InlineNativeIsland(host, "test") { _, _, _ -> }
        commit(island, 0, mount("a", 0), mount("b", 1))
        geometry(island, "a", "b")
        val container = host.getChildAt(0) as ViewGroup
        val viewport = container.getChildAt(0) as ViewGroup
        val viewportParams = viewport.layoutParams
        val contentParams = viewport.getChildAt(0).layoutParams
        host.settle()
        repeat(100) { geometry(island, "a", "b") }
        assertSame(viewportParams, viewport.layoutParams)
        assertSame(contentParams, viewport.getChildAt(0).layoutParams)
        assertEquals(0, host.layoutRequests)
        assertEquals(listOf("a", "b"), order(container))

        geometry(island, "a", "b", y = 40)
        assertTrue(host.layoutRequests > 0)
        assertNotSame(viewportParams, viewport.layoutParams)
        host.settle()
        island.onWebViewScroll(0, 12)
        assertTrue(host.layoutRequests > 0)
        host.settle()
        island.onWebViewScroll(0, 12)
        assertEquals(0, host.layoutRequests)
    }

    @Test fun structuralChangesStillOrderNestedNodesAndReplacements() {
        val host = Host(RuntimeEnvironment.getApplication())
        val island = InlineNativeIsland(host, "test") { _, _, _ -> }
        commit(island, 0, mount("a", 0), mount("b", 1), mount("child", 0, "a"))
        val container = host.getChildAt(0) as ViewGroup
        assertEquals(listOf("a", "child", "b"), order(container))
        commit(island, 1, mapOf("op" to "reorder", "node" to ref("a"), "order" to 2))
        assertEquals(listOf("b", "a", "child"), order(container))
        commit(island, 2, mapOf("op" to "reparent", "node" to ref("child"), "parent" to ref("b")))
        assertEquals(listOf("b", "child", "a"), order(container))
        val previous = container.getChildAt(0)
        commit(island, 3, mount("b", 1))
        assertEquals(listOf("b", "child", "a"), order(container))
        assertNotSame(previous, container.getChildAt(0))
        commit(island, 4, mapOf("op" to "unmount", "node" to ref("child")))
        assertEquals(listOf("b", "a"), order(container))
        island.teardown()
        assertEquals(0, host.childCount)
    }
}
