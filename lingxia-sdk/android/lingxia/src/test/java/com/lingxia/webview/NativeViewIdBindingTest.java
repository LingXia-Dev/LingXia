package com.lingxia.webview;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import org.junit.Test;

public final class NativeViewIdBindingTest {
    @Test
    public void accepts_one_positive_native_identity() {
        NativeViewIdBinding binding = new NativeViewIdBinding();

        binding.assign(41L);

        assertEquals(41L, binding.current());
        binding.assign(41L);
        assertEquals(41L, binding.current());
    }

    @Test
    public void cannot_rebind_a_reused_route_to_a_successor_identity() {
        NativeViewIdBinding binding = new NativeViewIdBinding();
        binding.assign(41L);

        assertThrows(IllegalStateException.class, () -> binding.assign(42L));
        assertEquals(41L, binding.current());
    }

    @Test
    public void rejects_an_unbound_identity() {
        NativeViewIdBinding binding = new NativeViewIdBinding();

        assertThrows(IllegalArgumentException.class, () -> binding.assign(0L));
    }
}
