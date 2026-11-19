import DOMPurify from 'dompurify';

const MODDB_ORIGIN = 'https://mods.vintagestory.at';

const ALLOWED_TAGS = [
  'p',
  'br',
  'strong',
  'b',
  'em',
  'i',
  'u',
  's',
  'del',
  'h1',
  'h2',
  'h3',
  'h4',
  'h5',
  'h6',
  'ul',
  'ol',
  'li',
  'blockquote',
  'pre',
  'code',
  'a',
  'img',
  'hr',
  'table',
  'thead',
  'tbody',
  'tr',
  'th',
  'td',
  'div',
  'span',
  'details',
  'summary',
  'sup',
  'sub',
];

const ALLOWED_ATTR = [
  'href',
  'src',
  'alt',
  'title',
  'class',
  'target',
  'rel',
  'width',
  'height',
];

const FORBID_TAGS = [
  'script',
  'style',
  'iframe',
  'object',
  'embed',
  'form',
  'input',
  'textarea',
  'select',
  'button',
  'svg',
  'math',
  'base',
  'meta',
  'link',
];

export function isSafeHttpUrl(raw: string | null | undefined): string | null {
  const trimmed = raw?.trim();
  if (!trimmed) return null;
  try {
    const parsed = new URL(trimmed);
    if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:') {
      return null;
    }
    if (parsed.username || parsed.password) {
      return null;
    }
    return parsed.toString();
  } catch {
    return null;
  }
}

export function sanitizeModDbHtml(rawHtml: string): string {
  const rewritten = rawHtml
    .replace(/(src|href)=(["'])\/\//gi, '$1=$2https://')
    .replace(/(src|href)=(["'])\/(?!\/)/gi, `$1=$2${MODDB_ORIGIN}/`);

  const clean = DOMPurify.sanitize(rewritten, {
    ALLOWED_TAGS,
    ALLOWED_ATTR,
    FORBID_TAGS,
    ALLOW_DATA_ATTR: false,
    ALLOW_UNKNOWN_PROTOCOLS: false,
    ALLOWED_URI_REGEXP: /^(?:https?:\/\/|#)/i,
  });

  const doc = new DOMParser().parseFromString(clean, 'text/html');

  doc.body.querySelectorAll('a').forEach((anchor) => {
    const href = anchor.getAttribute('href')?.trim() ?? '';
    if (href && !href.startsWith('#')) {
      const safe = isSafeHttpUrl(href);
      if (safe) {
        anchor.setAttribute('href', safe);
      } else {
        anchor.removeAttribute('href');
      }
    }
    anchor.setAttribute('rel', 'noopener noreferrer');
  });

  doc.body.querySelectorAll('img').forEach((img) => {
    const src = img.getAttribute('src')?.trim() ?? '';
    const safe = isSafeHttpUrl(src);
    if (safe) {
      img.setAttribute('src', safe);
    } else {
      img.remove();
    }
  });

  return doc.body.innerHTML;
}

export function htmlToPlainText(raw: string | null | undefined): string {
  if (!raw || !raw.trim()) {
    return 'No changelog provided for this release.';
  }

  const clean = DOMPurify.sanitize(raw, {
    ALLOWED_TAGS: ['p', 'br', 'li', 'div', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6'],
    ALLOWED_ATTR: [],
    ALLOW_DATA_ATTR: false,
  });

  const doc = new DOMParser().parseFromString(clean, 'text/html');

  doc.body.querySelectorAll('br').forEach((br) => {
    br.replaceWith(doc.createTextNode('\n'));
  });

  doc.body.querySelectorAll('li').forEach((li) => {
    li.prepend(doc.createTextNode('• '));
    li.append(doc.createTextNode('\n'));
  });

  doc.body
    .querySelectorAll('p, div, h1, h2, h3, h4, h5, h6')
    .forEach((block) => {
      block.append(doc.createTextNode('\n\n'));
    });

  const text = (doc.body.textContent ?? '').replace(/\n{3,}/g, '\n\n').trim();
  return text || 'No changelog provided for this release.';
}
