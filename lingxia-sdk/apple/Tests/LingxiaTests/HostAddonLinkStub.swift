// SwiftPM's package test executable has no product host extension to provide
// the registrar that the SDK intentionally force-links in real host apps.
@_cdecl("lingxia_register_host_addon")
func lingxiaTestRegisterHostAddon() {}
