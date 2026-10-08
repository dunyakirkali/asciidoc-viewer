// Reuse the actual private view and refresh loop, without adding a production testing API.
mod viewer {
    #![allow(
        dead_code,
        reason = "This example supplies its own entry point instead of the included main."
    )]
    include!("../src/main.rs");

    use gpui::{AsyncApp, Keystroke, WindowHandle};
    use std::{cell::Cell, fs, rc::Rc, time::Instant};

    async fn frame(handle: WindowHandle<Viewer>, cx: &mut AsyncApp) {
        let done = Rc::new(Cell::new(false));
        let signal = done.clone();
        handle
            .update(cx, |_, window, _| {
                window.refresh();
                // Callbacks run before drawing: the second callback follows the first
                // frame's layout/paint/Metal submission, not necessarily physical display.
                window.on_next_frame(move |window, _| {
                    window.on_next_frame(move |_, _| signal.set(true));
                });
            })
            .unwrap();
        let started = Instant::now();
        while !done.get() {
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "native frame timed out"
            );
        }
    }

    async fn wait_for(
        handle: WindowHandle<Viewer>,
        cx: &mut AsyncApp,
        ready: impl Fn(&Viewer) -> bool,
    ) {
        let started = Instant::now();
        while !handle.read_with(cx, |view, _| ready(view)).unwrap() {
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "saved-file refresh timed out"
            );
        }
        frame(handle, cx).await;
    }

    fn key(handle: WindowHandle<Viewer>, cx: &mut AsyncApp, key: &str) {
        handle
            .update(cx, |view, window, cx| {
                view.key_down(
                    &KeyDownEvent {
                        keystroke: Keystroke::parse(key).unwrap(),
                        is_held: false,
                    },
                    window,
                    cx,
                );
            })
            .unwrap();
    }

    pub fn check() {
        // A native-loop panic must fail the check, rather than leave a window hanging.
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_secs(20));
            eprintln!("Native viewer gate timed out.");
            std::process::exit(1);
        });
        let directory =
            std::env::temp_dir().join(format!("asciidoc-viewer-native-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("document.adoc");
        let source = include_str!("document.adoc").to_string();
        fs::write(&path, &source).unwrap();
        let passed = Rc::new(Cell::new(false));
        let result = passed.clone();
        Application::new().run(move |cx: &mut App| {
            let initial = source.clone();
            let file = path.clone();
            let handle = cx.open_window(WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(850.0), px(750.0)), cx))),
                ..WindowOptions::default()
            }, |window, cx| cx.new(|cx| Viewer::new(file, initial.clone(), parse(&initial), None, window, cx))).unwrap();
            cx.activate(true);
            cx.spawn(async move |cx| {
                frame(handle, cx).await;
                let bounds = handle.read_with(cx, |view, _| view.scroll.bounds()).unwrap();
                assert!(bounds.size.height > px(100.0), "document has no usable viewport");
                key(handle, cx, "down");
                assert!(handle.read_with(cx, |view, _| view.scroll.offset().y < px(0.0)).unwrap());
                key(handle, cx, "home");
                key(handle, cx, "tab");
                assert_eq!(handle.read_with(cx, |view, _| view.focused_link).unwrap(), Some(0));
                key(handle, cx, "shift-tab");
                assert!(handle.read_with(cx, |view, _| view.focused_link == Some(view.links().len() - 1)).unwrap());
                key(handle, cx, "escape");
                key(handle, cx, "cmd-+");
                assert_eq!(handle.read_with(cx, |view, _| view.text_size).unwrap(), 18.0);
                key(handle, cx, "cmd-0");
                frame(handle, cx).await;
                key(handle, cx, "pagedown");
                assert!(handle.read_with(cx, |view, _| view.scroll.offset().y < px(0.0)).unwrap());
                frame(handle, cx).await;

                for (revision, atomic) in [("First", false), ("Other", true)] {
                    let large: String = (0..250).map(|index| format!("== {revision} section {index}\n\nParagraph *bold* and _italic_. https://example.org[Website]\n\n")).collect();
                    assert_eq!(large.lines().count(), 1_000);
                    let offset = handle.read_with(cx, |view, _| view.scroll.offset()).unwrap();
                    let started = Instant::now();
                    if atomic {
                        let replacement = directory.join("replacement.adoc");
                        fs::write(&replacement, &large).unwrap();
                        fs::rename(&replacement, &path).unwrap();
                    } else {
                        fs::write(&path, &large).unwrap();
                    }
                    wait_for(handle, cx, |view| view.source == large && view.preview.rows.len() == 500).await;
                    let elapsed = started.elapsed();
                    assert!(elapsed < Duration::from_secs(1), "1,000-line refresh exceeded one second: {elapsed:?}");
                    assert_eq!(handle.read_with(cx, |view, _| view.scroll.offset()).unwrap(), offset);
                    println!("1,000-line {} save → native frame marker: {elapsed:?}", if atomic { "atomic" } else { "ordinary" });
                }
                let last_source = handle.read_with(cx, |view, _| view.source.clone()).unwrap();
                fs::write(&path, [0xff]).unwrap();
                wait_for(handle, cx, |view| view.read_error.is_some()).await;
                handle.read_with(cx, |view, _| {
                    assert_eq!(view.preview.rows[0].kind, Kind::Raw);
                    assert_eq!(view.preview.rows[0].runs[0].text, last_source);
                    assert!(view.preview.diagnostics[0].contains("not the latest saved file"));
                }).unwrap();
                fs::write(&path, &last_source).unwrap();
                wait_for(handle, cx, |view| view.read_error.is_none() && view.preview.rows.len() == 500).await;
                fs::remove_file(&path).unwrap();
                wait_for(handle, cx, |view| view.read_error.is_some()).await;
                fs::write(&path, &last_source).unwrap();
                wait_for(handle, cx, |view| view.read_error.is_none()).await;
                let malformed = "----\ncurrent unfinished block\n \t\n";
                fs::write(&path, malformed).unwrap();
                wait_for(handle, cx, |view| view.source == malformed).await;
                handle.read_with(cx, |view, _| {
                    assert_eq!(view.preview.rows[0].kind, Kind::Raw);
                    assert_eq!(view.preview.rows[0].runs[0].text, malformed);
                    assert!(!view.preview.diagnostics.is_empty());
                }).unwrap();
                fs::write(&path, &source).unwrap();
                wait_for(handle, cx, |view| view.source == source && view.preview.diagnostics.is_empty()).await;
                fs::remove_file(&path).unwrap();
                fs::remove_dir(&directory).unwrap();
                println!("Native viewer gate passed: save refresh, recovery, raw fallback, keyboard handler scrolling, link focus, text size, scroll position.");
                result.set(true);
                cx.update(|cx| cx.quit()).unwrap();
            }).detach();
        });
        assert!(
            passed.get(),
            "native application exited before checks completed"
        );
    }
}

fn main() {
    viewer::check();
}
