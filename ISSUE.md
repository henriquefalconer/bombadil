# External module scripts fail to load (empty MIME type)

`<script type="module" src="...">` tags never execute under Bombadil. The browser reports:

```
Failed to load module script: Expected a JavaScript-or-Wasm module script
but the server responded with a MIME type of "". Strict MIME type checking
is enforced for module scripts per HTML spec.
```

The server responds with `Content-Type: text/javascript` (verified via `curl -sI`). The same pages work in any normal browser.

## Reproduction

**script.js**
```js
document.getElementById("result").textContent = "LOADED";
```

**test.html**
```html
<!DOCTYPE html>
<html>
<body>
  <h1 id="result">WAITING...</h1>
  <script type="module" src="/script.js"></script>
</body>
</html>
```

**spec.ts**
```ts
import { actions } from "@antithesishq/bombadil";
export const fallback = actions(() => [
  { ScrollDown: { origin: { x: 0, y: 0 }, distance: 1 } },
]);
```

```bash
python3 -m http.server 9999 &
bombadil test http://localhost:9999/test.html ./spec.ts
```

Page stays at "WAITING...". Changing the script tag to `<script src="/script.js"></script>` (no `type="module"`) makes it work.

## What works vs what doesn't

| Variant | Result |
|---------|--------|
| `<script>alert(1)</script>` | Pass |
| `<script type="module">alert(1)</script>` | Pass |
| `<script src="/script.js"></script>` | Pass |
| `<script type="module" src="/script.js"></script>` | **Fail** |

The issue also reproduces with `bombadil test-external` connected to a system Chrome via `--remote-debugging-port=9222`, so it is not specific to the bundled Chromium.

## Impact

Any app built with Vite, Webpack 5 (with `output.module`), or any other tool that emits `<script type="module">` cannot be tested with Bombadil.

## Environment

- bombadil: latest release (downloaded 2025-02-24)
- macOS (Apple Silicon)
- Server: Python `http.server` (also reproduced with Vite dev server and Frappe/Werkzeug)
