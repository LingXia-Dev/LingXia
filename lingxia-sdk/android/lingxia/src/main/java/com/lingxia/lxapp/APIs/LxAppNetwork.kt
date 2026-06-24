package com.lingxia.lxapp.APIs

import android.content.Context
import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.net.LinkProperties
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.telephony.SubscriptionManager
import android.telephony.TelephonyManager
import com.lingxia.app.Lingxia
import com.lingxia.app.LxLog
import com.lingxia.lxapp.LxApp
import com.lingxia.app.NativeApi
import org.json.JSONArray
import org.json.JSONObject
import java.net.Inet4Address
import java.net.Inet6Address
import java.net.InetAddress
import java.net.NetworkInterface
import java.util.LinkedHashSet
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CopyOnWriteArraySet
import java.util.concurrent.atomic.AtomicLong

fun interface NetworkStatusListener {
    fun onNetworkStatusChanged(isConnected: Boolean)
}

object LxAppNetwork {
    private const val TAG = "LingXia.Network"
    private const val NETWORK_TYPE_NR = 20 // TelephonyManager.NETWORK_TYPE_NR (API 29+)
    private const val NETWORK_TYPE_LTE_CA = 19 // TelephonyManager.NETWORK_TYPE_LTE_CA (API 24+)
    // Coalesce bursts of onAvailable/onCapabilitiesChanged/onLinkPropertiesChanged into a single resolve.
    private const val EMIT_DEBOUNCE_MS = 50L

    private val changeCallbacks = CopyOnWriteArraySet<Long>()
    private val statusListeners = java.util.concurrent.ConcurrentHashMap<Long, NetworkStatusListener>()
    private var nextStatusListenerId = AtomicLong(1L)
    private val workerThread = HandlerThread("LingXia.Network").apply { start() }
    private val workerHandler = Handler(workerThread.looper)
    private val emitRunnable = Runnable { emitInfoToAll() }
    @Volatile private var networkCallback: ConnectivityManager.NetworkCallback? = null
    @Volatile private var connectivityManager: ConnectivityManager? = null
    @Volatile private var lastSignature: String? = null
    @Volatile private var lastIsConnected: Boolean? = null

    @JvmStatic
    fun getNetworkInfo(callbackId: Long) {
        emitInfoTo(callbackId)
    }

    @JvmStatic
    fun addNetworkChangeListener(callbackId: Long) {
        if (!changeCallbacks.add(callbackId)) {
            emitInfoTo(callbackId)
            return
        }

        val context = Lingxia.applicationContext()
        val connMgr = getConnectivityManager(context)
        if (changeCallbacks.size == 1 && connMgr != null) {
            registerNetworkCallback(connMgr)
        }
        emitInfoTo(callbackId)
    }

    @JvmStatic
    fun removeNetworkChangeListener(callbackId: Long) {
        if (!changeCallbacks.remove(callbackId)) {
            return
        }
        if (changeCallbacks.isEmpty()) {
            unregisterNetworkCallback()
            lastSignature = null
        }
    }

    @JvmStatic
    fun addNetworkStatusListener(listener: NetworkStatusListener): Long {
        val id = nextStatusListenerId.getAndIncrement()
        statusListeners[id] = listener

        // Ensure the OS NetworkCallback is registered even when the only consumer
        // is a Java/Kotlin listener (no JS-side `lx.onNetworkChange` subscriber).
        // Without this the listener would silently never receive updates.
        val context = Lingxia.applicationContext()
        val connMgr = getConnectivityManager(context)
        if (connMgr != null && networkCallback == null) {
            registerNetworkCallback(connMgr)
        }

        // Fire the current state immediately ONLY if we can actually resolve one.
        // When this is called before `Lingxia.initializeRuntime` finishes (e.g. an Application or
        // Activity registered early), `applicationContext` is still null and
        // `resolveNetworkStatus(null)` would falsely return "disconnected" — causing
        // a brief no-network UI flash on cold start. In that case, defer the first
        // emission to the OS NetworkCallback path, which will fire with the real
        // state once ConnectivityManager is available.
        if (context != null) {
            val current = lastIsConnected ?: resolveAndCache()
            listener.onNetworkStatusChanged(current)
        }
        return id
    }

