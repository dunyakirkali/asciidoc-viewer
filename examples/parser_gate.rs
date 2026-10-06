use asciidoc_parser::{
    Document, HasSpan, Parser, SafeMode, Span,
    attributes::Attrlist,
    blocks::{Block, FindBlocks, IsBlock, ListType},
    inlines::{InlineNode, RefVariant, StyleVariant},
    parser::{Fidelity, IncludeFileHandler, IncludeResolution, Transform},
    warnings::WarningType,
};

const SOURCE: &str = include_str!("document.adoc");

// A tripwire: secure parsing must not ask a host to read any include target.
#[derive(Debug)]
struct RejectIncludes;

impl IncludeFileHandler for RejectIncludes {
    fn resolve_target<'src>(
        &self,
        _source: Option<&str>,
        _target: &str,
        _attributes: &Attrlist<'src>,
        _parser: &Parser,
    ) -> IncludeResolution {
        panic!("include handler must never be invoked");
    }
}

fn parser() -> Parser {
    Parser::new()
        .with_safe_mode(SafeMode::Secure)
        .with_include_file_handler(RejectIncludes)
}

// Recover whole unsupported blocks from the retained input, not processed text.
// This also recovers include directives rewritten into links by secure mode.
fn raw_lines(source: &str, document: &Document<'_>, span: Span<'_>) -> String {
    let start = document.origin_of(span);
    let end_line = span.line() + span.data().bytes().filter(|byte| *byte == b'\n').count();
    let end = document.source_map().origin_at(end_line, 1);
    assert!(start.file.is_none() && end.file.is_none());
    assert!(start.line <= end.line);
    // ponytail: O(n) scan per raw block; index line starts if viewer profiling warrants it.
    source
        .split_inclusive('\n')
        .skip(start.line - 1)
        .take(end.line - start.line + 1)
        .collect()
}

fn raw_fallback<'a>(source: &'a str, document: &Document<'_>) -> Option<(&'a str, String)> {
    document
        .warnings()
        .find(|warning| {
            matches!(
                warning.warning,
                WarningType::UnterminatedDelimitedBlock | WarningType::MaxBlockNestingExceeded(_)
            )
        })
        .map(|warning| {
            let line = document.origin_of(warning.source).line;
            (source, format!("line {line}: {:?}", warning.warning))
        })
}

