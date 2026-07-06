package com.lingxia.lxapp.APIs

import android.Manifest
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.wifi.WifiConfiguration
import android.net.wifi.WifiManager
import android.net.wifi.WifiNetworkSpecifier
import android.os.Build
import android.util.Log
import androidx.core.content.ContextCompat
import com.lingxia.app.Lingxia
import com.lingxia.app.LxLog
import com.lingxia.lxapp.LxApp
import com.lingxia.app.NativeApi
import com.lingxia.app.PermissionManager
import org.json.JSONArray
import org.json.JSONObject

/**
 * WiFi management for Android
 *
 * Permissions required:
 * - ACCESS_WIFI_STATE
 * - CHANGE_WIFI_STATE
 * - ACCESS_FINE_LOCATION (for scanning on Android 6.0+)
 */
internal object LxAppWifi {
    private const val TAG = "LingXia.Wifi"
    private const val WIFI_CONNECT_TIMEOUT_MS = 30_000

    private val LOCATION_PERMISSIONS = arrayOf(
        Manifest.permission.ACCESS_FINE_LOCATION,
        Manifest.permission.ACCESS_COARSE_LOCATION,
    )

    private var wifiManager: WifiManager? = null
    private var connectivityManager: ConnectivityManager? = null
    private var scanResultsReceiver: BroadcastReceiver? = null
    private var pendingScanCallbackId: Long? = null

    // Multi-LxApp support: maintain a set of state listeners
    private val stateCallbacks = mutableSetOf<Long>()
    private var wifiNetworkCallback: ConnectivityManager.NetworkCallback? = null
    private var lastConnectedSignature: String? = null
    private var lastKnownConnected: Boolean? = null

    // Active network connection callback (Android 10+)
    private var activeNetworkCallback: ConnectivityManager.NetworkCallback? = null

