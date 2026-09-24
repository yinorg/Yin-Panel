# @yin-panel/create-theme

Starter and package metadata validation tools for Yin-Panel Theme API v1.

```sh
npx @yin-panel/create-theme init "Editorial Desk" --author "Your Name"
```

Within the Yin-Panel source workspace, the current command is:

```sh
npm run create:theme -- init "Editorial Desk" --author "Your Name" --directory ./editorial-desk
npm run create:theme -- validate ./editorial-desk
npm run create:theme -- build ./editorial-desk
npm run create:theme -- test ./editorial-desk
npm run create:theme -- pack ./editorial-desk
npm run create:theme -- preview ./editorial-desk --open
npm run create:theme -- dev ./editorial-desk
```

`init` creates a format v2 package with light and dark DTCG 2025.10 documents, a sandbox Theme API v1 home view, stylesheet, read-only permissions, and resource digests. It refuses to overwrite an existing directory.

`validate` checks package identity, API and DTCG declarations, token document schema URLs, safe file paths, entrypoint declarations, media types, resource SHA-256 values, and rejects undeclared files. `build` bundles the JavaScript entrypoint for browsers, refreshes its digest, validates the output package, and writes an immutable build directory outside the source. `test` runs that build and output validation in a temporary directory. `pack` writes a deterministic ZIP containing only the declared package closure and refuses to overwrite an existing archive. Author metadata files such as `README.md` and `LICENSE` stay in the source directory but are not included in the runtime archive. Yin Core remains authoritative for full DTCG validation, compatibility checks, and package installation.

`preview` builds and packs the source, uploads it to the Core administrator preview endpoint, and prints a URL that opens the package in the existing sandboxed Theme runtime. `dev` does the same initially, watches source files, and prints a fresh preview URL after each change. Set `YIN_THEME_AUTH_TOKEN` to an administrator JWT and optionally `YIN_THEME_CORE_URL` (default `http://localhost:3002`). Preview requires an administrator account; Core preview tokens expire after 10 minutes. Plain HTTP is accepted only for loopback Core URLs; remote Core URLs must use HTTPS. `--mode light|dark` selects the preview scheme, and `--open` opens each generated URL in the default browser.

The generated view uses only the framework-independent Theme API. It does not import Yin-Panel Vue components or query Core DOM. Replace the example view, styles, tokens, author metadata, and license before publishing.
