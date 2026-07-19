package com.lingxia.lxapp.NativeComponents

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Test

class StreamVideoDataTest {
    @Test
    fun convertsFourByteAvccAccessUnitToAnnexB() {
        val input = byteArrayOf(
            0, 0, 0, 2, 0x65, 0x01,
            0, 0, 0, 3, 0x41, 0x02, 0x03,
        )

        assertArrayEquals(
            byteArrayOf(
                0, 0, 0, 1, 0x65, 0x01,
                0, 0, 0, 1, 0x41, 0x02, 0x03,
            ),
            StreamVideoData.normalizeAccessUnit("avcc", 4, input),
        )
    }

    @Test
    fun honorsConfiguredNalLengthSize() {
        val input = byteArrayOf(0, 2, 0x65, 0x01, 0, 1, 0x41)

        assertArrayEquals(
            byteArrayOf(0, 0, 0, 1, 0x65, 0x01, 0, 0, 0, 1, 0x41),
            StreamVideoData.normalizeAccessUnit("AVCC", 2, input),
        )
    }

    @Test
    fun supportsOneAndThreeByteNalLengthsWithoutMutatingInput() {
        val oneByteInput = byteArrayOf(2, 0x65, 0x01, 1, 0x41)
        val oneByteSnapshot = oneByteInput.copyOf()
        val oneByteOutput = StreamVideoData.normalizeAccessUnit("avcc", 1, oneByteInput)
        assertArrayEquals(
            byteArrayOf(0, 0, 0, 1, 0x65, 0x01, 0, 0, 0, 1, 0x41),
            oneByteOutput,
        )
        assertArrayEquals(oneByteSnapshot, oneByteInput)
        assertNotSame(oneByteInput, oneByteOutput)

        val threeByteInput = byteArrayOf(0, 0, 2, 0x65, 0x01)
        assertArrayEquals(
            byteArrayOf(0, 0, 0, 1, 0x65, 0x01),
            StreamVideoData.normalizeAccessUnit("avcc", 3, threeByteInput),
        )
    }

    @Test
    fun rejectsTruncatedAvccAccessUnit() {
        val input = byteArrayOf(0, 0, 0, 4, 0x65, 0x01)

        assertNull(StreamVideoData.normalizeAccessUnit("avcc", 4, input))
        assertNull(
            StreamVideoData.normalizeAccessUnit(
                "avcc",
                2,
                byteArrayOf(0, 1, 0x65, 0),
            ),
        )
        assertNull(StreamVideoData.normalizeAccessUnit("avcc", 1, byteArrayOf(0)))
        assertNull(StreamVideoData.normalizeAccessUnit("avcc", 4, ByteArray(0)))
        assertNull(StreamVideoData.normalizeAccessUnit("avcc", 0, input))
        assertNull(StreamVideoData.normalizeAccessUnit("avcc", 5, input))
    }

    @Test
    fun leavesAnnexBAccessUnitUntouched() {
        val input = byteArrayOf(0, 0, 0, 1, 0x65, 0x01)

        assertSame(input, StreamVideoData.normalizeAccessUnit("annexb", 4, input))
    }

    @Test
    fun normalizesCodecSpecificDataToOneFourByteStartCode() {
        assertArrayEquals(
            byteArrayOf(0, 0, 0, 1, 0x67, 0x64),
            StreamVideoData.withAnnexBStartCode(byteArrayOf(0, 0, 1, 0x67, 0x64)),
        )
        assertArrayEquals(
            byteArrayOf(0, 0, 0, 1, 0x68, 0x01),
            StreamVideoData.withAnnexBStartCode(byteArrayOf(0, 0, 0, 1, 0x68, 0x01)),
        )
        assertArrayEquals(
            byteArrayOf(0, 0, 0, 1, 0x67, 0x64),
            StreamVideoData.withAnnexBStartCode(
                byteArrayOf(0, 0, 1, 0, 0, 0, 1, 0x67, 0x64),
            ),
        )
        assertArrayEquals(
            ByteArray(0),
            StreamVideoData.withAnnexBStartCode(byteArrayOf(0, 0, 0, 1)),
        )
    }
}
