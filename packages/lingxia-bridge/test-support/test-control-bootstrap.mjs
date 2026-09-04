import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import vm from 'node:vm';

const contentUrl = new URL('../../../crates/lingxia-lxapp/src/lxapp/content.rs', import.meta.url);
const browserInternalPagesUrl = new URL(
  '../../../crates/lingxia-browser/src/internal_pages.rs',
  import.meta.url,
);
const [contentSource, browserInternalPages] = await Promise.all([
  readFile(contentUrl, 'utf8'),
  readFile(browserInternalPagesUrl, 'utf8'),
]);

function bootstrapTemplate(source) {
  const functionStart = source.indexOf('fn build_control_document_bootstrap_script');
  assert.notEqual(functionStart, -1, 'missing bootstrap script generator');
  const rawStart = source.indexOf('r#"', functionStart);
  const rawEnd = source.indexOf('"#,', rawStart);
  assert.notEqual(rawStart, -1, 'missing bootstrap raw string');
  assert.notEqual(rawEnd, -1, 'unterminated bootstrap raw string');
  return source.slice(rawStart + 3, rawEnd);
}

function renderBootstrap(sessionId, secret) {
  const escapeRustJsString = (value) => value
    .replaceAll('\\', '\\\\')
    .replaceAll('"', '\\"')
    .replaceAll('\n', '\\n')
    .replaceAll('\r', '\\r')
    .replaceAll('\t', '\\t');
  return bootstrapTemplate(contentSource)
    .replaceAll('{{', '{')
    .replaceAll('}}', '}')
    .replace('{}', escapeRustJsString(secret))
    .replace('{}', escapeRustJsString(sessionId));
}

const secret = 'control-secret-not-in-runtime-config';
const sessionId = 'control-session-public-id';
const html = renderBootstrap(sessionId, secret);
assert.match(html, /^<script>.*<\/script>$/);
const inlineSource = html.slice('<script>'.length, -'</script>'.length);
const window = {
  __LX_BRIDGE_CFG: { os: 'macOS' },
  __LX_RUNTIME_CONFIG: { error: { reason: 'test' } },
};
vm.runInNewContext(inlineSource, { window });

const descriptor = Object.getOwnPropertyDescriptor(window, '__LingXiaTakeControlBootstrap');
assert.equal(descriptor?.enumerable, false);
assert.equal(typeof descriptor?.value, 'function');
const take = descriptor.value;
const first = take();
assert.deepEqual(JSON.parse(JSON.stringify(first)), {
  requiredProtocol: 3,
  publicSessionId: sessionId,
  secret,
});
assert.equal(window.__LingXiaTakeControlBootstrap, undefined);
assert.equal(take(), undefined);
assert.doesNotMatch(JSON.stringify(window.__LX_BRIDGE_CFG), new RegExp(secret));
assert.doesNotMatch(JSON.stringify(window.__LX_RUNTIME_CONFIG), new RegExp(secret));

const ordinaryGenerator = contentSource.slice(
  contentSource.indexOf('pub fn generate_page_html('),
  contentSource.indexOf('pub fn generate_page_html_with_bridge_bootstrap('),
);
const errorHtmlGenerator = contentSource.slice(
  contentSource.indexOf('fn get_404_page('),
  contentSource.indexOf('fn inject_content_security_policy('),
);
assert.doesNotMatch(ordinaryGenerator, /__LingXiaTakeControlBootstrap|ControlDocumentBootstrap/);
assert.doesNotMatch(errorHtmlGenerator, /__LingXiaTakeControlBootstrap|ControlDocumentBootstrap/);

const schemeHandler = browserInternalPages.slice(
  browserInternalPages.indexOf('pub(crate) async fn handle_browser_lingxia_scheme'),
  browserInternalPages.indexOf('pub(crate) async fn browser_attach_tab_page'),
);
const unsupportedFallback = browserInternalPages.slice(
  browserInternalPages.indexOf('Err(WebViewError::Unsupported(_)) =>'),
  browserInternalPages.indexOf('Err(error) =>', browserInternalPages.indexOf('Err(WebViewError::Unsupported(_)) =>')),
);
const trustedHtmlFailure = browserInternalPages.slice(
  browserInternalPages.indexOf('let html = match browser.generate_page_html_with_bridge_bootstrap('),
  browserInternalPages.indexOf('let html = String::from_utf8_lossy(&html);'),
);
assert.doesNotMatch(schemeHandler, /generate_page_html_with_bridge_bootstrap/);
assert.doesNotMatch(unsupportedFallback, /generate_page_html_with_bridge_bootstrap|__LingXiaTakeControlBootstrap/);
assert.match(trustedHtmlFailure, /documents\.revoke_if_matches\(\s*webview\.native_view_id\(\),\s*session_id,\s*create_token,\s*intent,/s);
assert.doesNotMatch(trustedHtmlFailure, /reservation\.load\(/);

console.log('control bootstrap: one-shot handoff and non-trusted paths verified');
