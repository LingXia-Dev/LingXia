import { getPlatformOS } from './runtime-env';

const PROXY_PREFIX = 'lx://proxy/';

function shouldProxyImageUrl(trimmed: string): boolean {
  if (trimmed.length === 0) {
    return false;
  }
  if (
    trimmed.startsWith('lx://') ||
    trimmed.startsWith('data:') ||
    trimmed.startsWith('blob:') ||
    trimmed.startsWith('file:')
  ) {
    return false;
  }
  return trimmed.startsWith('http://') || trimmed.startsWith('https://');
}

function toProxyUrl(url: string): string {
  const b64 = btoa(url);
  const urlSafe = b64.replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
  return `${PROXY_PREFIX}${urlSafe}`;
}

function proxyImageUrl(url: string): string {
  const trimmed = url.trim();
  if (!shouldProxyImageUrl(trimmed)) return url;
  return toProxyUrl(trimmed);
}

export function setupImageProxy(): void {
  const os = getPlatformOS();
  if (os !== 'iOS' && os !== 'macOS') return;
  if (typeof window === 'undefined' || typeof document === 'undefined') return;
  if (typeof HTMLImageElement === 'undefined') return;

  const proto = HTMLImageElement.prototype as HTMLImageElement & {
    __lxProxyPatched?: boolean;
  };
  if (proto.__lxProxyPatched) return;
  proto.__lxProxyPatched = true;

  const originalSetAttribute = proto.setAttribute;
  proto.setAttribute = function (name: string, value: string): void {
    if (typeof name === 'string' && name.toLowerCase() === 'src' && typeof value === 'string') {
      return originalSetAttribute.call(this, name, proxyImageUrl(value));
    }
    return originalSetAttribute.call(this, name, value);
  };

  const srcDescriptor = Object.getOwnPropertyDescriptor(proto, 'src');
  if (srcDescriptor?.set) {
    Object.defineProperty(proto, 'src', {
      configurable: srcDescriptor.configurable,
      enumerable: srcDescriptor.enumerable,
      get: srcDescriptor.get,
      set(value: string) {
        if (typeof value === 'string') {
          srcDescriptor.set?.call(this, proxyImageUrl(value));
        } else {
          srcDescriptor.set?.call(this, value);
        }
      },
    });
  }

  const rewriteExisting = () => {
    const imgs = document.querySelectorAll<HTMLImageElement>('img[src]');
    imgs.forEach((img) => {
      const src = img.getAttribute('src');
      if (!src) return;
      const proxied = proxyImageUrl(src);
      if (proxied !== src) {
        originalSetAttribute.call(img, 'src', proxied);
      }
    });
  };

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', rewriteExisting);
  } else {
    rewriteExisting();
  }
}