    @Suppress("DEPRECATION")
    private fun isWifiConnected(connMgr: ConnectivityManager?): Boolean {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            val network = connMgr?.activeNetwork ?: return false
            val capabilities = connMgr.getNetworkCapabilities(network) ?: return false
            return capabilities.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)
        } else {
            val networkInfo = connMgr?.activeNetworkInfo ?: return false
            return networkInfo.isConnected && networkInfo.type == ConnectivityManager.TYPE_WIFI
        }
    }

    /**
     * Initialize WiFi module
     */
    @JvmStatic
    fun startWifi(callbackId: Long) {
        try {
            val context = Lingxia.applicationContext() ?: run {
                LxLog.e(TAG, "Context not available")
                NativeApi.onCallback(callbackId, false, "12001") // System error
                return
            }

            // Check basic WiFi permissions
            if (ContextCompat.checkSelfPermission(context, Manifest.permission.ACCESS_WIFI_STATE)
                != PackageManager.PERMISSION_GRANTED) {
                LxLog.e(TAG, "Missing ACCESS_WIFI_STATE permission")
                NativeApi.onCallback(callbackId, false, "12006") // Permission denied
                return
            }

            // Check location permission (required for WiFi details like frequency on Android 6.0+)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                if (ContextCompat.checkSelfPermission(context, Manifest.permission.ACCESS_FINE_LOCATION)
                    != PackageManager.PERMISSION_GRANTED) {
                    val activity = LxApp.getCurrentActivity()
                    if (activity == null) {
                        LxLog.w(TAG, "Cannot request ACCESS_FINE_LOCATION permission (no activity)")
                        // Continue anyway - WiFi will work but with limited info (no frequency)
                        initializeWifiModule(callbackId, context)
                        return
                    }
                    PermissionManager.ensurePermissions(activity, LOCATION_PERMISSIONS) { granted ->
                        if (granted) {
                            Log.i(TAG, "Location permission granted for WiFi details")
                        } else {
                            LxLog.w(TAG, "Location permission denied - WiFi info will be limited (no frequency)")
                        }
                        // Initialize WiFi module regardless of location permission
                        initializeWifiModule(callbackId, context)
                    }
                    return
                }
            }

            // Permission already granted or not needed
            initializeWifiModule(callbackId, context)
        } catch (e: Exception) {
            LxLog.e(TAG, "startWifi error", e)
            NativeApi.onCallback(callbackId, false, "12001") // System error
        }
    }

    private fun initializeWifiModule(callbackId: Long, context: Context) {
        try {
            // Get WiFi manager
            wifiManager = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager
            connectivityManager = context.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager

            if (wifiManager == null) {
                LxLog.e(TAG, "WiFi manager not available")
                NativeApi.onCallback(callbackId, false, "12001") // System error
                return
            }

            Log.i(TAG, "WiFi module initialized")
            NativeApi.onCallback(callbackId, true, "{}")
        } catch (e: Exception) {
            LxLog.e(TAG, "initializeWifiModule error", e)
            NativeApi.onCallback(callbackId, false, "12001") // System error
        }
    }


    /**
     * Stop WiFi module
     */
    @JvmStatic
    fun stopWifi(callbackId: Long) {
        try {
            // Unregister scan receiver if registered
            scanResultsReceiver?.let { receiver ->
                try {
                    Lingxia.applicationContext()?.unregisterReceiver(receiver)
                } catch (e: Exception) {
                    LxLog.w(TAG, "Failed to unregister scan receiver", e)
                }
            }
            scanResultsReceiver = null
            pendingScanCallbackId = null
            lastConnectedSignature = null
            lastKnownConnected = null

            // Clean up active network connection (Android 10+)
            activeNetworkCallback?.let { callback ->
                try {
                    connectivityManager?.unregisterNetworkCallback(callback)
                    Log.i(TAG, "Unregistered active network connection")
                } catch (e: Exception) {
                    LxLog.w(TAG, "Failed to unregister active network callback", e)
                }
            }
            activeNetworkCallback = null

            Log.i(TAG, "WiFi module stopped")
            NativeApi.onCallback(callbackId, true, "{}")
        } catch (e: Exception) {
            LxLog.e(TAG, "stopWifi error", e)
            NativeApi.onCallback(callbackId, false, "12001") // System error
        }
    }

    /**
     * Add a WiFi state listener (supports multiple LxApp instances)
     */
    @JvmStatic
    fun addWifiStateListener(callbackId: Long) {
        Log.i(TAG, "addWifiStateListener: callbackId=$callbackId")

        if (stateCallbacks.add(callbackId)) {
            Log.i(TAG, "Added WiFi state listener: $callbackId (total=${stateCallbacks.size})")

            val context = Lingxia.applicationContext() ?: return
            val connMgr = connectivityManager
                ?: (context.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager)
                    .also { connectivityManager = it }

            // First subscriber: start system WiFi monitoring
            if (stateCallbacks.size == 1) {
                lastConnectedSignature = null
                lastKnownConnected = null

                if (wifiNetworkCallback == null && connMgr != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
                    val callback = object : ConnectivityManager.NetworkCallback() {
                        override fun onAvailable(network: Network) {
                            super.onAvailable(network)
                            Log.i(TAG, "WiFi network available")
                            emitWifiConnectedToAll(null, null, true)
                        }

                        override fun onCapabilitiesChanged(
                            network: Network,
                            networkCapabilities: NetworkCapabilities
                        ) {
                            super.onCapabilitiesChanged(network, networkCapabilities)
                            if (networkCapabilities.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) {
                                Log.i(TAG, "WiFi capabilities changed (TRANSPORT_WIFI)")
                                emitWifiConnectedToAll(null, null, true)
                            }
                        }

                        override fun onLost(network: Network) {
                            super.onLost(network)
                            Log.i(TAG, "WiFi network lost")
                            emitWifiConnectedToAll(null, null, false)
                        }
                    }
                    wifiNetworkCallback = callback
                    try {
                        val request = NetworkRequest.Builder()
                            .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
                            .build()
                        connMgr.registerNetworkCallback(request, callback)
                        Log.i(TAG, "Registered system WiFi network callback")
                    } catch (e: Exception) {
                        LxLog.w(TAG, "Failed to register wifi network callback", e)
                    }
                }
            }

            // Send current state to new subscriber
            emitWifiConnected(callbackId, null, null, isWifiConnected(connMgr))
        } else {
            LxLog.w(TAG, "WiFi state listener already exists: $callbackId")
        }
    }

    /**
     * Remove a WiFi state listener
     */
    @JvmStatic
    fun removeWifiStateListener(callbackId: Long) {
        Log.i(TAG, "removeWifiStateListener: callbackId=$callbackId")

        if (stateCallbacks.remove(callbackId)) {
            Log.i(TAG, "Removed WiFi state listener: $callbackId (remaining=${stateCallbacks.size})")

            // Last subscriber: stop system WiFi monitoring
            if (stateCallbacks.isEmpty()) {
                wifiNetworkCallback?.let { existing ->
                    try {
                        connectivityManager?.unregisterNetworkCallback(existing)
                        Log.i(TAG, "Unregistered system WiFi network callback")
                    } catch (e: Exception) {
                        LxLog.w(TAG, "Failed to unregister wifi network callback", e)
                    }
                }
                wifiNetworkCallback = null
                lastConnectedSignature = null
                lastKnownConnected = null
            }
        } else {
            LxLog.w(TAG, "WiFi state listener not found: $callbackId")
        }
    }

    /**
     * Connect to WiFi access point
     */
    @JvmStatic
    fun connectWifi(callbackId: Long, ssid: String, password: String?) {
        try {
            val context = Lingxia.applicationContext() ?: run {
                LxLog.e(TAG, "Context not available")
                NativeApi.onCallback(callbackId, false, "12001") // System error
                return
            }

            val wifiMgr = wifiManager ?: run {
                LxLog.e(TAG, "WiFi manager not available")
                NativeApi.onCallback(callbackId, false, "12001") // System error
                return
            }

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            connectWifiAndroid10Plus(context, callbackId, ssid, password)
        } else {
                connectWifiLegacy(wifiMgr, callbackId, ssid, password)
            }
        } catch (e: Exception) {
            LxLog.e(TAG, "connectWifi error", e)
            NativeApi.onCallback(callbackId, false, "12001") // System error
        }
    }

    @Suppress("DEPRECATION")
    private fun normalizeSsid(ssid: String?): String {
        val value = ssid?.trim().orEmpty()
        return if (value.length >= 2 && value.first() == '"' && value.last() == '"') {
            value.substring(1, value.length - 1)
        } else {
            value
        }
    }

    @Suppress("DEPRECATION")
    private fun isAlreadyConnectedToSsid(wifiMgr: WifiManager, ssid: String): Boolean {
        val info = wifiMgr.connectionInfo ?: return false
        if (info.networkId == -1) return false
        return normalizeSsid(info.ssid) == normalizeSsid(ssid)
    }

    @Suppress("DEPRECATION")
    private fun findConfiguredNetworkId(wifiMgr: WifiManager, ssid: String): Int? {
        val target = normalizeSsid(ssid)
        val configured = wifiMgr.configuredNetworks ?: return null
        for (network in configured) {
            if (normalizeSsid(network.SSID) == target && network.networkId != -1) {
                return network.networkId
            }
        }
        return null
    }

    @Suppress("DEPRECATION")
    private fun connectWifiLegacy(
        wifiMgr: WifiManager,
        callbackId: Long,
        ssid: String,
        password: String?
    ) {
        try {
            if (isAlreadyConnectedToSsid(wifiMgr, ssid)) {
                Log.i(TAG, "Already connected to WiFi: $ssid")
                NativeApi.onCallback(callbackId, true, "{}")
                emitWifiConnectedToAll(ssid, password, true)
                return
            }

            val existingNetworkId = findConfiguredNetworkId(wifiMgr, ssid)
            if (existingNetworkId != null) {
                val disconnected = wifiMgr.disconnect()
                val enabled = wifiMgr.enableNetwork(existingNetworkId, true)
                val reconnected = wifiMgr.reconnect()
                Log.i(
                    TAG,
                    "Try reuse configured network id=$existingNetworkId ssid=$ssid disconnect=$disconnected enable=$enabled reconnect=$reconnected",
                )
                if (enabled && reconnected) {
                    NativeApi.onCallback(callbackId, true, "{}")
                    emitWifiConnectedToAll(ssid, password, null)
                    return
                }
            }

            val config = WifiConfiguration().apply {
                SSID = "\"$ssid\""

                if (password.isNullOrEmpty()) {
                    // Open network
                    allowedKeyManagement.set(WifiConfiguration.KeyMgmt.NONE)
                } else {
                    // WPA/WPA2
                    preSharedKey = "\"$password\""
                    allowedKeyManagement.set(WifiConfiguration.KeyMgmt.WPA_PSK)
                }
            }

            val networkId = try {
                wifiMgr.addNetwork(config)
            } catch (security: SecurityException) {
                LxLog.e(TAG, "Permission denied while adding network", security)
                NativeApi.onCallback(callbackId, false, "12006")
                return
            }
            if (networkId == -1) {
                LxLog.e(TAG, "Failed to add network configuration")
                NativeApi.onCallback(callbackId, false, "12006")
                return
            }

            val disconnected = wifiMgr.disconnect()
            val enabled = wifiMgr.enableNetwork(networkId, true)
            val reconnected = wifiMgr.reconnect()
            if (!enabled || !reconnected) {
                LxLog.e(TAG, "Failed to enable network")
                NativeApi.onCallback(callbackId, false, "12003") // Connection timeout
                return
            }

            Log.i(TAG, "Successfully connected to WiFi: $ssid disconnect=$disconnected enable=$enabled reconnect=$reconnected")
            NativeApi.onCallback(callbackId, true, "{}")
            emitWifiConnectedToAll(ssid, password, null)
        } catch (e: Exception) {
            LxLog.e(TAG, "connectWifiLegacy error", e)
            NativeApi.onCallback(callbackId, false, "12001") // System error
        }
    }

    private fun connectWifiAndroid10Plus(
        context: Context,
        callbackId: Long,
        ssid: String,
        password: String?
    ) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return

        try {
            val connMgr = connectivityManager ?: run {
                LxLog.e(TAG, "Connectivity manager not available")
                NativeApi.onCallback(callbackId, false, "12001") // System error
                return
            }

            // Unregister any existing active network callback first
            activeNetworkCallback?.let { existing ->
                try {
                    connMgr.unregisterNetworkCallback(existing)
                    Log.i(TAG, "Unregistered previous network connection")
                } catch (e: Exception) {
                    LxLog.w(TAG, "Failed to unregister previous network callback", e)
                }
            }

            // Build WiFi network specifier
            val specifierBuilder = WifiNetworkSpecifier.Builder()
                .setSsid(ssid)

            if (!password.isNullOrEmpty()) {
                specifierBuilder.setWpa2Passphrase(password)
            }

            val specifier = specifierBuilder.build()

            // Build network request
            val request = NetworkRequest.Builder()
                .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
                .setNetworkSpecifier(specifier)
                .build()

            // Network callback - keep it active to maintain connection
            val networkCallback = object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) {
                    super.onAvailable(network)
                    Log.i(TAG, "WiFi network became available: $ssid")
                    // Emit actual connection state via event listener
                    emitWifiConnectedToAll(ssid, password, true)
                    // DON'T unregister - keep the callback active to maintain connection
                }

                override fun onUnavailable() {
                    super.onUnavailable()
                    LxLog.e(TAG, "Failed to connect to WiFi (timeout): $ssid")
                    // Emit failure via event listener
                    emitWifiConnectedToAll(ssid, password, false)
                    try {
                        connMgr.unregisterNetworkCallback(this)
                    } catch (e: Exception) {
                        LxLog.w(TAG, "Failed to unregister connect callback", e)
                    }
                    activeNetworkCallback = null
                }

                override fun onLost(network: Network) {
                    super.onLost(network)
                    Log.i(TAG, "Lost connection to WiFi: $ssid")
                    activeNetworkCallback = null
                }
            }

            // Request network with timeout and save the callback reference
            connMgr.requestNetwork(request, networkCallback, WIFI_CONNECT_TIMEOUT_MS)
            activeNetworkCallback = networkCallback

            // Return success immediately - connection request submitted
            Log.i(TAG, "WiFi connection request submitted: $ssid")
            NativeApi.onCallback(callbackId, true, "{}")
        } catch (e: Exception) {
            LxLog.e(TAG, "connectWifiAndroid10Plus error", e)
            NativeApi.onCallback(callbackId, false, "12001") // System error
        }
    }

    /**
     * Emit WiFi connected event to a specific callback
     */
    private fun emitWifiConnected(callbackId: Long, ssidHint: String?, password: String?, connectedHint: Boolean? = null) {
        val context = Lingxia.applicationContext() ?: run {
            LxLog.e(TAG, "Context not available for wifi connected event")
            return
        }

        val wifiInfo = buildWifiInfoJson(context, ssidHint, password, connectedHint) ?: return

        Log.i(TAG, "emitWifiConnected: callbackId=$callbackId")
        val success = NativeApi.onCallback(callbackId, true, wifiInfo)
        if (!success) {
            LxLog.w(TAG, "Failed to dispatch wifi connected event to callback $callbackId")
        }
    }

    /**
     * Broadcast WiFi connected event to all subscribers
     */
    private fun emitWifiConnectedToAll(ssidHint: String?, password: String?, connectedHint: Boolean? = null) {
        if (stateCallbacks.isEmpty()) {
            return
        }

        val context = Lingxia.applicationContext() ?: run {
            LxLog.e(TAG, "Context not available for wifi connected event")
            return
        }

        val wifiInfo = buildWifiInfoJson(context, ssidHint, password, connectedHint) ?: return

        Log.i(TAG, "emitWifiConnectedToAll: ${stateCallbacks.size} subscribers")
        for (callbackId in stateCallbacks.toList()) {  // toList() to avoid concurrent modification
            val success = NativeApi.onCallback(callbackId, true, wifiInfo)
            if (!success) {
                LxLog.w(TAG, "Failed to dispatch wifi connected event to callback $callbackId")
            }
        }
    }

    /**
     * Build WiFi info JSON with deduplication
     */
    @Synchronized
    private fun buildWifiInfoJson(
        context: Context,
        ssidHint: String?,
        password: String?,
        connectedHint: Boolean?
    ): String? {
        val wifiMgr = wifiManager ?: run {
            context.applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager
        }

        val connectionInfo = wifiMgr?.connectionInfo
        val rawSsid = connectionInfo?.ssid?.removeSurrounding("\"")
        val resolvedSsid = when {
            !rawSsid.isNullOrEmpty() && rawSsid != "<unknown ssid>" -> rawSsid
            !ssidHint.isNullOrEmpty() -> ssidHint
            else -> ""
        }

        // Only consider connected if we have a valid SSID
        // If connectedHint is true but we can't get SSID (permissions issue), treat as disconnected
        val connected = when {
            resolvedSsid.isEmpty() -> false  // No SSID means not connected (or no permission)
            connectedHint != null -> connectedHint && resolvedSsid.isNotEmpty()
            else -> resolvedSsid.isNotEmpty()
        }

        val ssid = if (connected) resolvedSsid else ""
        val bssid = if (connected) connectionInfo?.bssid else null

        // Skip initial disconnected state
        if (!connected && lastKnownConnected == null && resolvedSsid.isEmpty() && connectedHint != true) {
            Log.i(TAG, "WiFi connected event skipped (initial disconnected)")
            return null
        }

        // Get frequency and signal strength before deduplication
        val rssi = if (connected) connectionInfo?.rssi ?: -100 else -100
        val frequency = if (connected && Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
            connectionInfo?.frequency
        } else {
            null
        }

        // Deduplication
        val frequencyKey = if (frequency != null && frequency > 0) frequency.toString() else ""
        val signature = "${if (connected) 1 else 0}|$ssid|${bssid ?: ""}|$frequencyKey"
        if (signature == lastConnectedSignature) {
            Log.i(TAG, "WiFi connected event deduped (signature=$signature)")
            return null
        }
        lastConnectedSignature = signature
        lastKnownConnected = connected
        val signalStrength = if (connected) {
            when {
                rssi >= -30 -> 100
                rssi <= -100 -> 0
                else -> ((rssi + 100) / 70.0 * 100).toInt()
            }.coerceIn(0, 100)
        } else {
            0
        }
        val secure = if (password != null) {
            password.isNotEmpty()
        } else {
            connected
        }

        val result = JSONObject().apply {
            put("ssid", ssid)
            if (bssid != null) {
                put("bssid", bssid)
            }
            put("secure", secure)
            put("signalStrength", signalStrength)
            put("connected", connected)
            put("state", if (connected) "connected" else "disconnected")
            if (frequency != null && frequency > 0) {
                put("frequency", frequency)
            }
        }

        return result.toString()
    }

    /**
     * Get WiFi list (scan results)
     */
    @JvmStatic
    fun getWifiList(callbackId: Long) {
        try {
            val context = Lingxia.applicationContext() ?: run {
                LxLog.e(TAG, "Context not available")
                NativeApi.onCallback(callbackId, false, "12001") // System error
                return
            }

            val wifiMgr = wifiManager ?: run {
                LxLog.e(TAG, "WiFi manager not available")
                NativeApi.onCallback(callbackId, false, "12001") // System error
                return
            }

            // Check location permission (required for WiFi scanning on Android 6.0+)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                if (ContextCompat.checkSelfPermission(context, Manifest.permission.ACCESS_FINE_LOCATION)
                    != PackageManager.PERMISSION_GRANTED) {
                    val activity = LxApp.getCurrentActivity()
                    if (activity == null) {
                        LxLog.e(TAG, "Missing ACCESS_FINE_LOCATION permission for WiFi scanning")
                        NativeApi.onCallback(callbackId, false, "12006") // Permission denied
                        return
                    }
                    PermissionManager.ensurePermissions(activity, LOCATION_PERMISSIONS) { granted ->
                        if (granted) {
                            getWifiList(callbackId)
                        } else {
                            LxLog.e(TAG, "Missing ACCESS_FINE_LOCATION permission for WiFi scanning")
                            NativeApi.onCallback(callbackId, false, "12006") // Permission denied
                        }
                    }
                    return
                }
            }

            // Register broadcast receiver for scan results
            scanResultsReceiver = object : BroadcastReceiver() {
                override fun onReceive(context: Context?, intent: Intent?) {
                    if (intent?.action == WifiManager.SCAN_RESULTS_AVAILABLE_ACTION) {
                        handleScanResults(wifiMgr, callbackId)
                        // Unregister after receiving results
                        try {
                            context?.unregisterReceiver(this)
                        } catch (e: Exception) {
                            LxLog.w(TAG, "Failed to unregister receiver", e)
                        }
                        scanResultsReceiver = null
                        pendingScanCallbackId = null
                    }
                }
            }

            pendingScanCallbackId = callbackId

            val filter = IntentFilter(WifiManager.SCAN_RESULTS_AVAILABLE_ACTION)
            context.registerReceiver(scanResultsReceiver, filter)

            // Start scan
            val scanStarted = wifiMgr.startScan()
            if (!scanStarted) {
                LxLog.w(TAG, "WiFi scan may be throttled (Android 9+)")
                // Try to get cached results
                handleScanResults(wifiMgr, callbackId)
                scanResultsReceiver?.let { receiver ->
                    try {
                        context.unregisterReceiver(receiver)
                    } catch (e: Exception) {
                        LxLog.w(TAG, "Failed to unregister receiver", e)
                    }
                }
                scanResultsReceiver = null
                pendingScanCallbackId = null
            }
        } catch (e: Exception) {
            LxLog.e(TAG, "getWifiList error", e)
            NativeApi.onCallback(callbackId, false, "12001") // System error
        }
    }

    @Suppress("DEPRECATION")
    private fun handleScanResults(wifiMgr: WifiManager, callbackId: Long) {
        try {
            val scanResults = wifiMgr.scanResults
            val wifiList = JSONArray()

            for (result in scanResults) {
                // Convert signal strength from RSSI (dBm) to 0-100 scale
                val rssi = result.level
                val signalStrength = when {
                    rssi >= -30 -> 100
                    rssi <= -100 -> 0
                    else -> ((rssi + 100) / 70.0 * 100).toInt()
                }.coerceIn(0, 100)

                val wifiInfo = JSONObject().apply {
                    put("ssid", result.SSID)
                    put("bssid", result.BSSID)
                    put("secure", result.capabilities.contains("WPA") ||
                            result.capabilities.contains("WEP") ||
                            result.capabilities.contains("PSK"))
                    put("signalStrength", signalStrength)
                    put("frequency", result.frequency)
                }

                wifiList.put(wifiInfo)
            }

            Log.i(TAG, "Found ${wifiList.length()} WiFi networks")
            NativeApi.onCallback(callbackId, true, wifiList.toString())
        } catch (e: Exception) {
            LxLog.e(TAG, "handleScanResults error", e)
            NativeApi.onCallback(callbackId, false, "12001") // System error
        }
    }

    /**
     * Check if WiFi is enabled on the device (synchronous)
     */
    @JvmStatic
    fun isWifiEnabled(): Boolean {
        return try {
            val context = Lingxia.applicationContext() ?: return false
            val wifiMgr = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager
            wifiMgr?.isWifiEnabled ?: false
        } catch (e: Exception) {
            LxLog.e(TAG, "isWifiEnabled error", e)
            false
        }
    }

    /**
     * Get connected WiFi info
     */
    @Suppress("DEPRECATION")
    @JvmStatic
    fun getConnectedWifi(callbackId: Long) {
        try {
            val context = Lingxia.applicationContext() ?: run {
                LxLog.e(TAG, "Context not available")
                NativeApi.onCallback(callbackId, false, "12001") // System error
                return
            }

            val wifiMgr = (wifiManager
                ?: context.applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager)
                ?.also { wifiManager = it }
                ?: run {
                LxLog.e(TAG, "WiFi manager not available")
                NativeApi.onCallback(callbackId, false, "12001") // System error
                return
            }

            val connMgr = (connectivityManager
                ?: context.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager)
                ?.also { connectivityManager = it }

            val connectionInfo = try {
                wifiMgr.connectionInfo
            } catch (e: SecurityException) {
                LxLog.w(TAG, "getConnectedWifi: permission restricted, fallback to empty SSID", e)
                null
            } catch (e: Throwable) {
                LxLog.w(TAG, "getConnectedWifi: failed to read WifiInfo", e)
                null
            }

            val rawSsid = try {
                connectionInfo?.ssid
            } catch (e: Throwable) {
                LxLog.w(TAG, "getConnectedWifi: failed to read SSID", e)
                null
            }
            val ssid = rawSsid
                ?.removeSurrounding("\"")
                ?.takeIf { it.isNotEmpty() && it != "<unknown ssid>" }
                ?: ""
            val bssid = try {
                connectionInfo?.bssid
            } catch (e: Throwable) {
                LxLog.w(TAG, "getConnectedWifi: failed to read BSSID", e)
                null
            }?.takeIf { it.isNotEmpty() && it != "02:00:00:00:00:00" }

            // Android 8/9 may return "<unknown ssid>" when location is unavailable.
            // In that case, keep API successful and return empty SSID to avoid hard failures.
            val connected = isWifiConnected(connMgr) || ssid.isNotEmpty()

            // Convert signal strength
            val rssi = connectionInfo?.rssi ?: -100
            val signalStrength = if (connected) {
                when {
                    rssi >= -30 -> 100
                    rssi <= -100 -> 0
                    else -> ((rssi + 100) / 70.0 * 100).toInt()
                }.coerceIn(0, 100)
            } else {
                0
            }
            val frequency = if (connected && Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
                try {
                    connectionInfo?.frequency
                } catch (_: Throwable) {
                    null
                }
            } else {
                null
            }

            val result = JSONObject().apply {
                put("ssid", if (connected) ssid else "")
                if (connected && bssid != null) {
                    put("bssid", bssid)
                }
                put("secure", connected)
                put("signalStrength", signalStrength)
                if (frequency != null && frequency > 0) {
                    put("frequency", frequency)
                }
            }

            Log.i(TAG, "getConnectedWifi resolved: connected=$connected ssid=${if (ssid.isEmpty()) "<empty>" else ssid}")
            NativeApi.onCallback(callbackId, true, result.toString())
        } catch (e: Throwable) {
            LxLog.e(TAG, "getConnectedWifi error", e)
            NativeApi.onCallback(callbackId, false, "12001") // System error
        }
    }
}
