export function isHttpUrl(value: string): boolean {
  const normalized = value.trim().toLowerCase();
  return normalized.startsWith('https://') || normalized.startsWith('http://');
}

export function isFileUrl(value: string): boolean {
  const normalized = value.trim();
  const schemeEnd = normalized.indexOf(':');
  return schemeEnd > 0 && normalized.substring(0, schemeEnd).toLowerCase() === 'file';
}

export function isSupportedUrl(value: string, urlCallback: boolean): boolean {
  return isHttpUrl(value) || (!urlCallback && isFileUrl(value));
}

export function isSameOrigin(baseUrl: string, targetUrl: string): boolean {
  const baseOrigin = originOf(baseUrl);
  const targetOrigin = originOf(targetUrl);
  return !!baseOrigin && baseOrigin === targetOrigin;
}

function originOf(value: string): string {
  const normalized = value.trim().toLowerCase();
  const schemeEnd = normalized.indexOf('://');
  if (schemeEnd <= 0) {
    return '';
  }
  const scheme = normalized.substring(0, schemeEnd);
  let authority = normalized.substring(schemeEnd + 3);
  const pathStart = authority.search(/[/?#]/);
  if (pathStart >= 0) {
    authority = authority.substring(0, pathStart);
  }
  if (!authority) {
    return '';
  }
  const at = authority.lastIndexOf('@');
  if (at >= 0) {
    authority = authority.substring(at + 1);
  }
  if (authority.startsWith('[')) {
    const close = authority.indexOf(']');
    if (close > 0) {
      const host = authority.substring(0, close + 1);
      const portPart = authority.substring(close + 1);
      const port = portPart.startsWith(':') ? portPart.substring(1) : '';
      return `${scheme}://${host}:${normalizePort(scheme, port)}`;
    }
  }
  const colon = authority.lastIndexOf(':');
  const host = colon > 0 ? authority.substring(0, colon) : authority;
  const port = colon > 0 ? authority.substring(colon + 1) : '';
  return `${scheme}://${host}:${normalizePort(scheme, port)}`;
}

function normalizePort(scheme: string, port: string): string {
  if (port) {
    return port;
  }
  if (scheme === 'http') {
    return '80';
  }
  if (scheme === 'https') {
    return '443';
  }
  return '';
}
