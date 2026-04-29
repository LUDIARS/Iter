# Third-Party Licenses

Iter is licensed under the MIT License (see [LICENSE](LICENSE)).

This document lists the third-party software bundled with, or required at
runtime by, Iter, together with the SPDX identifier of each license and a
link to its canonical license text. Where a package is dual-licensed,
Iter relies on it under one of the listed licenses; the user may rely on
either.

For Apache-2.0 packages, see [NOTICE](NOTICE) for the attribution notice
required by section 4(d) of the Apache 2.0 license.

## Frontend (npm)

| Package | Version range | License (SPDX) | Source |
|---|---|---|---|
| `monaco-editor` | ^0.52 | MIT | <https://github.com/microsoft/monaco-editor/blob/main/LICENSE.md> |
| `@monaco-editor/react` | ^4.7 | MIT | <https://github.com/suren-atoyan/monaco-react/blob/master/LICENSE> |
| `@xyflow/react` (React Flow) | ^12.3 | MIT | <https://github.com/xyflow/xyflow/blob/main/LICENSE> |
| `react`, `react-dom` | ^19 | MIT | <https://github.com/facebook/react/blob/main/LICENSE> |
| `@tauri-apps/api` | ^2.1 | Apache-2.0 OR MIT | <https://github.com/tauri-apps/tauri/blob/dev/LICENSE_MIT> |
| `@tauri-apps/cli` | ^2.1 | Apache-2.0 OR MIT | <https://github.com/tauri-apps/tauri/blob/dev/LICENSE_MIT> |
| `@tauri-apps/plugin-dialog` | ^2.0 | Apache-2.0 OR MIT | <https://github.com/tauri-apps/plugins-workspace> |
| `@tauri-apps/plugin-fs` | ^2.0 | Apache-2.0 OR MIT | <https://github.com/tauri-apps/plugins-workspace> |
| `@vitejs/plugin-react` | ^4 | MIT | <https://github.com/vitejs/vite-plugin-react/blob/main/LICENSE> |
| `vite` | ^6 | MIT | <https://github.com/vitejs/vite/blob/main/LICENSE> |
| `vitest` | ^3 | MIT | <https://github.com/vitest-dev/vitest/blob/main/LICENSE> |
| `typescript` | ^5.7 | Apache-2.0 | <https://github.com/microsoft/TypeScript/blob/main/LICENSE.txt> |
| `@types/*` | (transitive) | MIT | DefinitelyTyped repository |

## Backend (cargo)

| Crate | Version range | License (SPDX) | Source |
|---|---|---|---|
| `tauri` | 2 | Apache-2.0 OR MIT | <https://github.com/tauri-apps/tauri> |
| `tauri-build` | 2 | Apache-2.0 OR MIT | <https://github.com/tauri-apps/tauri> |
| `tauri-plugin-dialog` | 2 | Apache-2.0 OR MIT | <https://github.com/tauri-apps/plugins-workspace> |
| `tauri-plugin-fs` | 2 | Apache-2.0 OR MIT | <https://github.com/tauri-apps/plugins-workspace> |
| `serde`, `serde_json` | 1 | Apache-2.0 OR MIT | <https://github.com/serde-rs/serde> |
| `tokio` | 1 | MIT | <https://github.com/tokio-rs/tokio/blob/master/LICENSE> |
| `thiserror` | 2 | Apache-2.0 OR MIT | <https://github.com/dtolnay/thiserror> |
| `lsp-types` | 0.97 | MIT | <https://github.com/gluon-lang/lsp-types/blob/master/LICENSE> |
| `url` | 2 | Apache-2.0 OR MIT | <https://github.com/servo/rust-url> |
| `which` | 7 | MIT | <https://github.com/harryfei/which-rs/blob/master/LICENSE.txt> |

(Transitive crate dependencies inherit the licenses declared in their
respective `Cargo.toml` and the upstream crate registry. A full
machine-readable inventory can be regenerated with `cargo about generate`
or `cargo deny check licenses`.)

## Runtime tooling (not bundled)

| Tool | License (SPDX) | Source |
|---|---|---|
| **clangd** (LSP server, invoked as a subprocess when present in `PATH`) | Apache-2.0 WITH LLVM-exception | <https://github.com/clangd/clangd> |
| **CMake** (used to generate `compile_commands.json` when present in `PATH`) | BSD-3-Clause | <https://gitlab.kitware.com/cmake/cmake/-/blob/master/Copyright.txt> |

Iter neither redistributes nor links statically against clangd or CMake.
Users install them separately and Iter shells out to whichever copy is
on `PATH`.

## License texts

Full canonical license texts are not duplicated here to keep this file
maintainable; follow the "Source" links above. When distributing Iter
binaries, the per-package `LICENSE` files installed under `node_modules/`
and the cargo registry cache satisfy the "include a copy of the License"
requirement of Apache 2.0 section 4(a).
