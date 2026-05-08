// Small rendering helpers shared across sections.

// Format a hex hash with subtle whitespace grouping every 8 chars.
// Returns an HTMLElement <span class="hash"> with click-to-copy.
export function hashChip(hex, { title } = {}) {
  const norm = hex.startsWith("0x") || hex.startsWith("0X") ? hex.slice(2) : hex;
  const span = document.createElement("span");
  span.className = "hash";
  span.dataset.value = norm;
  if (title) span.title = title;
  for (let i = 0; i < norm.length; i += 8) {
    const grp = document.createElement("span");
    grp.className = "grp";
    grp.textContent = norm.slice(i, i + 8);
    span.appendChild(grp);
  }
  copyOnClick(span, norm);
  return span;
}

// Attach copy-to-clipboard behavior with a brief flash.
export function copyOnClick(el, text) {
  el.addEventListener("click", async (e) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // Older browsers may need a fallback; the bytes are still
      // selectable via the user-select: all rule in the stylesheet.
      return;
    }
    el.classList.add("flash");
    setTimeout(() => el.classList.remove("flash"), 280);
  });
}

// Tiny syntax highlighter for JSON. Escapes HTML, then replaces tokens
// with span-wrapped versions. Matches strings (incl. keys), numbers,
// booleans, null. No fancy AST.
export function highlightJson(jsonText) {
  const escaped = jsonText
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
  // Keys vs strings: a string immediately followed by `:` is a key.
  return escaped.replace(
    /("(?:\\.|[^"\\])*")(\s*:)?|\b(true|false|null)\b|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)/g,
    (match, str, colon, bool, num) => {
      if (str) {
        const cls = colon ? "tok-key" : "tok-string";
        return `<span class="${cls}">${str}</span>${colon || ""}`;
      }
      if (bool) {
        const cls = bool === "null" ? "tok-null" : "tok-bool";
        return `<span class="${cls}">${bool}</span>`;
      }
      if (num) return `<span class="tok-number">${num}</span>`;
      return match;
    },
  );
}

// Pretty-print a JSON-serializable value, then highlight.
export function highlightedJson(value) {
  return highlightJson(JSON.stringify(value, null, 2));
}

export function el(tag, props = {}, children = []) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k === "class") node.className = v;
    else if (k === "html") node.innerHTML = v;
    else if (k === "text") node.textContent = v;
    else if (k.startsWith("on") && typeof v === "function") {
      node.addEventListener(k.slice(2).toLowerCase(), v);
    } else if (k === "attrs") {
      for (const [ak, av] of Object.entries(v)) node.setAttribute(ak, av);
    } else {
      node[k] = v;
    }
  }
  for (const c of [].concat(children)) {
    if (c == null) continue;
    if (typeof c === "string") node.appendChild(document.createTextNode(c));
    else node.appendChild(c);
  }
  return node;
}

// Format a millisecond unix timestamp to a UTC ISO-ish string.
export function formatUtc(ms) {
  const d = new Date(ms);
  const pad = (n, w = 2) => String(n).padStart(w, "0");
  return `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())} ${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())}:${pad(d.getUTCSeconds())} UTC`;
}
