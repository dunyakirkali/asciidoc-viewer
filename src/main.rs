use std::{path::PathBuf, time::Duration};

use asciidoc_viewer::{Kind, Preview, Row, parse, read_source, web_url};
use gpui::{
    App, Application, Bounds, Context, FocusHandle, InteractiveText, KeyDownEvent, ScrollHandle,
    StyledText, TextRun, TitlebarOptions, UnderlineStyle, Window, WindowBounds, WindowOptions, div,
    font, prelude::*, px, rgb, size,
};

struct Viewer {
    path: PathBuf,
    source: String,
    preview: Preview,
    read_error: Option<String>,
    focus: FocusHandle,
    scroll: ScrollHandle,
    focused_link: Option<usize>,
    text_size: f32,
}

impl Viewer {
    fn new(
        path: PathBuf,
        source: String,
        preview: Preview,
        read_error: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window);
        // ponytail: reread one saved file every 250 ms; use directory watching if I/O matters.
        // Comparing content, not inode/mtime, also handles atomic editor saves.
        cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(250)).await;
                let Ok((path, previous, retry)) = view.update(cx, |this, _| {
                    (this.path.clone(), this.source.clone(), this.read_error.is_some())
                }) else { break };
                let result = cx.background_executor().spawn(async move {
                    let source = read_source(&path)?;
                    if source == previous && !retry {
                        Ok(None)
                    } else {
                        let preview = parse(&source);
                        Ok(Some((source, preview)))
                    }
                }).await;
                if view.update(cx, |this, cx| {
                    match result {
                        Ok(Some((source, preview))) => {
                            this.source = source;
                            this.preview = preview;
                            this.read_error = None;
                            this.focused_link = None;
                            cx.notify();
                        }
                        Err(error) if this.read_error.as_ref() != Some(&error) => {
                            this.preview = Preview::raw(&this.source, format!("{error}\nShowing the last readable source; this is not the latest saved file."));
                            this.read_error = Some(error);
                            this.focused_link = None;
                            cx.notify();
                        }
                        _ => {}
                    }
                }).is_err() { break }
            }
        }).detach();
        Self {
            path,
            source,
            preview,
            read_error,
            focus,
            scroll: ScrollHandle::new(),
            focused_link: None,
            text_size: 16.0,
        }
    }

    fn links(&self) -> Vec<(usize, &str)> {
        self.preview
            .rows
            .iter()
            .enumerate()
            .flat_map(|(row, content)| {
                content
                    .runs
                    .iter()
                    .filter_map(move |run| run.url.as_deref().map(|url| (row, url)))
            })
            .collect()
    }

    fn key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        if event.keystroke.modifiers.platform {
            match key {
                "q" => cx.quit(),
                "=" | "+" => self.text_size = (self.text_size + 2.0).min(40.0),
                "-" => self.text_size = (self.text_size - 2.0).max(8.0),
                "0" => self.text_size = 16.0,
                _ => return,
            }
        } else if key == "tab" {
            let links = self.links();
            if !links.is_empty() {
                let next = match (self.focused_link, event.keystroke.modifiers.shift) {
                    (Some(index), true) => (index + links.len() - 1) % links.len(),
                    (Some(index), false) => (index + 1) % links.len(),
                    (None, true) => links.len() - 1,
                    (None, false) => 0,
                };
                let row = links[next].0;
                self.focused_link = Some(next);
                self.scroll.scroll_to_item(row);
            }
        } else if key == "enter" {
            if let Some(url) = self
                .focused_link
                .and_then(|index| self.links().get(index).and_then(|(_, url)| web_url(url)))
            {
                cx.open_url(&url);
            }
        } else {
            let delta = match key {
                "down" => -40.0,
                "up" => 40.0,
                "pagedown" | "space" if !event.keystroke.modifiers.shift => {
                    -f32::from(self.scroll.bounds().size.height) * 0.85
                }
                "pageup" | "space" => f32::from(self.scroll.bounds().size.height) * 0.85,
                "home" => f32::MAX,
                "end" => -f32::MAX,
                "escape" => {
                    self.focused_link = None;
                    0.0
                }
                _ => return,
            };
            let mut offset = self.scroll.offset();
            offset.y = (offset.y + px(delta)).clamp(-self.scroll.max_offset().height, px(0.0));
            self.scroll.set_offset(offset);
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn row(
        &self,
        index: usize,
        row: &Row,
        first_link: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let mut text = String::new();
        let mut runs = Vec::new();
        let mut ranges = Vec::new();
        let mut targets = Vec::new();
        let heading = matches!(row.kind, Kind::Heading(_));
        let monospace = matches!(row.kind, Kind::Code | Kind::Raw);
        for run in &row.runs {
            let start = text.len();
            text.push_str(&run.text);
            let mut font = font(if monospace || run.monospace {
                "Menlo"
            } else {
                ".SystemUIFont"
            });
            if heading || run.bold {
                font = font.bold();
            }
            if run.italic {
                font = font.italic();
            }
            let focused =
                run.url.is_some() && self.focused_link == Some(first_link + targets.len());
            runs.push(TextRun {
                len: run.text.len(),
                font,
                color: rgb(if run.url.is_some() {
                    0x0645ad
                } else {
                    0x1f2937
                })
                .into(),
                background_color: focused.then(|| rgb(0xdbeafe).into()),
                underline: run.url.as_ref().map(|_| UnderlineStyle {
                    thickness: px(if focused { 3.0 } else { 1.0 }),
                    color: None,
                    wavy: false,
                }),
                strikethrough: None,
            });
            if let Some(url) = &run.url {
                ranges.push(start..text.len());
                targets.push(url.clone());
            }
        }
        let text = StyledText::new(text).with_runs(runs);
        let view = cx.entity().downgrade();
        let text = InteractiveText::new(("text", index), text).on_click(
            ranges,
            move |link, window, cx| {
                if let Some(url) = targets.get(link).and_then(|url| web_url(url)) {
                    cx.open_url(&url);
                    view.update(cx, |this, cx| {
                        this.focused_link = Some(first_link + link);
                        this.focus.focus(window);
                        cx.notify();
                    })
                    .ok();
                }
            },
        );
        let mut content = div()
            .id(("row", index))
            .flex()
            .gap_3()
            .w_full()
            .flex_shrink_0()
            .pl(px(row.indent as f32 * 20.0))
            .pb_3();
        let scale = match row.kind {
            Kind::Heading(1) => 2.0,
            Kind::Heading(2) => 1.5,
            Kind::Heading(_) => 1.2,
            _ => 1.0,
        };
        content = content.text_size(px(self.text_size * scale));
        if let Some(marker) = &row.marker {
            content = content.child(div().min_w(px(24.0)).child(marker.clone()));
        }
        let mut body = div().flex_1().min_w_0().child(text);
        if monospace {
            body = body.p_3().rounded_md().bg(rgb(0xf3f4f6));
        }
        content.child(body)
    }
}

