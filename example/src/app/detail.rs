//! Structured detail presentation; every value comes from the API or library.
use super::{Library, MetabookApp, Phase};
use crate::api::LibraryBook;
use ely_gpui_component::feedback::ResultView;
use ely_gpui_component::theme::{ActiveTheme as _, TextSize};
use gpui::{
    div, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled,
};

use gpui_component::{h_flex, v_flex, StyledExt as _};
use serde_json::Value;

pub(super) fn field(value: &Value, key: &str) -> String {
    display(&value[key])
}
fn display(value: &Value) -> String {
    match value {
        Value::Null => "Not provided".into(),
        Value::String(text) if text.is_empty() => "Not provided".into(),
        Value::String(text) => text.clone(),
        Value::Array(values) => values.iter().map(display).collect::<Vec<_>>().join("; "),
        _ => value.to_string(),
    }
}
pub(super) fn capitalized(value: &str) -> String {
    let mut chars = value.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}
pub(super) fn authors(book: &Value) -> String {
    let Some(authors) = book["authors"].as_array().filter(|a| !a.is_empty()) else {
        return "Author not provided".into();
    };
    authors
        .iter()
        .map(|author| field(author, "name"))
        .collect::<Vec<_>>()
        .join("; ")
}
pub(super) fn language(book: &Value) -> String {
    match book["language"].as_str() {
        Some("en") => "English (en)".into(),
        Some("fr") => "French (fr)".into(),
        Some("de") => "German (de)".into(),
        Some("es") => "Spanish (es)".into(),
        Some(code) => code.into(),
        None => "Language not provided".into(),
    }
}
fn timestamp(value: &Value) -> String {
    value
        .as_str()
        .map(|date| {
            let short = date.get(..19).unwrap_or(date).replace('T', " · ");
            format!("{short} UTC")
        })
        .unwrap_or_else(|| "Not provided".into())
}
fn decimal(value: &Value) -> String {
    value
        .as_f64()
        .map(|n| format!("{n:.2}"))
        .unwrap_or_else(|| "Not provided".into())
}

impl MetabookApp {
    pub(super) fn result_record(&self, value: &Value) -> Option<&LibraryBook> {
        let Library::Ready(books) = &self.library else {
            return None;
        };
        let id = value["record_id"].as_str()?;
        books.iter().find(|book| book.id == id)
    }

