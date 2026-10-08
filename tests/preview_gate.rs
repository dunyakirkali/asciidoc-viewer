use std::fs;

use asciidoc_viewer::{Kind, parse, read_source, web_url};

#[test]
fn preview_gate() {
    let source = include_str!("../examples/document.adoc");
    let preview = parse(source);
    assert!(preview.diagnostics.is_empty(), "{:?}", preview.diagnostics);
    assert_eq!(
        preview
            .rows
            .iter()
            .filter(|row| matches!(row.kind, Kind::Heading(_)))
            .count(),
        3
    );
    let runs: Vec<_> = preview.rows.iter().flat_map(|row| &row.runs).collect();
    assert!(runs.iter().any(|run| run.text.contains("Widget")));
    assert!(runs.iter().any(|run| run.bold && run.italic));
    assert!(
        runs.iter()
            .any(|run| run.url.as_deref() == Some("https://example.org/"))
    );
    for raw in [
        ".Unsupported table\n[cols=\"1,1\"]\n|===",
        ".Unsupported image",
        "NOTE:",
        "[WARNING]\n====",
        "include::missing.adoc[]",
        "include::https://example.org/remote.adoc[lines=1..3]",
        "link:other.adoc[Other document]",
        "<<supported-content,Section>>",
        "image:missing.png[Small]",
    ] {
        assert!(
            runs.iter().any(|run| run.text.contains(raw)),
            "missing raw source: {raw}"
        );
    }
    assert!(
        preview.rows.iter().any(
            |row| row.kind == Kind::Code && row.runs[0].text.contains("<hello> {product-name}")
        )
    );
    assert!(
        preview
            .rows
            .iter()
            .any(|row| row.marker.as_deref() == Some("1."))
    );
    assert!(
        preview
            .rows
            .iter()
            .any(|row| row.marker.as_deref() == Some("•") && row.indent > 0)
    );

    for source in [
        "= *Rich title*\n\nBody.\n",
        "----\nunfinished\n  \t\n",
        "Before.\ninclude::{missing}[opts=optional]\nAfter.\n",
        "ifdef::missing[]\nHidden.\nendif::missing[]\n",
        "[NOTE]\r\n====\r\nTabs:\t λ 東京\r\n====\r\n",
    ] {
        let preview = parse(source);
        assert!(
            preview.rows.iter().any(
                |row| (row.kind == Kind::Raw && !row.runs[0].text.is_empty())
                    || row
                        .runs
                        .iter()
                        .any(|run| run.monospace && run.text.contains("include::"))
            ),
            "source: {source:?}; preview: {preview:?}"
        );
        if preview.rows.len() == 1 && preview.rows[0].kind == Kind::Raw {
            assert_eq!(preview.rows[0].runs[0].text, source);
        }
    }
    for source in [
        "----\ninclude::missing.adoc[]\n----\n",
        "[listing]\ninclude::missing.adoc[]\n",
    ] {
        let listing = parse(source);
        assert_eq!(listing.rows[0].kind, Kind::Code, "{source}");
        assert_eq!(
            listing.rows[0].runs[0].text.trim_end(),
            "include::missing.adoc[]"
        );
    }

    let empty_listing = parse("----\n----\n");
    assert_eq!(empty_listing.rows[0].kind, Kind::Code);
    assert!(empty_listing.rows[0].runs[0].text.is_empty());

    let embedded = parse("Before.\ninclude::https://example.org/secret.adoc[]\nAfter.\n");
    let runs: Vec<_> = embedded.rows.iter().flat_map(|row| &row.runs).collect();
    assert_eq!(
        runs.iter()
            .filter(|run| run.text.contains("include::"))
            .count(),
        1
    );
    assert!(runs.iter().all(|run| run.url.is_none()));
    for url in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "mailto:someone@example.org",
        "https://example.org\n/hidden",
        "not a URL",
    ] {
        assert!(web_url(url).is_none(), "{url}");
    }
    assert_eq!(
        web_url("HTTPS://example.org"),
        Some("https://example.org/".into())
    );

    let directory = std::env::temp_dir().join(format!(
        "asciidoc-viewer-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let path = directory.join("document.adoc");
    fs::write(&path, "First.").unwrap();
    assert_eq!(read_source(&path).unwrap(), "First.");
    let replacement = directory.join("replacement.adoc");
    fs::write(&replacement, "Second.").unwrap();
    fs::rename(&replacement, &path).unwrap();
    assert_eq!(read_source(&path).unwrap(), "Second.");
    fs::write(&path, [0xff]).unwrap();
    assert!(read_source(&path).is_err());
    fs::remove_file(&path).unwrap();
    assert!(read_source(&path).is_err());
    fs::remove_dir(&directory).unwrap();

    let large: String = (0..250)
        .map(|index| format!("== Section {index}\n\nParagraph *bold* and _italic_.\n\n"))
        .collect();
    assert_eq!(large.lines().count(), 1_000);
    let started = std::time::Instant::now();
    let preview = parse(&large);
    assert_eq!(preview.rows.len(), 500);
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
    eprintln!(
        "1,000-line parser/projection: {:?} (not an end-to-end GUI measurement)",
        started.elapsed()
    );
}
