//! Shared catalog filters: multi-value constraints and one selected source.

use std::collections::BTreeSet;

use ely_gpui_component::{
    forms::{Choice, MultiSelect, Select},
    theme::{ActiveTheme as _, ControlSize, Radius, TextSize},
};
use gpui::{
    div, rems, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement, Role,
    SharedString, StatefulInteractiveElement as _, Styled,
};

use super::super::{
    search_filters::{DATE_OPTIONS, DOMAIN_OPTIONS, KIND_OPTIONS, SOURCE_OPTIONS},
    Library, MetabookApp,
};

const FILTER_WIDTH_REMS: f32 = 12.;
const FILTER_ROW_WIDTH_REMS: f32 = 4. * FILTER_WIDTH_REMS + 3. * 0.5;

const DATE_DESCRIPTION: &str =
    "Publication date in catalog searches; saved activity locally. Today uses UTC.";

#[derive(Clone, Copy)]
enum FilterField {
    Date,
    Category,
    Kind,
}

impl FilterField {
    fn label(self) -> &'static str {
        match self {
            Self::Date => "Date",
            Self::Category => "Category",
            Self::Kind => "Type",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Date => DATE_DESCRIPTION,
            Self::Category => "OpenAlex primary-topic domains or exact saved subjects. No selections means all categories.",
            Self::Kind => "Canon, Corpus, and Collection are saved schema filters only. Other is saved-only except OpenAlex's exact other type. No selections means all types.",

        }
    }
}

impl MetabookApp {
    pub(in crate::app) fn render_search_filters(
        &self,
        id: &'static str,
        cx: &Context<Self>,
    ) -> AnyElement {
        let mut subjects = BTreeSet::new();
        if let Library::Ready(books) = &self.library {
            for book in books {
                if let Some(values) = book.record["book"]["subjects"].as_array() {
                    subjects.extend(values.iter().filter_map(|subject| {
                        subject.as_str().filter(|subject| !subject.is_empty())
                    }));
                }
            }
        }
        // Retain selected subjects while the library reloads or loses a record.
        subjects.extend(
            self.filters
                .category
                .iter()
                .filter_map(|category| category.strip_prefix("subject:"))
                .filter(|subject| !subject.is_empty()),
        );
        div()
            .id(id)
            .role(Role::Group)
            .aria_label("Search filters")
            .aria_description("Select any matching value within Date, Category, or Type; all filters must match. Choose one Source, defaulting to Gutendex. Filters are locked while processing.")
            .w(rems(FILTER_ROW_WIDTH_REMS))
            .max_w_full()
            .min_w_0()
            .flex_none()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(self.filter_select(id, &self.filters.date, date_choices(), FilterField::Date, cx))
            .child(self.filter_select(id, &self.filters.category, category_choices(subjects), FilterField::Category, cx))
            .child(self.filter_select(id, &self.filters.kind, kind_choices(), FilterField::Kind, cx))
            .child(self.render_source_select(id, cx))
            .into_any_element()
    }

    fn filter_select(
        &self,
        id: &'static str,
        selected: &[String],
        choices: Vec<Choice>,
        field: FilterField,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let label = field.label();
        let content = if self.is_processing() {
            let summary = if matches!(field, FilterField::Date) {
                self.filters.date_label()
            } else {
                let empty_label = match field {
                    FilterField::Category => "All categories",
                    FilterField::Kind => "All types",

                    FilterField::Date => "All dates",
                };
                selection_summary(selected, &choices, empty_label)
            };
            div()
                .min_h(theme.control_height(ControlSize::Lg))
                .px_3()
                .py_1()
                .rounded(theme.radius(Radius::Md))
                .border_1()
                .border_color(theme.colors.border)
                .bg(theme.colors.sunken)
                .text_size(theme.text_size(TextSize::Base))
                .text_color(theme.colors.fg_muted)
                .child(summary)
                .into_any_element()
        } else {
            let this = cx.entity();
            MultiSelect::new((gpui::ElementId::from(id), label), choices)
                .selected(selected.iter().cloned())
                .placeholder(label)
                .size(ControlSize::Lg)
                .on_change(move |values: &[SharedString], _, cx| {
                    this.update(cx, |this, cx| {
                        if this.is_processing() {
                            return;
                        }
                        let target = match field {
                            FilterField::Date => &mut this.filters.date,
                            FilterField::Category => &mut this.filters.category,
                            FilterField::Kind => &mut this.filters.kind,
                        };
                        *target = values.iter().map(ToString::to_string).collect();
                        cx.notify();
                    });
                })
                .into_any_element()
        };
        div()
            .id((gpui::ElementId::from(id), format!("{label}-group")))
            .role(Role::Group)
            .aria_label(label)
            .aria_description(field.description())
            .w(rems(FILTER_WIDTH_REMS))
            .max_w_full()
            .min_w_0()
            .flex_none()
            .child(content)
            .into_any_element()
    }