    pub(super) fn render_book_detail(&self, cx: &mut Context<Self>) -> AnyElement {
        let Phase::Done { explorer, .. } = &self.phase else {
            return div().into_any_element();
        };
        let value = explorer.read(cx).value().clone();
        // These regions share the explorer's callbacks without exposing its
        // mutable tree, graph navigation, or editor internals to the shell.
        let presentation = explorer.update(cx, |explorer, cx| explorer.presentation(cx));
        let book = &value["book"];
        let structure = &value["structure"];
        let summary = &structure["summary"];
        let meta = &value["meta"];
        let record = self
            .result_record(&value)
            .map(|book| &book.record)
            .unwrap_or(&Value::Null);
        let authors = book["authors"]
            .as_array()
            .map(|authors| {
                authors
                    .iter()
                    .map(|a| {
                        let name = field(a, "name");
                        if a["birth_year"].is_number() || a["death_year"].is_number() {
                            format!(
                                "{name} · {}–{}",
                                a["birth_year"]
                                    .as_i64()
                                    .map(|y| y.to_string())
                                    .unwrap_or_else(|| "?".into()),
                                a["death_year"]
                                    .as_i64()
                                    .map(|y| y.to_string())
                                    .unwrap_or_else(|| "?".into())
                            )
                        } else {
                            name
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Not provided".into());
        let bibliographic = vec![
            ("Title", field(book, "title")),
            ("Author", authors),
            ("Language", language(book)),
            ("Subjects", field(book, "subjects")),
            ("ISBN", field(book, "isbn")),
            (
                "Gutenberg ID",
                book["gutenberg_id"]
                    .as_u64()
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "Not applicable · uploaded EPUB".into()),
            ),
            (
                "Source",
                book["source"].as_str().unwrap_or("gutenberg").to_string(),
            ),
            (
                "Format",
                record["format"]
                    .as_str()
                    .map(str::to_uppercase)
                    .unwrap_or_else(|| "Not provided".into()),
            ),
            ("Publisher", field(book, "publisher")),
            ("License", field(book, "license")),
            ("Publication date", field(book, "date")),
            ("Pages", field(book, "number_of_pages")),
        ];
        let scope = record["scan"]["scope"].as_str().unwrap_or("structure");
        let scan = vec![
            ("State / scope", format!("Scanned · {scope} detail")),
            (
                "Last scanned",
                timestamp(&record["scan"]["last_scanned_at"]),
            ),
            ("Created", timestamp(&record["created_at"])),
            ("Updated", timestamp(&record["updated_at"])),
            ("Uploaded at", timestamp(&meta["uploaded_at"])),
            ("Stored pathname", field(&value["blob"], "pathname")),
            (
                "File size",
                value["blob"]["size_bytes"]
                    .as_u64()
                    .map(|n| format!("{n} bytes"))
                    .unwrap_or_else(|| "Not provided".into()),
            ),
            ("Spine documents", field(meta, "spine_document_count")),
            (
                "Processing time",
                meta["processing_time_ms"]
                    .as_u64()
                    .map(|n| format!("{n} ms"))
                    .unwrap_or_else(|| "Not provided".into()),
            ),
        ];
        let classification = vec![
            ("Applied schema", field(structure, "schema")),
            ("Detected schema", field(structure, "schema_detected")),
            (
                "Override",
                if structure["schema_overridden"] == true {
                    "Yes · manual selection".into()
                } else {
                    "No · automatic selection".into()
                },
            ),
        ];
        let hierarchy = match structure["schema"].as_str() {
            Some("canonical_scripture") => "Book → Chapter → Verse",
            Some("sectioned_book") => "Part → Chapter → Paragraph",
            Some("standard_book") => "Chapter → Paragraph",
            Some("essay_or_story_collection") => "Essay / Story → Paragraph",
            Some("flat") => "Paragraph",
            _ => "Not provided",
        };
        let automatic = vec![
            (
                "Confidence",
                capitalized(&field(structure, "schema_confidence")),
            ),
            ("Heuristic support", decimal(&structure["schema_score"])),
            ("Hierarchy", hierarchy.into()),
        ];
        let totals = vec![
            ("Top-level nodes", field(summary, "total_top_level_nodes")),
            (
                "Mid-level nodes",
                if summary["total_mid_level_nodes"].is_null() {
                    "Not applicable".into()
                } else {
                    field(summary, "total_mid_level_nodes")
                },
            ),
            ("Paragraphs", field(summary, "total_paragraphs")),
            ("Sentences", field(summary, "total_sentences")),
            ("Words", field(summary, "total_words")),
        ];
        let averages = vec![
            (
                "Paragraphs / chapter",
                decimal(&summary["avg_paragraphs_per_chapter"]),
            ),
            (
                "Sentences / paragraph",
                decimal(&summary["avg_sentences_per_paragraph"]),
            ),
            (
                "Words / sentence",
                decimal(&summary["avg_words_per_sentence"]),
            ),
            (
                "Tokens / tokenizer",
                if summary["total_tokens"].is_null() {
                    "Not requested".into()
                } else {
                    format!(
                        "{} · {}",
                        field(summary, "total_tokens"),
                        field(&meta["tokenizer"], "name")
                    )
                },
            ),
            (
                "Vocabulary size",
                if meta["tokenizer"].is_null() {
                    "Not requested".into()
                } else {
                    field(&meta["tokenizer"], "vocab_size")
                },
            ),
        ];
        let evidence = [
            ("Verse-number lines", "verse_number_lines"),
            ("Part markers", "part_markers"),
            ("Chapter-word markers", "chapter_word_markers"),
            ("Chapter-numeral markers", "chapter_numeral_markers"),
            ("All-caps title lines", "caps_title_lines"),
            ("Title / byline pairs", "title_byline_pairs"),
            ("Paragraph blocks", "paragraph_blocks"),
        ]
        .map(|(label, key)| (label, field(&structure["schema_evidence"], key)))
        .to_vec();
        let candidates = [
            "canonical_scripture",
            "sectioned_book",
            "standard_book",
            "essay_or_story_collection",
            "flat",
        ]
        .map(|key| {
            let score = decimal(&structure["schema_candidates"][key]);
            (
                key,
                if structure["schema_detected"] == key {
                    format!("{score} · selected")
                } else {
                    score
                },
            )
        })
        .to_vec();

        div().id("book-detail-page").flex_1().min_h_0().min_w_0().overflow_y_scroll()
            .child(v_flex().pl_12().pr_8().py_8().gap_8()
                .child(ResultView::success(("schema-success",self.request_ix), "Book schema ready")
                    .body(format!("{} · {} · metadata and structural counts, without book text",field(book,"title"),field(structure,"schema")))
                    .action(presentation.copy))
                .child(presentation.graph)
                .child(self.detail_pair("Bibliographic metadata",bibliographic,"Scan & source file",scan,cx))
                .child(v_flex().gap_5().child(self.detail_pair("Schema classification",classification,"Automatic detection",automatic,cx))
                    .child(self.detail_note("Rule support, not a probability. Confidence describes automatic detection even when overridden.",cx)))
                .child(self.detail_pair("Cleaned-body totals",totals,"Averages & optional tokens",averages,cx))
                .child(v_flex().gap_4().child(self.detail_pair("Detection evidence · signal counts",evidence,"Candidate support",candidates,cx))
                    .child(self.detail_note("Independent candidate scores; they do not sum to one.",cx)))
                .child(explorer.clone())
                .child(div().border_t_1().border_color(cx.theme().colors.border).pt_4()
                    .child(self.detail_note("Indices restart at 1 within each cleaned-body parent; original-source offsets are not exposed.",cx))))
            .into_any_element()
    }

    fn section_heading(&self, title: &'static str, cx: &Context<Self>) -> AnyElement {
        div()
            .w_full()
            .pb_2()
            .border_b_1()
            .border_color(cx.theme().colors.border)
            .text_size(cx.theme().text_size(TextSize::Base))
            .font_medium()
            .child(title)
            .into_any_element()
    }
    fn detail_note(&self, note: &'static str, cx: &Context<Self>) -> AnyElement {
        div()
            .text_size(cx.theme().text_size(TextSize::Xs))
            .text_color(cx.theme().colors.fg_muted)
            .child(note)
            .into_any_element()
    }
    fn detail_pair(
        &self,
        left: &'static str,
        left_rows: Vec<(&'static str, String)>,
        right: &'static str,
        right_rows: Vec<(&'static str, String)>,
        cx: &Context<Self>,
    ) -> AnyElement {
        h_flex()
            .w_full()
            .items_start()
            .gap(gpui::rems(3.5))
            .child(self.detail_section(left, left_rows, cx))
            .child(self.detail_section(right, right_rows, cx))
            .into_any_element()
    }
    fn detail_section(
        &self,
        title: &'static str,
        rows: Vec<(&'static str, String)>,
        cx: &Context<Self>,
    ) -> AnyElement {
        v_flex()
            .flex_1()
            .min_w_0()
            .gap_4()
            .child(self.section_heading(title, cx))
            .child(
                v_flex()
                    .gap_1()
                    .children(rows.into_iter().map(|(label, value)| {
                        h_flex()
                            .items_start()
                            .gap_3()
                            .child(
                                div()
                                    .w_40()
                                    .flex_none()
                                    .text_size(cx.theme().text_size(TextSize::Xs))
                                    .text_color(cx.theme().colors.fg_muted)
                                    .child(label),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(cx.theme().text_size(TextSize::Sm))
                                    .child(value),
                            )
                    })),
            )
            .into_any_element()
    }
}