    private fun resolveAndCache(): Boolean {
        val connected = resolveNetworkStatus(Lingxia.applicationContext()).isConnected
        lastIsConnected = connected
        return connected
    }

    @JvmStatic
    fun removeNetworkStatusListener(id: Long) {
        statusListeners.remove(id)
    }

    private fun registerNetworkCallback(connMgr: ConnectivityManager) {
        if (networkCallback != null) {
            return
        }
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                dispatchNetworkChange()
            }

            override fun onCapabilitiesChanged(network: Network, networkCapabilities: NetworkCapabilities) {
                dispatchNetworkChange()
            }

            override fun onLinkPropertiesChanged(network: Network, linkProperties: LinkProperties) {
                dispatchNetworkChange()
            }

            override fun onLost(network: Network) {
                dispatchNetworkChange()
            }
        }
        networkCallback = callback

        // Run register/unregister and deliver callbacks on a dedicated worker looper. This keeps
        // resolve work (telephony queries, NetworkInterface enumeration) off the main thread and
        // gives the OS a stable Looper to deliver callbacks on.
        workerHandler.post {
            try {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                        connMgr.registerDefaultNetworkCallback(callback, workerHandler)
                    } else {
                        connMgr.registerDefaultNetworkCallback(callback)
                    }
                } else {
                    val request = NetworkRequest.Builder()
                        .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
                        .build()
                    connMgr.registerNetworkCallback(request, callback)
                }
            } catch (e: Exception) {
                LxLog.w(TAG, "Failed to register network callback", e)
                networkCallback = null
            }
        }
    }

    private fun dispatchNetworkChange() {
        // Trailing-edge debounce: the OS often delivers onAvailable + onCapabilitiesChanged +
        // onLinkPropertiesChanged in quick succession; collapse them into one resolve+emit.
        workerHandler.removeCallbacks(emitRunnable)
        workerHandler.postDelayed(emitRunnable, EMIT_DEBOUNCE_MS)
    }

    private fun unregisterNetworkCallback() {
        val connMgr = connectivityManager ?: return
        val callback = networkCallback ?: return
        // Clear slot first to avoid add->remove->add races losing the active callback.
        networkCallback = null
        workerHandler.removeCallbacks(emitRunnable)
        workerHandler.post {
            try {
                connMgr.unregisterNetworkCallback(callback)
            } catch (e: Exception) {
                LxLog.w(TAG, "Failed to unregister network callback", e)
            }
        }
    }

    private fun emitInfoTo(callbackId: Long) {
        val info = resolveNetworkInfo(Lingxia.applicationContext())
        val payload = JSONObject().apply {
            put("isConnected", info.isConnected)
            put("networkType", info.networkType)
            put("ipv4", JSONArray(info.ipv4))
            put("ipv6", JSONArray(info.ipv6))
        }
        NativeApi.onCallback(callbackId, true, payload.toString())
    }

    private fun emitInfoToAll() {
        if (changeCallbacks.isEmpty() && statusListeners.isEmpty()) {
            return
        }
        val info = resolveNetworkInfo(Lingxia.applicationContext())
        val signature = buildNetworkInfoSignature(info)
        if (signature == lastSignature) {
            return
        }
        lastSignature = signature

        // Rust callbacks (full network info).
        if (changeCallbacks.isNotEmpty()) {
            val payload = JSONObject().apply {
                put("isConnected", info.isConnected)
                put("networkType", info.networkType)
                put("ipv4", JSONArray(info.ipv4))
                put("ipv6", JSONArray(info.ipv6))
            }.toString()
            changeCallbacks.forEach { id ->
                NativeApi.onCallback(id, true, payload)
            }
        }

        // Java status listeners (connected/disconnected only).
        val prev = lastIsConnected
        if (prev == null || prev != info.isConnected) {
            lastIsConnected = info.isConnected
            statusListeners.values.forEach { listener ->
                listener.onNetworkStatusChanged(info.isConnected)
            }
        }
    }

    private fun getConnectivityManager(context: Context?): ConnectivityManager? {
        if (context == null) {
            return null
        }
        if (connectivityManager == null) {
            connectivityManager =
                context.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager
        }
        return connectivityManager
    }

    private data class NetworkStatusData(
        val isConnected: Boolean,
        val networkType: String,
    )

    private data class NetworkInfoData(
        val isConnected: Boolean,
        val networkType: String,
        val ipv4: List<String>,
        val ipv6: List<String>,
    )

    private fun buildNetworkInfoSignature(info: NetworkInfoData): String {
        return buildString {
            append(info.isConnected)
            append(':')
            append(info.networkType)
            append(':')
            append(info.ipv4.joinToString(","))
            append(':')
            append(info.ipv6.joinToString(","))
        }
    }

    private fun resolveNetworkStatus(context: Context?): NetworkStatusData {
        val connMgr = getConnectivityManager(context) ?: return NetworkStatusData(false, "none")
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) {
            @Suppress("DEPRECATION")
            val networkInfo = connMgr.activeNetworkInfo ?: return NetworkStatusData(false, "none")
            @Suppress("DEPRECATION")
            if (!networkInfo.isConnected) return NetworkStatusData(false, "none")

            @Suppress("DEPRECATION")
            val type = when (networkInfo.type) {
                ConnectivityManager.TYPE_WIFI -> "wifi"
                ConnectivityManager.TYPE_ETHERNET -> "ethernet"
                ConnectivityManager.TYPE_MOBILE -> resolveCellularNetworkType(context, connMgr)
                ConnectivityManager.TYPE_VPN -> "unknown"
                else -> "unknown"
            }
            return NetworkStatusData(true, type)
        }

        val network = connMgr.activeNetwork ?: return NetworkStatusData(false, "none")
        val capabilities = connMgr.getNetworkCapabilities(network)
            ?: return NetworkStatusData(false, "none")

        val resolved = resolveNetworkType(connMgr, network, capabilities, context)
        return NetworkStatusData(
            resolved.isConnected,
            if (resolved.isConnected) resolved.networkType else "none",
        )
    }

    private fun resolveNetworkInfo(context: Context?): NetworkInfoData {
        val status = resolveNetworkStatus(context)
        if (!status.isConnected) {
            return NetworkInfoData(
                isConnected = false,
                networkType = "none",
                ipv4 = emptyList(),
                ipv6 = emptyList(),
            )
        }

        val ips = resolveLocalIpAddresses(context)
        return NetworkInfoData(
            isConnected = true,
            networkType = status.networkType,
            ipv4 = ips.ipv4,
            ipv6 = ips.ipv6,
        )
    }

    private data class ResolvedNetworkType(
        val isConnected: Boolean,
        val networkType: String,
    )

    private fun resolveNetworkType(
        connMgr: ConnectivityManager,
        active: Network,
        caps: NetworkCapabilities,
        context: Context?,
    ): ResolvedNetworkType {
        // "Connected" here means the OS currently reports an active network.
        // Do NOT gate on VALIDATED: many real networks (captive portals, restricted DNS,
        // or regions without Google reachability) won't be validated but still should show as WiFi.
        val connected =
            caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) ||
                caps.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) ||
                caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) ||
                caps.hasTransport(NetworkCapabilities.TRANSPORT_VPN) ||
                caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)

        val type = when {
            caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) -> "wifi"
            caps.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) ->
                resolveCellularNetworkType(context, connMgr)
            caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) -> "ethernet"
            // Some devices report TRANSPORT_VPN as the active network; try to infer the underlying transport.
            caps.hasTransport(NetworkCapabilities.TRANSPORT_VPN) ->
                resolveUnderlyingType(connMgr, active, context) ?: "unknown"
            else -> "unknown"
        }

        return ResolvedNetworkType(connected, if (connected) type else "none")
    }

    private fun resolveUnderlyingType(
        connMgr: ConnectivityManager,
        active: Network,
        context: Context?,
    ): String? {
        // Best-effort: look for a concurrent non-VPN network with internet.
        // If none found, we keep "unknown" (still connected).
        connMgr.allNetworks.forEach { net ->
            if (net == active) return@forEach
            val c = connMgr.getNetworkCapabilities(net) ?: return@forEach
            if (!c.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)) return@forEach

            when {
                c.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) -> return "wifi"
                c.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) ->
                    return resolveCellularNetworkType(context, connMgr)
                c.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) -> return "ethernet"
            }
        }
        return null
    }

    private fun resolveCellularNetworkType(
        context: Context?,
        connMgr: ConnectivityManager? = null,
    ): String {
        val ctx = context ?: return "unknown"
        val baseTm = ctx.getSystemService(Context.TELEPHONY_SERVICE) as? TelephonyManager ?: return "unknown"

        // Multi-SIM devices can report UNKNOWN unless querying the default data subscription.
        val tm = try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                val subId = SubscriptionManager.getDefaultDataSubscriptionId()
                if (subId != SubscriptionManager.INVALID_SUBSCRIPTION_ID) {
                    baseTm.createForSubscriptionId(subId)
                } else {
                    baseTm
                }
            } else {
                baseTm
            }
        } catch (_: Throwable) {
            baseTm
        }

        // Gather multiple sources; some devices return UNKNOWN for dataNetworkType even while connected.
        val dataType: Int? = try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) tm.dataNetworkType else null
        } catch (_: Throwable) {
            null
        }
        val netType: Int? = try {
            @Suppress("DEPRECATION")
            tm.networkType
        } catch (_: Throwable) {
            null
        }
        val voiceType: Int? = try {
            tm.voiceNetworkType
        } catch (_: Throwable) {
            null
        }

        val unknown = TelephonyManager.NETWORK_TYPE_UNKNOWN
        val networkType =
            listOfNotNull(dataType, netType, voiceType).firstOrNull { it != unknown && it != 0 } ?: unknown

        // 5G NSA devices may report LTE here; try to detect via TelephonyDisplayInfo override network type.
        val overrideType = getDisplayOverrideNetworkType(tm)
        if (overrideType == 3 || overrideType == 4) {
            return "5g"
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && networkType == NETWORK_TYPE_NR) {
            return "5g"
        }

        var mapped = mapRadioNetworkType(networkType)
        var source = "telephony"

        // Some devices return UNKNOWN from TelephonyManager even when mobile data is active.
        // NetworkInfo.subtype is deprecated but still reports the active RAT on these devices.
        if (mapped == "unknown") {
            val subtype = try {
                @Suppress("DEPRECATION")
                (connMgr ?: getConnectivityManager(ctx))?.activeNetworkInfo?.subtype
            } catch (_: Throwable) {
                null
            }
            if (subtype != null) {
                val mappedBySubtype = mapRadioNetworkType(subtype)
                if (mappedBySubtype != "unknown") {
                    mapped = mappedBySubtype
                    source = "activeNetworkInfo"
                }
            }
        }

        if (mapped == "unknown") {
            val hasReadPhoneState =
                ctx.checkCallingOrSelfPermission("android.permission.READ_PHONE_STATE") == PackageManager.PERMISSION_GRANTED
            val hasReadBasicPhoneState =
                ctx.checkCallingOrSelfPermission("android.permission.READ_BASIC_PHONE_STATE") == PackageManager.PERMISSION_GRANTED
            LxLog.w(
                TAG,
                "Cellular network type unknown; dataType=$dataType netType=$netType voiceType=$voiceType chosen=$networkType overrideType=$overrideType readPhoneState=$hasReadPhoneState readBasicPhoneState=$hasReadBasicPhoneState",
            )
        } else {
            LxLog.d(
                TAG,
                "Cellular network type resolved=$mapped source=$source dataType=$dataType netType=$netType voiceType=$voiceType overrideType=$overrideType",
            )
        }
        return mapped
    }

    private fun mapRadioNetworkType(networkType: Int): String {
        return when (networkType) {
            TelephonyManager.NETWORK_TYPE_LTE,
            NETWORK_TYPE_LTE_CA -> "4g"

            TelephonyManager.NETWORK_TYPE_UMTS,
            TelephonyManager.NETWORK_TYPE_HSDPA,
            TelephonyManager.NETWORK_TYPE_HSUPA,
            TelephonyManager.NETWORK_TYPE_HSPA,
            TelephonyManager.NETWORK_TYPE_HSPAP,
            TelephonyManager.NETWORK_TYPE_EVDO_0,
            TelephonyManager.NETWORK_TYPE_EVDO_A,
            TelephonyManager.NETWORK_TYPE_EVDO_B,
            TelephonyManager.NETWORK_TYPE_EHRPD,
            TelephonyManager.NETWORK_TYPE_TD_SCDMA,
            // Some Android SDK stubs don't include NETWORK_TYPE_WCDMA. Most devices report UMTS/HSPA instead.
            -> "3g"

            TelephonyManager.NETWORK_TYPE_GPRS,
            TelephonyManager.NETWORK_TYPE_EDGE,
            TelephonyManager.NETWORK_TYPE_CDMA,
            TelephonyManager.NETWORK_TYPE_1xRTT,
            TelephonyManager.NETWORK_TYPE_IDEN,
            TelephonyManager.NETWORK_TYPE_GSM -> "2g"

            else -> "unknown"
        }
    }

    private fun getDisplayOverrideNetworkType(tm: TelephonyManager): Int? {
        return try {
            // TelephonyManager#getDisplayInfo exists on newer Android; use reflection to avoid compileSdk constraints.
            val getDisplayInfo = tm.javaClass.getMethod("getDisplayInfo")
            val displayInfo = getDisplayInfo.invoke(tm) ?: return null
            val getOverride = displayInfo.javaClass.getMethod("getOverrideNetworkType")
            getOverride.invoke(displayInfo) as? Int
        } catch (_: Throwable) {
            null
        }
    }

    private data class LocalIpAddresses(
        val ipv4: List<String>,
        val ipv6: List<String>,
    )

    private fun selectPrimaryAddress(addresses: Set<String>): List<String> {
        if (addresses.isEmpty()) {
            return emptyList()
        }
        val sorted = addresses.toMutableList()
        sorted.sort()
        return listOf(sorted[0])
    }

    private fun resolveLocalIpAddresses(context: Context?): LocalIpAddresses {
        val connMgr = getConnectivityManager(context) ?: return LocalIpAddresses(emptyList(), emptyList())
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) {
            return resolveLocalIpAddressesFromInterfaces()
        }

        val network = connMgr.activeNetwork ?: return LocalIpAddresses(emptyList(), emptyList())
        val props: LinkProperties = connMgr.getLinkProperties(network)
            ?: return LocalIpAddresses(emptyList(), emptyList())

        val ipv4 = LinkedHashSet<String>()
        val ipv6 = LinkedHashSet<String>()
        for (address in props.linkAddresses) {
            collectInetAddress(address.address, ipv4, ipv6)
        }
        return LocalIpAddresses(
            ipv4 = selectPrimaryAddress(ipv4),
            ipv6 = selectPrimaryAddress(ipv6),
        )
    }

    private fun resolveLocalIpAddressesFromInterfaces(): LocalIpAddresses {
        val ipv4 = LinkedHashSet<String>()
        val ipv6 = LinkedHashSet<String>()

        try {
            val interfaces = NetworkInterface.getNetworkInterfaces() ?: return LocalIpAddresses(emptyList(), emptyList())
            while (interfaces.hasMoreElements()) {
                val networkInterface = interfaces.nextElement()
                if (!networkInterface.isUp || networkInterface.isLoopback) {
                    continue
                }
                val addresses = networkInterface.inetAddresses
                while (addresses.hasMoreElements()) {
                    collectInetAddress(addresses.nextElement(), ipv4, ipv6)
                }
            }
        } catch (error: Throwable) {
            LxLog.w(TAG, "Failed to enumerate local IP addresses", error)
        }

        return LocalIpAddresses(
            ipv4 = selectPrimaryAddress(ipv4),
            ipv6 = selectPrimaryAddress(ipv6),
        )
    }

    private fun collectInetAddress(inet: InetAddress, ipv4: MutableSet<String>, ipv6: MutableSet<String>) {
        when (inet) {
            is Inet4Address -> {
                val value = (inet.hostAddress ?: "").trim()
                if (value.isNotEmpty() && !inet.isLoopbackAddress && value != "0.0.0.0") {
                    ipv4.add(value)
                }
            }

            is Inet6Address -> {
                if (
                    inet.isLoopbackAddress ||
                        inet.isLinkLocalAddress ||
                        inet.isAnyLocalAddress ||
                        inet.isMulticastAddress
                ) {
                    return
                }
                val value = (inet.hostAddress ?: "").substringBefore('%').trim()
                if (value.isNotEmpty() && value != "::") {
                    ipv6.add(value)
                }
            }
        }
    }
}
