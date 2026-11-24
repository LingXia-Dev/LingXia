package com.lingxia.lxapp

/** Where a collapsed side panel's tongue sits along the panel's edge of a phone window. */
internal object TonguePlacement {
    /**
     * The top of the clear position nearest the middle of an edge [length] long, or null when no
     * clear stretch holds the tongue and it insets instead. [blocked] are (from, to) stretches a
     * cutout or bar covers, kept [margin] clear; [start] and [end] are reserved at the ends.
     */
    fun place(
        length: Float, blocked: List<Pair<Float, Float>>, start: Float, end: Float, size: Float, margin: Float,
    ): Float? {
        val middle = (length - size) / 2
        val clear = mutableListOf<Pair<Float, Float>>()
        var from = start
        for ((low, high) in blocked.sortedBy { it.first }) {
            if (low - margin > from) clear += from to low - margin
            from = maxOf(from, high + margin)
        }
        clear += from to length - end
        return clear
            .filter { it.second - it.first >= size }
            .map { minOf(maxOf(middle, it.first), it.second - size) }
            .minByOrNull { kotlin.math.abs(it - middle) }
    }
}