impl Render for Viewer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut next_link = 0;
        let rows: Vec<_> = self
            .preview
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let element = self.row(index, row, next_link, cx);
                next_link += row.runs.iter().filter(|run| run.url.is_some()).count();
                element
            })
            .collect();
        let target = self
            .focused_link
            .and_then(|index| self.links().get(index).map(|(_, url)| (*url).to_string()));
        div().size_full().flex().flex_col().bg(rgb(0xffffff)).text_color(rgb(0x1f2937))
            .font_family(".SystemUIFont").text_size(px(self.text_size))
            .track_focus(&self.focus).on_key_down(cx.listener(Self::key_down))
            .child(div().p_3().border_b_1().border_color(rgb(0xd1d5db))
                .child(self.path.display().to_string())
                .children(self.preview.diagnostics.iter().map(|message| {
                    div().mt_2().child(format!("Diagnostic: {message}"))
                })))
            .child(div().id("document").flex_1().min_h_0().overflow_y_scroll()
                .track_scroll(&self.scroll).flex().flex_col().p_6().children(rows))
            .child(div().p_2().border_t_1().border_color(rgb(0xd1d5db)).text_size(px(12.0))
                .child(target.map(|url| format!("Enter opens {url}")).unwrap_or_else(||
                    "On-save preview · Tab: links · Enter: open · Arrows/Page Up/Down: scroll · ⌘+/−: text size".into())))
    }
}

fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let Some(path) = arguments.next() else {
        eprintln!("Usage: asciidoc-viewer FILE.adoc");
        std::process::exit(2);
    };
    if arguments.next().is_some() {
        eprintln!("Open one document per window: asciidoc-viewer FILE.adoc");
        std::process::exit(2);
    }
    let path = PathBuf::from(path);
    let (source, preview, error) = match read_source(&path) {
        Ok(source) => {
            let preview = parse(&source);
            (source, preview, None)
        }
        Err(error) => (String::new(), Preview::raw("", error.clone()), Some(error)),
    };
    Application::new().run(move |cx: &mut App| {
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(850.0), px(750.0)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("AsciiDoc Viewer".into()),
                    ..TitlebarOptions::default()
                }),
                ..WindowOptions::default()
            },
            move |window, cx| cx.new(|cx| Viewer::new(path, source, preview, error, window, cx)),
        );
        if let Err(error) = result {
            eprintln!("Cannot open preview window: {error}");
            cx.quit();
        }
        cx.activate(true);
    });
}
