// Entry of the `<script>` build (`@lingxia/html/global`): the same page API as
// the module. The host already booted the bridge, so the build aliases
// `@lingxia/bridge` to its side-effect-free parts.
export { pageReady, getPage, subscribePage } from "./page.js";
export { getHost, subscribeHost } from "@lingxia/bridge";
