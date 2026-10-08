# AsciiDoc Viewer — Feasibility Experiment

Status: native macOS experiment implemented; parser/projection and native refresh checks pass. Visual, OS-input, browser, and VoiceOver acceptance remain manual checks.

## Settled decisions

- Build a standalone GPUI application, not a Zed extension or fork.
- A limited AsciiDoc subset is sufficient; full Asciidoctor compatibility is not required.
- Start with a feasibility experiment. A production-ready tool is a possible later goal, not the initial deliverable.
- Keep the glossary and design notes in this project directory.
- Initial supported subset: headings, paragraphs, bold/italic, ordered/unordered lists, listing/code blocks, and links.
- Initially defer rendering tables, images, and admonitions.
- Use the viewer beside an external editor while writing, with automatic refresh when the file is saved; unsaved-buffer integration is out of scope.
- Aim for a self-contained Rust application.
- Target macOS for the experiment; other platforms are not initially required.
- Accept an existing processor's built-in attribute substitution. The rendering subset does not require disabling those semantics.
- Do not expand includes; display include directives as raw source.
- Display unsupported constructs as raw source.
- If parsing cannot recover safely, display the current source plus a diagnostic instead of silently retaining an old preview. Recoverable warnings may accompany rendered output.
- Open one document per window, initially by passing its path at launch.
- HTTP/HTTPS links open in the system browser. Other destinations remain raw source; arbitrary URL schemes must not launch applications.
- Evaluate `asciidoc-parser` first and render its structured document directly with GPUI, retaining the original input for raw-source fallback.
- Do not introduce HTML rendering, Markdown conversion, or a custom parser. Reassess the processor if source preservation cannot satisfy the checks.

## Acceptance checks

- Commit a representative example covering headings, paragraphs, bold/italic, ordered/unordered lists, listing/code blocks, HTTP/HTTPS links, and attribute substitution.
- Tables, images, admonitions, and include directives remain visible as raw source; includes cause no external reads.
- Saving updates the preview, including when the editor atomically replaces the file.
- Unrecoverable parsing displays current raw source with a diagnostic.
- A roughly 1,000-line document refreshes within one second on the development Mac without freezing the window. Measure this rather than assume it.
- The document is readable and scrollable with mouse and keyboard; refresh does not unnecessarily reset the reading position.

## Implementation sequence

1. **Processor gate:** add the smallest runnable check for the agreed nodes, attribute substitution, disabled includes, warnings, and exact raw-source fallback. Source preservation across preprocessing is not yet proven. Stop and reassess if the gate fails.
2. **Native view:** build a standalone macOS GPUI window with the supported formatting, plain monospace code blocks, scrolling, raw-source fallback, diagnostics, and safe web-link opening. Establish a compatible pinned dependency set with an actual build.
3. **On-save refresh:** reload the same path automatically, handling atomic replacement, preserving reading position where practical, and visibly reporting read/parse failures. Never present a failed refresh as successful.
4. **Acceptance run:** exercise the committed example, unsupported constructs, incomplete input, normal/atomic saves, and the measured 1,000-line refresh target. Record passed checks, failures, and remaining limitations.

## Explicit non-goals

- Zed integration, an editor, or unsaved-buffer integration.
- File browser, tabs, or native file picker in the experiment.
- Full Asciidoctor compatibility, include expansion, or rich rendering of deferred constructs.
- Syntax highlighting, synchronized editor scrolling, or untested cross-platform support.
- Production packaging, plugin systems, or speculative extensibility.

## Processor gate result

The initial executable check passes with the published `asciidoc-parser` 0.31.2 release, pinned in `Cargo.toml` and `Cargo.lock`.

```sh
cargo run --manifest-path /Users/dunyakirkali/Projects/asciidoc-viewer/Cargo.toml --locked --example parser_gate
```

`examples/document.adoc` demonstrates the agreed subset and deferred constructs. `examples/parser_gate.rs` checks the AST, attribute substitution, verbatim code, raw block/inline source, Unicode/tabs/CRLF, secure includes (using a handler tripwire), embedded include source mapping, and structural-warning fallback.

Two implementation constraints were confirmed:

- Retain the original input: the processor trims trailing document whitespace and rewrites secure includes into links. Recover raw constructs using original input and the public source map, not normalized AST text alone.
- Source-map fidelity is line-level; more than one inline node can map to a rewritten include line. Rendering must avoid displaying that directive more than once.

This is a scoped parser check, not proof of arbitrary-input losslessness. The implemented projection and native checks below extend coverage; exhaustive source preservation and warning classification remain open.

## Running the experiment

From the repository root:

```sh
cargo run --locked -- examples/document.adoc
```

Pass one regular UTF-8 file, up to 8 MiB. Use a separate editor to change it; the viewer never writes the document. Close the window or press Cmd-Q to quit.

- Mouse/trackpad scrolling; arrows, Page Up/Down, Space/Shift-Space, Home/End.
- Tab/Shift-Tab selects web-link runs; Enter opens the selected HTTP/HTTPS destination. Escape clears selection. The selected target appears in the footer.
- Cmd-plus/minus changes document text size; Cmd-0 restores it.

