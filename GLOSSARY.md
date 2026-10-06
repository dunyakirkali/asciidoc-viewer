# AsciiDoc Viewer

A standalone application for reading AsciiDoc documents.

## Language

**Viewer**:
A standalone application for reading rendered AsciiDoc documents.
_Avoid_: Zed extension, editor

**Supported subset**:
The AsciiDoc constructs the viewer presents with formatting rather than raw source. This describes presentation support, not all semantics understood by its document processor.
_Avoid_: Full AsciiDoc compatibility

**Unsupported content**:
AsciiDoc content outside the viewer's supported subset. Unsupported content is not necessarily invalid AsciiDoc.
_Avoid_: Invalid syntax

**Document processing**:
Interpretation of AsciiDoc attributes, directives, and substitutions that determines a document's content.
_Avoid_: Rendering

**Rendering**:
The visual presentation of an AsciiDoc document's content.
_Avoid_: Document processing

**Live preview**:
A document presentation automatically refreshed when its source file is saved. It does not represent unsaved changes in an external editor.
_Avoid_: Unsaved-buffer preview
