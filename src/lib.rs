use std::{fs, io::Read, path::Path};

use asciidoc_parser::{
    Document, HasSpan, Parser, SafeMode, Span,
    blocks::{Block, FindBlocks, IsBlock, ListType},
    inlines::{CharRef, InlineNode, RawOrigin, RefVariant, StyleVariant},
    parser::{Fidelity, Transform},
    warnings::{WarningSeverity, WarningType},
};
use url::Url;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Kind {
    Heading(usize),
    #[default]
    Paragraph,
    Code,
    Raw,
}

#[derive(Clone, Debug, Default)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub monospace: bool,
    pub url: Option<String>,
}

#[derive(Debug, Default)]
pub struct Row {
    pub kind: Kind,
    pub runs: Vec<Run>,
    pub indent: usize,
    pub marker: Option<String>,
}

#[derive(Debug, Default)]
pub struct Preview {
    pub rows: Vec<Row>,
    pub diagnostics: Vec<String>,
}

impl Preview {
    pub fn raw(source: &str, diagnostic: String) -> Self {
        Self {
            rows: vec![Row {
                kind: Kind::Raw,
                runs: vec![Run {
                    text: source.into(),
                    ..Run::default()
                }],
                ..Row::default()
            }],
            diagnostics: vec![diagnostic],
        }
    }
}

pub fn read_source(path: &Path) -> Result<String, String> {
    const MAX_BYTES: usize = 8 * 1024 * 1024;
    let read = || -> std::io::Result<String> {
        if !fs::metadata(path)?.is_file() {
            return Err(std::io::Error::other("expected a regular UTF-8 file"));
        }
        let mut text = String::new();
        fs::File::open(path)?
            .take((MAX_BYTES + 1) as u64)
            .read_to_string(&mut text)?;
        if text.len() > MAX_BYTES {
            return Err(std::io::Error::other(
                "experiment supports files up to 8 MiB",
            ));
        }
        Ok(text)
    };
    read().map_err(|error| format!("Cannot read {}: {error}", path.display()))
}

pub fn web_url(target: &str) -> Option<String> {
    if target.chars().any(char::is_control) {
        return None;
    }
    let url = Url::parse(target).ok()?;
    (matches!(url.scheme(), "http" | "https") && url.host_str().is_some()).then(|| url.to_string())
}

pub fn parse(source: &str) -> Preview {
    std::panic::catch_unwind(|| parse_document(source)).unwrap_or_else(|_| {
        Preview::raw(
            source,
            "The processor failed unexpectedly. Showing current source.".into(),
        )
    })
}

fn parse_document(source: &str) -> Preview {
    let document = Parser::new().with_safe_mode(SafeMode::Secure).parse(source);
    let diagnostics: Vec<_> = document
        .warnings()
        .filter(|warning| warning.severity >= WarningSeverity::Warning)
        .map(|warning| {
            format!(
                "Line {}: {:?}",
                document.origin_of(warning.source).line,
                warning.warning
            )
        })
        .collect();
    if document.warnings().any(|warning| {
        matches!(
            warning.warning,
            WarningType::UnterminatedDelimitedBlock
                | WarningType::MaxBlockNestingExceeded(_)
                | WarningType::UnterminatedConditionalDirective(_)
                | WarningType::UnmatchedConditionalDirective(_)
        )
    }) {
        return Preview::raw(source, diagnostics.join("\n"));
    }

    // Don't silently lose directives/content removed by semantic preprocessing.
    let mut mapped = vec![false; source.lines().count()];
    for line in 1..=document.span().data().lines().count() {
        let origin = document.source_map().origin_at(line, 1);
        if let Some(mapped) = origin
            .line
            .checked_sub(1)
            .and_then(|line| mapped.get_mut(line))
        {
            *mapped = true;
        }
    }
    if source
        .lines()
        .zip(mapped)
        .any(|(line, mapped)| !mapped && !line.trim().is_empty())
    {
        return Preview::raw(
            source,
            "The processor removed content outside the rendering subset. Showing current source."
                .into(),
        );
    }

    let mut preview = Preview {
        diagnostics,
        ..Preview::default()
    };
    if let (Some(title), Some(span)) = (document.doctitle(), document.header().title_source()) {
        heading(title, span, 1, source, &document, &mut preview);
    }
    for block in document.child_blocks() {
        append(block, source, &document, 0, None, &mut preview);
    }
    preview.diagnostics.dedup();
    preview
}

fn raw_lines(source: &str, document: &Document<'_>, span: Span<'_>) -> String {
    if span.data().is_empty() {
        return String::new();
    }
    let start = document.origin_of(span).line;
    let last = span.line() + span.data().bytes().filter(|byte| *byte == b'\n').count();
    let end = document.source_map().origin_at(last, 1).line;
    // ponytail: O(n) source scan per raw block; index line starts if profiling warrants it.
    source
        .split_inclusive('\n')
        .skip(start.saturating_sub(1))
        .take(end.saturating_sub(start) + 1)
        .collect()
}

fn heading(
    title: &str,
    span: Span<'_>,
    level: usize,
    source: &str,
    document: &Document<'_>,
    preview: &mut Preview,
) {
    let (kind, text) = if title.contains(['<', '&']) {
        preview.diagnostics.push("Rich titles need a public inline-tree accessor from the processor; showing their source.".into());
        (Kind::Raw, raw_lines(source, document, span))
    } else {
        (Kind::Heading(level), title.into())
    };
    preview.rows.push(Row {
        kind,
        runs: vec![Run {
            text,
            ..Run::default()
        }],
        ..Row::default()
    });
}

