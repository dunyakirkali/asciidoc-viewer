# AsciiDoc Viewer — Feasibility Experiment

Status: design confirmed; initial processor gate passes. GPUI view and on-save refresh are not implemented yet.

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

This is a scoped parser check, not proof of arbitrary-input losslessness or a complete viewer. Other preprocessing removals, exhaustive warning classification, GUI behavior, save handling, and end-to-end performance remain to be checked.

## Research

[Primary-source findings](research.md) identify existing Rust processors and standalone GPUI primitives. `asciidoc-parser` is the agreed first candidate to evaluate, not a production dependency commitment. Raw-source preservation across preprocessing remains unproven.

No ADR yet: the current choices deliberately remain reversible experiment decisions. Revisit architecture decisions if a production implementation is approved.
