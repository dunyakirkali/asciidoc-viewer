# Feasibility Research

The initial investigation below was source inspection only. Research subagents failed with `You have no credits remaining`; inspection continued directly using the GitHub CLI. A subsequent executable processor gate is now implemented and passing; see [the design notes](design.md#processor-gate-result). No GPUI viewer has been implemented yet.

## Rust processors

These are existing Rust parser libraries, not proposals to run Ruby or Node subprocesses. Versions below are source-manifest versions, not independently verified published releases.

| Candidate | Verified seams | Main concern |
| --- | --- | --- |
| `asciidoc-parser` | Public block AST, inline AST via `Content::inlines()`, source spans via `HasSpan`, and pre-substitution text via `Content::original()`. `Parser::parse` returns an owned document and reports warnings rather than returning a parse error. Manifest: 0.31.2, Rust 1.88+, MIT OR Apache-2.0. | Preprocessing happens before document construction; AST spans and inline substitutions do not by themselves guarantee a byte-for-byte representation of every original construct. |
| `acdc-parser` | Public `ParseResult::document()`, warnings, source-recovery warnings, and source locations. `Location` documents original-source-relative byte offsets with cross-file caveats. Manifest: 0.9.0, MIT OR Apache-2.0. Network include support is an optional feature. | `ParseResult::source()` explicitly returns preprocessed text, not caller input; preprocessing may remove or transform content. Public parse entry points return `Result`. |
| `asciidork-parser` | Separate AST crate, block source locations, and byte start/end locations with include depth. Manifest: 0.38.2, MIT. | README explicitly warns of unfinished behavior and error handling. Source locations are useful, but exact raw-source recovery was not established. |

### Primary sources

- `asciidoc-parser` snapshot: [`c4ea28b`](https://github.com/asciidoc-rs/asciidoc-parser/tree/c4ea28b3cd6e3a310d123eebeddb991d882f865f).
  - [Manifest](https://github.com/asciidoc-rs/asciidoc-parser/blob/c4ea28b3cd6e3a310d123eebeddb991d882f865f/parser/Cargo.toml).
  - [Block AST](https://github.com/asciidoc-rs/asciidoc-parser/blob/c4ea28b3cd6e3a310d123eebeddb991d882f865f/parser/src/blocks/block.rs).
  - [Inline AST](https://github.com/asciidoc-rs/asciidoc-parser/blob/c4ea28b3cd6e3a310d123eebeddb991d882f865f/parser/src/inlines/inline_node.rs): `Text` values already have attribute references expanded; `Raw` can represent processed output rather than original source.
  - [Content APIs](https://github.com/asciidoc-rs/asciidoc-parser/blob/c4ea28b3cd6e3a310d123eebeddb991d882f865f/parser/src/content/content.rs): `original()` and `inlines()`.
  - [Span API](https://github.com/asciidoc-rs/asciidoc-parser/blob/c4ea28b3cd6e3a310d123eebeddb991d882f865f/parser/src/span/mod.rs).
  - [Parser](https://github.com/asciidoc-rs/asciidoc-parser/blob/c4ea28b3cd6e3a310d123eebeddb991d882f865f/parser/src/parser/parser.rs): preprocessing, include-handler configuration, default secure mode, and warning behavior.
- `acdc-parser` snapshot: [`c5e746d`](https://github.com/nlopes/acdc/tree/c5e746d00afc6cc5ed5e684ba8697573b594cf4f).
  - [Manifest](https://github.com/nlopes/acdc/blob/c5e746d00afc6cc5ed5e684ba8697573b594cf4f/acdc-parser/Cargo.toml).
  - [Parse result](https://github.com/nlopes/acdc/blob/c5e746d00afc6cc5ed5e684ba8697573b594cf4f/acdc-parser/src/parsed.rs).
  - [Locations](https://github.com/nlopes/acdc/blob/c5e746d00afc6cc5ed5e684ba8697573b594cf4f/acdc-parser/src/model/location.rs).
  - [Parse entry points](https://github.com/nlopes/acdc/blob/c5e746d00afc6cc5ed5e684ba8697573b594cf4f/acdc-parser/src/lib.rs).
- Asciidork snapshot: [`79671b5`](https://github.com/jaredh159/asciidork/tree/79671b57923841e41fc91d2c1d2fce18cca3131c).
  - [README caveats](https://github.com/jaredh159/asciidork/blob/79671b57923841e41fc91d2c1d2fce18cca3131c/readme.md).
  - [Parser manifest](https://github.com/jaredh159/asciidork/blob/79671b57923841e41fc91d2c1d2fce18cca3131c/parser/Cargo.toml).
  - [Block AST](https://github.com/jaredh159/asciidork/blob/79671b57923841e41fc91d2c1d2fce18cca3131c/ast/src/block.rs) and [source location](https://github.com/jaredh159/asciidork/blob/79671b57923841e41fc91d2c1d2fce18cca3131c/ast/src/source_location.rs).

### Recommended next check, not an accepted architecture

Evaluate `asciidoc-parser` first. Its structured inline tree appears suitable for direct native rendering without consuming HTML. A small executable check must establish the agreed subset and exact raw-source fallback, including unsupported metadata/delimiters and preprocessing directives. Keep the original input available; do not assume a semantic AST is a lossless syntax tree.

The design must distinguish a **limited renderer** from a **limited processor**. Reusing a processor that already expands attributes may be simpler than deliberately disabling its existing semantics. Includes and other external-resource access remain a separate security and scope decision.

## Standalone GPUI

Official GPUI documentation explicitly supports standalone applications. The inspected Zed snapshot uses `gpui_platform::application()` to choose host backends and GPUI views implementing `Render` to construct windows. No Zed workspace integration is required for that example.

Verified primitives include:

- `StyledText` for runs with differing styles.
- `InteractiveText::on_click` for clickable text ranges.
- Scrolling containers in the official `scrollable` example.
- macOS Metal rendering and the `font-kit` platform feature for actual text glyph rendering.

These establish available primitives, not a finished document renderer or a guarantee of accessibility/selection behavior. GPUI is pre-1.0 and its documentation warns about breaking changes; choose a compatible pinned dependency set after a build check rather than copying wildcard versions from examples.

Sources at Zed snapshot [`c3ab556`](https://github.com/zed-industries/zed/tree/c3ab5564f83ef5ec2b49f9ed4c8544b33c16401b):

- [GPUI getting started and macOS prerequisites](https://github.com/zed-industries/zed/blob/c3ab5564f83ef5ec2b49f9ed4c8544b33c16401b/crates/gpui/README.md).
- [Standalone hello-world example](https://github.com/zed-industries/zed/blob/c3ab5564f83ef5ec2b49f9ed4c8544b33c16401b/crates/gpui/examples/hello_world.rs).
- [Styled and interactive text](https://github.com/zed-industries/zed/blob/c3ab5564f83ef5ec2b49f9ed4c8544b33c16401b/crates/gpui/src/elements/text.rs).
- [Scrolling example](https://github.com/zed-industries/zed/blob/c3ab5564f83ef5ec2b49f9ed4c8544b33c16401b/crates/gpui/examples/scrollable.rs).

## Local development facts

- Host architecture: `arm64`.
- `cargo`, `rustc`, and `rustup` commands exist.
- `rustup show active-toolchain` reports `1.96.0-aarch64-apple-darwin`, selected by `RUSTUP_TOOLCHAIN`.
- `xcode-select -p` reports `/Applications/Xcode.app/Contents/Developer`.
- SDK completeness, Metal compilation, dependency resolution, and application startup have not been tested.

## Open checks

- Decide saved-file refresh versus unsaved-editor-buffer integration before selecting a refresh mechanism. Atomic-save replacement must be handled if filesystem watching is chosen; no watcher library was evaluated yet.
- Decide whether attribute substitution is permitted despite the limited rendering subset.
- Extend the passing scoped parser gate to other preprocessing removals and exhaustive malformed-input fallback; the initial checks establish original-source recovery for the committed examples, not arbitrary documents.
- Check GPUI build and basic usability on the chosen target platform.

## Inspection commands

Discovery and local-tool commands actually run:

```sh
gh api 'search/repositories?q=asciidoc+language:Rust&per_page=8' --jq '.items[] | {full_name, description, html_url}'
gh api repos/zed-industries/zed/contents/crates/gpui/examples --jq '.[].name'
command -v cargo rustc xcrun xcodebuild
xcode-select -p
uname -m
command -v rustup
rustup show active-toolchain
```

Representative exact source-fetch commands actually run (decoded results were saved under a temporary inspection directory and examined using the file-reading tool):

```sh
gh api 'repos/asciidoc-rs/asciidoc-parser/contents/parser/src/inlines/inline_node.rs?ref=c4ea28b3cd6e3a310d123eebeddb991d882f865f' --jq '.content | @base64d'
gh api 'repos/zed-industries/zed/contents/crates/gpui/src/elements/text.rs?ref=c3ab5564f83ef5ec2b49f9ed4c8544b33c16401b' --jq '.content | @base64d'
```