    fn render_source_select(&self, id: &'static str, cx: &Context<Self>) -> AnyElement {
        let this = cx.entity();
        div()
            .id((gpui::ElementId::from(id), "Source-group"))
            .role(Role::Group)
            .aria_label("Source")
            .aria_description("Choose one source. Gutendex is selected by default; Upload is saved-only and Kindle is not connected.")
            .w(rems(FILTER_WIDTH_REMS))
            .max_w_full()
            .min_w_0()
            .flex_none()
            .child(
                Select::new((gpui::ElementId::from(id), "Source"), source_choices())
                    .label("Source")
                    .selected(self.filters.source.clone())
                    .size(ControlSize::Lg)
                    .disabled(self.is_processing())
                    .on_change(move |value, _, cx| {
                        this.update(cx, |this, cx| {
                            if this.is_processing() {
                                return;
                            }
                            this.filters.source = value.to_string();
                            cx.notify();
                        });
                    }),
            )
            .into_any_element()
    }
}

fn date_choices() -> Vec<Choice> {
    DATE_OPTIONS
        .iter()
        .map(|&(value, label)| Choice::new(value, label).note(DATE_DESCRIPTION))
        .collect()
}

fn category_choices(subjects: BTreeSet<&str>) -> Vec<Choice> {
    DOMAIN_OPTIONS
        .iter()
        .map(|&(value, label)| Choice::new(value, label).note("OpenAlex primary-topic domain"))
        .chain(subjects.into_iter().map(|subject| {
            Choice::new(format!("subject:{subject}"), subject.to_string())
                .note("Exact saved subject / Gutendex topic phrase; not an OpenAlex domain")
        }))
        .collect()
}

fn kind_choices() -> Vec<Choice> {
    KIND_OPTIONS
        .iter()
        .map(|&(value, label)| {
            Choice::new(value, label).note(match value {
                "book" => "Gutendex / OpenAlex books; saved books",
                "academic" => "OpenAlex scholarly catalog; saved academic records",
                "canon" | "corpus" | "collection" => {
                    "Saved schema only; unavailable in live catalogs"
                }
                "other" => "Saved-only except OpenAlex's exact other type",
                _ => "Saved library type",
            })
        })
        .collect()
}

fn source_choices() -> Vec<Choice> {
    SOURCE_OPTIONS
        .iter()
        .map(|&(value, label)| {
            let choice = Choice::new(value, label);
            match value {
                "gutendex" => choice.note("Live catalog; selected by default"),
                "openalex" => choice.note("Live scholarly catalog"),
                "upload" => choice.note("Saved records only"),
                "kindle" => choice.note("Not connected yet").disabled(),
                _ => choice,
            }
        })
        .collect()
}

fn selection_summary(selected: &[String], choices: &[Choice], empty_label: &str) -> String {
    if selected.is_empty() {
        return empty_label.to_string();
    }
    selected
        .iter()
        .map(|value| {
            choices
                .iter()
                .find(|choice| &*choice.value == value.as_str())
                .map_or(value.as_str(), |choice| &*choice.label)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_control_has_nonempty_choices_without_all_chips() {
        for choices in [
            date_choices(),
            category_choices(BTreeSet::new()),
            kind_choices(),
            source_choices(),
        ] {
            assert!(!choices.is_empty());
            assert!(choices
                .iter()
                .all(|choice| !choice.value.is_empty() && &*choice.value != "all"));
        }
    }

    #[test]
    fn categories_include_all_domains_even_without_saved_subjects() {
        let choices = category_choices(BTreeSet::new());
        assert_eq!(choices.len(), 4);
        for (choice, &(value, label)) in choices.iter().zip(DOMAIN_OPTIONS) {
            assert_eq!(&*choice.value, value);
            assert_eq!(&*choice.label, label);
            assert_eq!(
                choice.note.as_deref(),
                Some("OpenAlex primary-topic domain")
            );
        }
    }

    #[test]
    fn saved_subjects_remain_distinct_from_domains_and_all_labels() {
        let choices = category_choices(BTreeSet::from(["Life Sciences", "all"]));
        assert_eq!(choices.len(), 6);
        assert!(choices
            .iter()
            .any(|choice| &*choice.value == "subject:Life Sciences"));
        assert!(choices.iter().any(|choice| &*choice.value == "subject:all"));
        assert!(choices.iter().all(|choice| choice.note.is_some()));
    }

    #[test]
    fn sources_identify_live_and_saved_providers_and_disable_only_kindle() {
        for choice in source_choices() {
            assert_eq!(choice.disabled, &*choice.value == "kindle");
            assert!(choice.note.is_some());
            if &*choice.value == "upload" {
                assert_eq!(choice.note.as_deref(), Some("Saved records only"));
            }
        }
    }

    #[test]
    fn date_and_type_choices_explain_catalog_and_saved_semantics() {
        assert!(date_choices()
            .iter()
            .all(|choice| choice.note.as_deref() == Some(DATE_DESCRIPTION)));
        for choice in kind_choices() {
            assert!(choice.note.is_some());
            if matches!(&*choice.value, "canon" | "corpus" | "collection") {
                assert_eq!(
                    choice.note.as_deref(),
                    Some("Saved schema only; unavailable in live catalogs")
                );
            }
        }
    }

    #[test]
    fn processing_summary_uses_labels_and_keeps_unknown_selections_visible() {
        let choices = category_choices(BTreeSet::new());
        assert_eq!(
            selection_summary(&[], &choices, "All categories"),
            "All categories"
        );
        assert_eq!(
            selection_summary(
                &[
                    "domain:1".into(),
                    "domain:4".into(),
                    "subject:History".into()
                ],
                &choices,
                "All categories"
            ),
            "Life Sciences, Health Sciences, subject:History"
        );
    }
}