The dependency set pins `asciidoc-parser` 0.31.2, `gpui` 0.2.2, and `url` 2.5.8. Published GPUI uses `Application::new()` rather than the newer `gpui_platform` API inspected initially. `font-kit` supplies glyph rendering; `runtime_shaders` uses the native Metal runtime compiler, avoiding a separate Xcode Metal Toolchain download. A graphics-free parser check is available through `--no-default-features`.

## Native-view and refresh checks

`src/lib.rs` projects the processor's AST into owned rows and styled runs. `src/main.rs` renders them with GPUI and rereads the saved path every 250 ms, comparing content rather than inode/mtime. Read/parse work runs in the background. The existing scroll handle retains pixel position when content still permits it; shortening a document clamps the offset rather than providing semantic scroll synchronization.

Structural delimiter/nesting/conditional warnings, detected removal of nonempty original lines, and caught processor panics show current raw source with a diagnostic. Read failures instead show the **last readable source**, explicitly marked as not the latest saved file, and retry automatically. Recovery also reparses when the restored content equals the last readable source.

Raw blocks and listing bodies are recovered from retained input/source-map locations. This preserves include directives inside both delimited and paragraph-style listings instead of displaying the processor's generated `link:…[role=include]` text. Empty code spans stay empty. Unsupported inline constructs remain literal monospace text; rewritten embedded includes are shown once, without a clickable target.

Exact passing validation commands:

```sh
cargo test --manifest-path /Users/dunyakirkali/Projects/asciidoc-viewer/Cargo.toml --no-default-features --locked --test preview_gate -- --nocapture
cargo run --manifest-path /Users/dunyakirkali/Projects/asciidoc-viewer/Cargo.toml --no-default-features --locked --example parser_gate
cargo run --manifest-path /Users/dunyakirkali/Projects/asciidoc-viewer/Cargo.toml --locked --example viewer_gate
cargo clippy --manifest-path /Users/dunyakirkali/Projects/asciidoc-viewer/Cargo.toml --locked --lib --bin asciidoc-viewer --example parser_gate --example viewer_gate -- -D warnings
cargo build --manifest-path /Users/dunyakirkali/Projects/asciidoc-viewer/Cargo.toml --locked --bin asciidoc-viewer
cargo fmt --manifest-path /Users/dunyakirkali/Projects/asciidoc-viewer/Cargo.toml --all -- --check
```

`examples/viewer_gate.rs` opens the actual native view, exercises its polling loop against temporary files, and exits. It checks ordinary/atomic saves, invalid UTF-8 and missing-file recovery, restored-identical-content recovery, malformed-input raw fallback, keyboard-handler scrolling/link focus/text size, a usable scroll viewport, and practical offset retention. It does not open a browser or require capture/accessibility permissions.

Across three development-profile native runs, the 1,000-line sample (250 headings, 250 styled paragraphs, 250 links) took **114–142 ms for ordinary saves** and **307–312 ms for atomic saves**, from initiating the write through a frame callback following layout/paint/Metal submission. The parser/projection-only check took approximately **16 ms**. The native marker includes polling and UI rendering, but is not a physical-display measurement or proof of continuous input responsiveness.

## Quality checks

The [quality-tooling baseline](quality.md) records setup, the one-command check, coverage scope, and dependency-audit findings. The projection check lives in `tests/preview_gate.rs` so coverage excludes test assertions from application-code metrics.

## Remaining acceptance and limitations

- **Not verified:** visual formatting/readability, actual OS keyboard delivery and mouse-wheel gestures, system-browser activation, narrow-window/large-text behavior, VoiceOver, and sustained scroll responsiveness during refresh. GPUI's public input-dispatch method returns a private type in 0.2.2, so the native check exercises our keyboard handler directly rather than simulating OS delivery.
- Window creation and native rendering callbacks work. Window-only capture failed with `could not create image from window`; no capture permission or system component was installed to bypass that failure.
- Plain headings render natively. Formatted or HTML-escaped titles remain raw with a diagnostic: the chosen processor exposes rendered title strings but keeps title inline trees private. No HTML conversion or custom title parser was added.
- Source-preservation checks are scoped, not arbitrary-input losslessness. Other processor edge cases and exhaustive warning classification need broader evidence before production use.
- Polling rereads one file four times per second; use directory watching if this becomes costly. Raw-source recovery scans lines per block; index line starts if larger documents warrant it.
- GPUI is pre-1.0. Cargo reports an upstream future-incompatibility warning for `block` 0.1.6; current builds pass, but future Rust compatibility is not promised.

Manual acceptance: launch the example, check its rendered and raw constructs, follow a web link with mouse and keyboard, scroll/resize/enlarge text, and save edits from an external editor while interacting with the window. Try an unfinished listing and a temporarily missing file; check that diagnostics are visible and recovery is clear. Screen-reader readiness is not claimed.

## Research

[Primary-source findings](research.md) identify existing Rust processors and standalone GPUI primitives. `asciidoc-parser` remains a reversible experiment dependency. The checks establish original-source recovery for tested inputs, not arbitrary preprocessing.

No ADR yet: the current choices deliberately remain reversible experiment decisions. Revisit architecture decisions if a production implementation is approved.