fn inspect_inlines(nodes: &[InlineNode<'_>], seen: &mut [bool; 8]) {
    for node in nodes {
        match node {
            InlineNode::Text { value, .. } => seen[0] |= value.contains("Widget"),
            InlineNode::Styled(styled) => {
                seen[1] |= styled.variant == StyleVariant::Strong;
                seen[2] |= styled.variant == StyleVariant::Emphasis;
                inspect_inlines(&styled.children, seen);
            }
            InlineNode::Ref(reference) if reference.variant == RefVariant::Link => {
                seen[3] |= reference.target.as_ref() == "https://example.org";
                seen[4] |= reference.target.as_ref() == "http://example.org";
                seen[5] |= reference.span().data() == "link:other.adoc[Other document]";
                inspect_inlines(&reference.children, seen);
            }
            InlineNode::Ref(reference) => {
                seen[6] |= reference.span().data() == "<<supported-content,Section>>";
            }
            InlineNode::Image(image) => {
                seen[7] |= image.span().data() == "image:missing.png[Small]";
            }
            _ => {}
        }
    }
}

fn main() {
    let document = parser().parse(SOURCE);
    assert_eq!(document.doctitle(), Some("Viewer Example"));
    assert_eq!(document.warnings().count(), 0);

    let mut seen_inlines = [false; 8];
    let mut sections = 0;
    let mut lists = [false; 2];
    let mut listing = false;
    let mut unsupported = [false; 4];
    let mut includes = 0;

    for block in document.descendant_blocks() {
        if let Some(nodes) = block.inlines() {
            inspect_inlines(nodes, &mut seen_inlines);
        }
        match block {
            Block::Section(section) => {
                assert_eq!(section.level(), 1);
                sections += 1;
            }
            Block::List(list) => {
                lists[0] |= list.type_() == ListType::Unordered;
                lists[1] |= list.type_() == ListType::Ordered;
            }
            Block::RawDelimited(code) if code.resolved_context().as_ref() == "listing" => {
                assert_eq!(
                    code.content().original().data(),
                    "fn main() {\n    println!(\"<hello> {product-name}\");\n}"
                );
                listing = true;
            }
            Block::Table(_) => {
                assert_eq!(
                    raw_lines(SOURCE, &document, block.span()),
                    ".Unsupported table\n[cols=\"1,1\"]\n|===\n|Name |Value\n|{product-name} |42\n|===\n"
                );
                unsupported[0] = true;
            }
            Block::Media(_) => {
                assert_eq!(
                    raw_lines(SOURCE, &document, block.span()),
                    ".Unsupported image\nimage::missing.png[An image]\n"
                );
                unsupported[1] = true;
            }
            Block::Admonition(_) => {
                let raw = raw_lines(SOURCE, &document, block.span());
                unsupported[2] |= raw == "NOTE: This admonition must remain raw source.\n";
                unsupported[3] |= raw
                    == "[WARNING]\n====\nThis *formatted* warning must remain raw source.\n====\n";
            }
            _ => {}
        }
        if document.origin_of(block.span()).fidelity
            == Fidelity::Synthetic(Transform::SecureLinkRewrite)
        {
            let raw = raw_lines(SOURCE, &document, block.span());
            assert!(raw.starts_with("include::"), "{raw:?}");
            includes += 1;
        }
    }

    assert_eq!(sections, 2);
    assert_eq!(
        seen_inlines, [true; 8],
        "attributes, styles, links, raw inlines"
    );
    assert_eq!(lists, [true; 2]);
    assert!(listing);
    assert_eq!(unsupported, [true; 4]);
    assert_eq!(includes, 2);

    // Original-line recovery must preserve Unicode, tabs, CRLF, and metadata.
    for source in [
        "[NOTE]\n====\nTabs:\t λ 東京\n====\n",
        "[NOTE]\r\n====\r\nTabs:\t λ 東京\r\n====\r\n",
        "include::file with spaces.adoc[opts=optional]\n",
    ] {
        let document = parser().parse(source);
        let block = document.child_blocks().next().expect("expected a block");
        assert_eq!(raw_lines(source, &document, block.span()), source);
    }

    // A rewritten include can be one inline inside a larger paragraph.
    let embedded = "= Includes\n:target: hidden.adoc\n:safe-mode: unsafe\n\nBefore.\ninclude::{target}[]\nAfter.\n";
    let document = parser().parse(embedded);
    let includes: Vec<_> = document
        .descendant_blocks()
        .flat_map(|block| block.inlines().unwrap_or_default())
        .filter(|node| {
            matches!(node, InlineNode::Ref(_))
                && document.origin_of(node.span()).fidelity
                    == Fidelity::Synthetic(Transform::SecureLinkRewrite)
        })
        .collect();
    assert_eq!(includes.len(), 1);
    assert_eq!(
        raw_lines(embedded, &document, includes[0].span()),
        "include::{target}[]\n"
    );

    let incomplete = "= Incomplete\n\n----\nunfinished *code*\n  \t\n";
    let document = parser().parse(incomplete);
    // The processor trims document whitespace; fallback must use retained input.
    assert_ne!(document.span().data(), incomplete);
    let (raw, diagnostic) = raw_fallback(incomplete, &document).expect("structural warning");
    assert_eq!(raw, incomplete);
    assert!(diagnostic.contains("UnterminatedDelimitedBlock"));
    assert!(diagnostic.starts_with("line 3:"));

    let recoverable = "See <<missing>>.\n";
    let document = parser().parse(recoverable);
    assert!(document.warnings().count() > 0);
    assert!(raw_fallback(recoverable, &document).is_none());

    println!(
        "Parser gate passed: AST, substitutions, raw blocks/inlines, disabled includes, diagnostics."
    );
}