fn append(
    block: &Block<'_>,
    source: &str,
    document: &Document<'_>,
    indent: usize,
    marker: Option<String>,
    preview: &mut Preview,
) {
    if document.origin_of(block.span()).fidelity
        == Fidelity::Synthetic(Transform::SecureLinkRewrite)
    {
        preview.rows.push(Row {
            kind: Kind::Raw,
            indent,
            marker,
            runs: vec![Run {
                text: raw_lines(source, document, block.span()),
                ..Run::default()
            }],
        });
        return;
    }
    // Listing content is also preprocessed: recover its original body, not secure-link rewrites.
    let listing = match block {
        Block::Simple(code) if code.resolved_context().as_ref() == "listing" => {
            Some(code.content().original())
        }
        Block::RawDelimited(code) if code.resolved_context().as_ref() == "listing" => {
            Some(code.content().original())
        }
        _ => None,
    };
    if let Some(content) = listing {
        preview.rows.push(Row {
            kind: Kind::Code,
            indent,
            marker,
            runs: vec![Run {
                text: raw_lines(source, document, content),
                ..Run::default()
            }],
        });
        return;
    }
    match block {
        Block::Section(section) => {
            heading(
                section.section_title(),
                section.section_title_source(),
                section.level() + 1,
                source,
                document,
                preview,
            );
            for child in section.child_blocks() {
                append(child, source, document, indent, None, preview);
            }
        }
        Block::Preamble(_) => {
            for child in block.child_blocks() {
                append(child, source, document, indent, None, preview);
            }
        }
        Block::List(list)
            if matches!(list.type_(), ListType::Ordered | ListType::Unordered)
                && !list.is_checklist() =>
        {
            for (index, item) in list.child_blocks().enumerate() {
                let marker = if list.type_() == ListType::Ordered {
                    format!("{}.", list.start().unwrap_or(1) + index as i64)
                } else {
                    "•".into()
                };
                for (index, child) in item.child_blocks().enumerate() {
                    append(
                        child,
                        source,
                        document,
                        indent + usize::from(index > 0),
                        (index == 0).then(|| marker.clone()),
                        preview,
                    );
                }
            }
        }
        Block::Simple(paragraph) if paragraph.resolved_context().as_ref() == "paragraph" => {
            preview.rows.push(Row {
                kind: Kind::Paragraph,
                indent,
                marker,
                runs: inlines(
                    paragraph.content().inlines(),
                    source,
                    document,
                    &Run::default(),
                ),
            });
        }
        Block::DocumentAttribute(_) => {}
        _ => preview.rows.push(Row {
            kind: Kind::Raw,
            indent,
            marker,
            runs: vec![Run {
                text: raw_lines(source, document, block.span()),
                ..Run::default()
            }],
        }),
    }
}

fn inlines(
    nodes: &[InlineNode<'_>],
    source: &str,
    document: &Document<'_>,
    style: &Run,
) -> Vec<Run> {
    let mut runs = Vec::new();
    for node in nodes {
        match node {
            InlineNode::Text { value, .. }
            | InlineNode::Raw {
                value,
                origin: RawOrigin::Substitution,
                ..
            } => {
                runs.push(Run {
                    text: value.to_string(),
                    ..style.clone()
                });
            }
            InlineNode::CharRef {
                value: CharRef::Special(character),
                ..
            } => {
                runs.push(Run {
                    text: character.to_string(),
                    ..style.clone()
                });
            }
            InlineNode::CharRef {
                value: CharRef::Replacement(text),
                ..
            } => {
                runs.push(Run {
                    text: (*text).into(),
                    ..style.clone()
                });
            }
            InlineNode::Styled(styled)
                if matches!(
                    styled.variant,
                    StyleVariant::Strong | StyleVariant::Emphasis | StyleVariant::Code
                ) && styled.passthrough.is_none() =>
            {
                let style = Run {
                    bold: style.bold || styled.variant == StyleVariant::Strong,
                    italic: style.italic || styled.variant == StyleVariant::Emphasis,
                    monospace: style.monospace || styled.variant == StyleVariant::Code,
                    ..style.clone()
                };
                runs.extend(inlines(&styled.children, source, document, &style));
            }
            InlineNode::Ref(reference)
                if document.origin_of(node.span()).fidelity
                    == Fidelity::Synthetic(Transform::SecureLinkRewrite) =>
            {
                runs.push(Run {
                    text: raw_lines(source, document, reference.span())
                        .trim_end_matches(['\r', '\n'])
                        .into(),
                    monospace: true,
                    ..Run::default()
                });
            }
            InlineNode::Ref(reference)
                if reference.variant == RefVariant::Link
                    && web_url(reference.target.as_ref()).is_some() =>
            {
                let style = Run {
                    url: web_url(reference.target.as_ref()),
                    ..style.clone()
                };
                if reference.children.is_empty() {
                    runs.push(Run {
                        text: reference.target.to_string(),
                        ..style
                    });
                } else {
                    runs.extend(inlines(&reference.children, source, document, &style));
                }
            }
            InlineNode::LineBreak { .. } => runs.push(Run {
                text: "\n".into(),
                ..style.clone()
            }),
            _ => runs.push(Run {
                text: node.span().data().into(),
                monospace: true,
                ..Run::default()
            }),
        }
    }
    runs
}
