package com.lingxia.lxapp.NativeComponents

internal object StreamVideoData {
    private val annexBStartCode = byteArrayOf(0, 0, 0, 1)

    fun normalizeAccessUnit(
        format: String,
        nalLengthSize: Int?,
        data: ByteArray,
    ): ByteArray? {
        if (!format.equals("avcc", ignoreCase = true)) return data
        if (data.isEmpty()) return null

        val lengthSize = when (nalLengthSize) {
            null -> 4
            in 1..4 -> nalLengthSize
            else -> return null
        }
        var outputSize = 0L
        var nalCount = 0
        var offset = 0

        while (offset < data.size) {
            if (data.size - offset < lengthSize) return null
            val nalLength = readNalLength(data, offset, lengthSize)
            offset += lengthSize
            if (nalLength == 0L) continue
            if (nalLength > data.size - offset) return null

            outputSize += annexBStartCode.size + nalLength
            if (outputSize > Int.MAX_VALUE) return null
            nalCount += 1
            offset += nalLength.toInt()
        }

        if (nalCount == 0) return null

        val output = ByteArray(outputSize.toInt())
        var inputOffset = 0
        var outputOffset = 0
        while (inputOffset < data.size) {
            val nalLength = readNalLength(data, inputOffset, lengthSize).toInt()
            inputOffset += lengthSize
            if (nalLength == 0) continue

            annexBStartCode.copyInto(output, outputOffset)
            outputOffset += annexBStartCode.size
            data.copyInto(
                destination = output,
                destinationOffset = outputOffset,
                startIndex = inputOffset,
                endIndex = inputOffset + nalLength,
            )
            inputOffset += nalLength
            outputOffset += nalLength
        }
        return output
    }

    fun withAnnexBStartCode(data: ByteArray): ByteArray {
        if (data.isEmpty()) return data

        var payloadOffset = 0
        while (true) {
            val prefixLength = annexBPrefixLengthAt(data, payloadOffset)
            if (prefixLength == 0) break
            payloadOffset += prefixLength
        }
        if (payloadOffset == data.size) return ByteArray(0)

        val output = ByteArray(annexBStartCode.size + data.size - payloadOffset)
        annexBStartCode.copyInto(output)
        data.copyInto(output, annexBStartCode.size, payloadOffset)
        return output
    }

    private fun readNalLength(data: ByteArray, offset: Int, lengthSize: Int): Long {
        var length = 0L
        for (index in offset until offset + lengthSize) {
            length = (length shl 8) or (data[index].toInt() and 0xFF).toLong()
        }
        return length
    }

    private fun annexBPrefixLengthAt(data: ByteArray, offset: Int): Int {
        val remaining = data.size - offset
        if (remaining >= 4 &&
            data[offset] == 0.toByte() &&
            data[offset + 1] == 0.toByte() &&
            data[offset + 2] == 0.toByte() &&
            data[offset + 3] == 1.toByte()
        ) {
            return 4
        }
        if (remaining >= 3 &&
            data[offset] == 0.toByte() &&
            data[offset + 1] == 0.toByte() &&
            data[offset + 2] == 1.toByte()
        ) {
            return 3
        }
        return 0
    }
}
