//! Shared catalog filter keys and saved-library matching without UI state.

use jiff::Timestamp;
use serde_json::Value;

/// Publication windows in catalog searches, saved activity windows locally.
/// `today` uses UTC; other windows roll, with `year` meaning 365 days.
pub(super) const DATE_OPTIONS: &[(&str, &str)] = &[
    ("today", "Today (UTC)"),
    ("7days", "Past 7 days"),
    ("30days", "Past 30 days"),
    ("year", "Past year"),
];

/// Stable document-type keys; schema types are saved-library filters only.
pub(super) const KIND_OPTIONS: &[(&str, &str)] = &[
    ("book", "Book"),
    ("academic", "Academic"),
    ("canon", "Canon"),
    ("corpus", "Corpus"),
    ("collection", "Collection"),
    ("other", "Other"),
];

pub(super) const SOURCE_OPTIONS: &[(&str, &str)] = &[
    ("gutendex", "Gutendex"),
    ("openalex", "OpenAlex"),
    ("upload", "Upload"),
    ("kindle", "Kindle"),
];

/// OpenAlex primary-topic domains, verified via api.openalex.org/domains.
pub(super) const DOMAIN_OPTIONS: &[(&str, &str)] = &[
    ("domain:1", "Life Sciences"),
    ("domain:2", "Social Sciences"),
    ("domain:3", "Physical Sciences"),
    ("domain:4", "Health Sciences"),
];

const DAY: i64 = 24 * 60 * 60;

/// Source selects one provider, defaulting to Gutendex. Empty selections impose
/// no restriction internally. Categories use `subject:<exact subject>` or `domain:1..4`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SearchFilters {
    pub(super) date: Vec<String>,
    pub(super) category: Vec<String>,
    pub(super) kind: Vec<String>,
    pub(super) source: String,
}

impl Default for SearchFilters {
    fn default() -> Self {
        Self {
            date: Vec::new(),
            category: Vec::new(),
            kind: Vec::new(),
            source: "gutendex".into(),
        }
    }
}

impl SearchFilters {
    /// OR within multi-select dimensions, AND across dimensions and the metadata query.
    /// An empty/whitespace query imposes no search restriction.
    pub(super) fn matches(&self, record: &Value, query: &str, now: Timestamp) -> bool {
        self.matches_date(record, now)
            && (self.category.is_empty()
                || self
                    .category
                    .iter()
                    .any(|category| matches_category(record, category)))
            && (self.kind.is_empty() || self.kind.iter().any(|kind| kind == record_kind(record)))
            && (self.source.is_empty()
                || source_key(&self.source).is_some_and(|source| {
                    record["source"].as_str().and_then(source_key) == Some(source)
                }))
            && matches_query(record, query)
    }

    pub(super) fn date_label(&self) -> String {
        if self.date.is_empty() {
            return "All dates".into();
        }
        self.date
            .iter()
            .map(|date| {
                DATE_OPTIONS
                    .iter()
                    .find(|(key, _)| *key == date)
                    .map_or(date.as_str(), |(_, label)| *label)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn matches_date(&self, record: &Value, now: Timestamp) -> bool {
        if self.date.is_empty() {
            return true;
        }
        let Some(activity) = recorded_activity(record, now) else {
            return false;
        };
        self.date.iter().any(|date| {
            let days = match date.as_str() {
                "today" => {
                    return activity.as_second().div_euclid(DAY) == now.as_second().div_euclid(DAY);
                }
                "7days" => 7,
                "30days" => 30,
                "year" => 365,
                _ => return false,
            };
            now.as_nanosecond() - activity.as_nanosecond() <= i128::from(days * DAY) * 1_000_000_000
        })
    }
}

fn matches_category(record: &Value, category: &str) -> bool {
    if let Some(subject) = category.strip_prefix("subject:") {
        return !subject.is_empty()
            && record["book"]["subjects"]
                .as_array()
                .is_some_and(|subjects| {
                    subjects.iter().any(|value| value.as_str() == Some(subject))
                });
    }
    let Some(domain) = category.strip_prefix("domain:") else {
        return false;
    };
    // Only primary-topic IDs count; subject or display-name guesses do not.
    let primary_domain = [
        &record["primary_topic"]["domain"]["id"],
        &record["book"]["primary_topic"]["domain"]["id"],
    ]
    .into_iter()
    .find_map(domain_id);
    primary_domain == Some(domain)
}

fn domain_id(value: &Value) -> Option<&'static str> {
    match value.as_str() {
        Some("1" | "https://openalex.org/domains/1") => Some("1"),
        Some("2" | "https://openalex.org/domains/2") => Some("2"),
        Some("3" | "https://openalex.org/domains/3") => Some("3"),
        Some("4" | "https://openalex.org/domains/4") => Some("4"),
        _ => match value.as_u64() {
            Some(1) => Some("1"),
            Some(2) => Some("2"),
            Some(3) => Some("3"),
            Some(4) => Some("4"),
            _ => None,
        },
    }
}

/// Latest parseable saved activity, never a publication date. Reject a future
/// latest timestamp rather than falling back to an older one, matching home
/// recents. Callers can sort newest-first with `Reverse(recorded_activity(..))`;
/// `None` then sorts after dated records.
pub(super) fn recorded_activity(record: &Value, now: Timestamp) -> Option<Timestamp> {
    let latest = [
        &record["updated_at"],
        &record["created_at"],
        &record["scan"]["last_scanned_at"],
    ]
    .into_iter()
    .filter_map(|value| value.as_str()?.parse::<Timestamp>().ok())
    .max()?;
    (latest <= now).then_some(latest)
}

fn source_key(source: &str) -> Option<&'static str> {
    match source.trim().to_ascii_lowercase().as_str() {
        "gutenberg" | "gutendex" => Some("gutendex"),
        "upload" => Some("upload"),
        "openalex" => Some("openalex"),
        "kindle" => Some("kindle"),
        _ => None,
    }
}

fn kind_key(kind: &str) -> Option<&'static str> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "book" => Some("book"),
        "academic" => Some("academic"),
        "canon" => Some("canon"),
        "corpus" => Some("corpus"),
        "collection" => Some("collection"),
        "other" => Some("other"),
        _ => None,
    }
}

fn record_kind(record: &Value) -> &'static str {
    let explicit = [record["type"].as_str(), record["book"]["type"].as_str()];
    if let Some(kind) = explicit.into_iter().flatten().find_map(kind_key) {
        return kind;
    }
    let source = record["source"].as_str().and_then(source_key);
    // An unrecognized but nonempty explicit type still prevents guessing that
    // OpenAlex metadata is academic. Known schemas may classify it instead.
    let has_explicit = explicit
        .into_iter()
        .flatten()
        .any(|kind| !kind.trim().is_empty());
    if !has_explicit && source == Some("openalex") {
        return "academic";
    }
    let schema = record["scan"]["schema"]
        .as_str()
        .or_else(|| record["schema"].as_str())
        .or_else(|| record["book"]["schema"].as_str())
        .or_else(|| record["structure"]["schema"].as_str());
    match schema {
        Some("canonical_scripture") => "canon",
        Some("essay_or_story_collection") => "collection",
        Some("standard_book" | "sectioned_book" | "flat") => "book",
        None if matches!(source, Some("gutendex" | "upload")) => "book",
        _ => "other",
    }
}

fn normalized_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn contains_query(value: &Value, query: &str) -> bool {
    value
        .as_str()
        .is_some_and(|text| normalized_text(text).contains(query))
}

fn normalized_isbn(isbn: &str) -> String {
    isbn.chars()
        .filter(|ch| *ch != '-' && !ch.is_whitespace())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

fn contains_isbn_query(value: &Value, query: &str) -> bool {
    let query = normalized_isbn(query);
    !query.is_empty()
        && value
            .as_str()
            .is_some_and(|isbn| normalized_isbn(isbn).contains(&query))
}

fn matches_query(record: &Value, query: &str) -> bool {
    let query = normalized_text(query);
    if query.is_empty() {
        return true;
    }
    let book = &record["book"];
    if contains_query(&book["title"], &query)
        || contains_isbn_query(&book["isbn"], &query)
        || book["authors"].as_array().is_some_and(|authors| {
            authors.iter().any(|author| {
                contains_query(author, &query) || contains_query(&author["name"], &query)
            })
        })
        || book["subjects"].as_array().is_some_and(|subjects| {
            subjects
                .iter()
                .any(|subject| contains_query(subject, &query))
        })
    {
        return true;
    }
    let id_query = query.strip_prefix('#').unwrap_or(&query).trim();
    if id_query.is_empty() {
        return false;
    }
    let id = &record["gutenberg_id"];
    id.as_u64()
        .is_some_and(|id| id.to_string().contains(id_query))
        || contains_query(id, id_query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn now() -> Timestamp {
        "2026-10-08T06:00:00Z".parse().unwrap()
    }

    fn all_dates() -> SearchFilters {
        SearchFilters {
            source: String::new(),
            ..SearchFilters::default()
        }
    }

    fn selected(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn defaults_select_gutendex_without_restricting_other_dimensions() {
        let filters = SearchFilters::default();
        assert!(filters.date.is_empty());
        assert!(filters.category.is_empty());
        assert!(filters.kind.is_empty());
        assert_eq!(filters.source, "gutendex");
        for record in [
            json!({"source": "gutenberg"}),
            json!({"source": "gutendex", "created_at": "1900-01-01T00:00:00Z"}),
            json!({"source": "gutenberg", "created_at": "2026-10-09T06:00:00Z"}),
            json!({"source": "gutendex", "type": "unknown", "book": {"subjects": ["Fiction"]}}),
        ] {
            assert!(filters.matches(&record, "", now()), "{record}");
        }
    }

    #[test]
    fn default_source_retains_gutenberg_and_excludes_uploads_and_other_sources() {
        let filters = SearchFilters::default();
        for (source, expected) in [
            (json!("gutenberg"), true),
            (json!("gutendex"), true),
            (json!("upload"), false),
            (json!("openalex"), false),
            (json!("kindle"), false),
            (json!("unknown"), false),
            (json!(""), false),
            (Value::Null, false),
        ] {
            assert_eq!(
                filters.matches(&json!({"source": source}), "", now()),
                expected,
                "{source}"
            );
        }
        assert!(!filters.matches(&json!({}), "", now()));
    }

    #[test]
    fn empty_internal_source_keeps_saved_dimensions_unrestricted() {
        let filters = all_dates();
        assert!(filters.source.is_empty());
        for record in [
            json!({}),
            json!({"created_at": "1900-01-01T00:00:00Z"}),
            json!({"created_at": "2026-10-09T06:00:00Z"}),
            json!({"source": "unknown", "type": "unknown"}),
        ] {
            assert!(filters.matches(&record, "", now()), "{record}");
        }
    }

    #[test]
    fn choices_have_stable_keys_without_all_chips() {
        for options in [DATE_OPTIONS, KIND_OPTIONS, SOURCE_OPTIONS, DOMAIN_OPTIONS] {
            assert!(!options.is_empty());
            assert!(options
                .iter()
                .all(|(key, label)| { !key.is_empty() && *key != "all" && !label.is_empty() }));
        }
        assert!(!KIND_OPTIONS.iter().any(|(key, _)| *key == "kindle"));
        assert!(SOURCE_OPTIONS.iter().any(|(key, _)| *key == "kindle"));
        assert!(SOURCE_OPTIONS.contains(&("gutendex", "Gutendex")));
        assert!(DATE_OPTIONS.contains(&("today", "Today (UTC)")));
        assert_eq!(
            DOMAIN_OPTIONS,
            &[
                ("domain:1", "Life Sciences"),
                ("domain:2", "Social Sciences"),
                ("domain:3", "Physical Sciences"),
                ("domain:4", "Health Sciences"),
            ]
        );
    }

    #[test]
    fn date_labels_cover_empty_single_and_multiple_selections() {
        assert_eq!(SearchFilters::default().date_label(), "All dates");
        let mut filters = SearchFilters {
            date: selected(&["7days"]),
            ..SearchFilters::default()
        };
        assert_eq!(filters.date_label(), "Past 7 days");
        filters.date = selected(&["today", "30days", "year"]);
        assert_eq!(filters.date_label(), "Today (UTC), Past 30 days, Past year");
        filters.date = selected(&["unknown"]);
        assert_eq!(filters.date_label(), "unknown");
    }

    #[test]
    fn seven_days_keeps_boundary_and_excludes_old_future_or_missing_dates() {
        let filters = SearchFilters {
            date: selected(&["7days"]),
            ..all_dates()
        };
        for (date, expected) in [
            ("2026-10-01T06:00:00Z", true),
            ("2026-10-01T05:59:59Z", false),
            ("2026-10-08T06:00:00Z", true),
            ("2026-10-09T06:00:00Z", false),
            ("broken", false),
        ] {
            assert_eq!(
                filters.matches(&json!({"created_at": date}), "", now()),
                expected,
                "{date}"
            );
        }
        assert!(!filters.matches(&json!({}), "", now()));
    }

    #[test]
    fn rolling_windows_are_inclusive_and_use_precise_activity_not_publication() {
        for (date, boundary, outside) in [
            (
                "7days",
                "2026-10-01T06:00:00Z",
                "2026-10-01T05:59:59.999999999Z",
            ),
            ("30days", "2026-09-08T06:00:00Z", "2026-09-08T05:59:59Z"),
            ("year", "2025-10-08T06:00:00Z", "2025-10-08T05:59:59Z"),
        ] {
            let filters = SearchFilters {
                date: selected(&[date]),
                ..all_dates()
            };
            assert!(filters.matches(
                &json!({"created_at": boundary, "book": {"date": "1800"}}),
                "",
                now()
            ));
            assert!(!filters.matches(
                &json!({"created_at": outside, "book": {"date": "2026-10-08"}}),
                "",
                now()
            ));
        }
        let filters = SearchFilters {
            date: selected(&["7days"]),
            ..all_dates()
        };
        assert!(!filters.matches(&json!({"book": {"date": "2026-10-08"}}), "", now()));
    }

    #[test]
    fn today_starts_at_utc_midnight_not_twenty_four_hours_ago() {
        let filters = SearchFilters {
            date: selected(&["today"]),
            ..all_dates()
        };
        for (date, expected) in [
            ("2026-10-08T00:00:00Z", true),
            ("2026-10-07T20:00:00-04:00", true),
            ("2026-10-07T23:59:59Z", false),
            ("2026-10-08T06:00:01Z", false),
        ] {
            assert_eq!(
                filters.matches(&json!({"updated_at": date}), "", now()),
                expected
            );
        }
    }

    #[test]
    fn all_dates_include_old_and_undated_records() {
        for record in [
            json!({}),
            json!({"created_at": "1900-01-01T00:00:00Z"}),
            json!({"created_at": "broken"}),
        ] {
            assert!(all_dates().matches(&record, " \t\n ", now()));
        }
    }

    #[test]
    fn activity_uses_latest_valid_field_and_parses_timezone_offsets() {
        let filters = SearchFilters {
            date: selected(&["7days"]),
            ..all_dates()
        };
        let expected: Timestamp = "2026-10-08T05:00:00Z".parse().unwrap();
        let mut record = json!({"created_at": "2025-01-01T00:00:00Z", "updated_at": "2026-10-08T01:00:00-04:00", "scan": {"last_scanned_at": "2026-10-07T00:00:00Z"}});
        assert_eq!(recorded_activity(&record, now()), Some(expected));
        record["updated_at"] = json!("broken");
        record["scan"]["last_scanned_at"] = json!("2026-10-08T05:00:00Z");
        assert_eq!(recorded_activity(&record, now()), Some(expected));
        assert!(filters.matches(&record, "", now()));
    }

    #[test]
    fn future_latest_activity_never_falls_back_to_an_older_valid_date() {
        let filters = SearchFilters {
            date: selected(&["today", "7days", "30days", "year"]),
            ..all_dates()
        };
        for field in ["created_at", "updated_at", "last_scanned_at"] {
            let mut record = json!({"created_at": "2026-10-07T00:00:00Z", "scan": {}});
            if field == "last_scanned_at" {
                record["scan"][field] = json!("2026-10-09T00:00:00Z");
            } else {
                record[field] = json!("2026-10-09T00:00:00Z");
            }
            assert_eq!(recorded_activity(&record, now()), None);
            assert!(!filters.matches(&record, "", now()));
        }
    }

    #[test]
    fn categories_are_namespaced_exact_subjects_without_sentinel_collisions() {
        let record = json!({"book": {"subjects": ["all", "History -- France", "domain:1"]}});
        for (category, expected) in [
            ("subject:all", true),
            ("subject:History -- France", true),
            ("subject:history -- france", false),
            ("subject:History", false),
            ("subject:domain:1", true),
            ("domain:1", false),
            ("History -- France", false),
            ("all", false),
            ("", false),
            ("subject:", false),
        ] {
            let filters = SearchFilters {
                category: selected(&[category]),
                ..all_dates()
            };
            assert_eq!(filters.matches(&record, "", now()), expected, "{category}");
        }
        let filters = SearchFilters {
            category: selected(&["subject:all"]),
            ..all_dates()
        };
        assert!(!filters.matches(&json!({"book": {"subjects": ["History"]}}), "", now()));
        assert!(all_dates().matches(&record, "", now()));
    }

    #[test]
    fn domain_categories_use_primary_topic_ids_at_record_or_book_level() {
        for domain in 1..=4 {
            let filters = SearchFilters {
                category: vec![format!("domain:{domain}")],
                ..all_dates()
            };
            for id in [
                json!(domain),
                json!(domain.to_string()),
                json!(format!("https://openalex.org/domains/{domain}")),
            ] {
                for record in [
                    json!({"primary_topic": {"domain": {"id": id}}}),
                    json!({"book": {"primary_topic": {"domain": {"id": id}}}}),
                ] {
                    assert!(filters.matches(&record, "", now()), "{record}");
                }
            }
            assert!(!filters.matches(
                &json!({"primary_topic": {"domain": {"id": domain % 4 + 1}}}),
                "",
                now()
            ));
        }
    }

    #[test]
    fn domains_are_never_guessed_from_subjects_labels_or_secondary_topics() {
        let filters = SearchFilters {
            category: selected(&["domain:1", "domain:2", "domain:3", "domain:4"]),
            ..all_dates()
        };
        for record in [
            json!({"book": {"subjects": ["Life Sciences", "Health Sciences"]}}),
            json!({"primary_topic": {"domain": {"display_name": "Life Sciences"}}}),
            json!({"primary_topic": {"domain": {"id": "Life Sciences"}}}),
            json!({"primary_topic": {"domain": {"id": "https://example.com/domains/1"}}}),
            json!({"primary_topic": {"domain": {"id": 5, "display_name": "Life Sciences"}}}),
            json!({"topics": [{"domain": {"id": "https://openalex.org/domains/1"}}]}),
        ] {
            assert!(!filters.matches(&record, "", now()), "{record}");
        }
        for category in ["domain:0", "domain:5", "domain:Life Sciences", "domain:"] {
            let filters = SearchFilters {
                category: selected(&[category]),
                ..all_dates()
            };
            assert!(!filters.matches(&json!({"primary_topic": {"domain": {"id": 1}}}), "", now()));
        }
    }

    #[test]
    fn dates_match_any_selected_window() {
        let filters = SearchFilters {
            date: selected(&["today", "7days", "30days"]),
            ..all_dates()
        };
        for (date, expected) in [
            ("2026-10-08T05:00:00Z", true),
            ("2026-10-04T00:00:00Z", true),
            ("2026-09-20T00:00:00Z", true),
            ("2026-09-01T00:00:00Z", false),
        ] {
            assert_eq!(
                filters.matches(&json!({"created_at": date}), "", now()),
                expected
            );
        }
    }

    #[test]
    fn categories_match_any_selected_subject_or_domain() {
        let filters = SearchFilters {
            category: selected(&["subject:History", "subject:Fiction", "domain:3"]),
            ..all_dates()
        };
        for (record, expected) in [
            (json!({"book": {"subjects": ["History"]}}), true),
            (json!({"book": {"subjects": ["Fiction"]}}), true),
            (json!({"primary_topic": {"domain": {"id": 3}}}), true),
            (json!({"book": {"subjects": ["Poetry"]}}), false),
            (json!({"primary_topic": {"domain": {"id": 4}}}), false),
        ] {
            assert_eq!(filters.matches(&record, "", now()), expected, "{record}");
        }
    }

    #[test]
    fn kinds_match_any_selected_classification() {
        let filters = SearchFilters {
            kind: selected(&["canon", "academic"]),
            ..all_dates()
        };
        for (record, expected) in [
            (json!({"scan": {"schema": "canonical_scripture"}}), true),
            (json!({"source": "openalex"}), true),
            (json!({"source": "gutendex"}), false),
            (json!({"type": "corpus"}), false),
        ] {
            assert_eq!(filters.matches(&record, "", now()), expected, "{record}");
        }
    }

    #[test]
    fn selecting_another_source_replaces_previous_provider() {
        let mut filters = SearchFilters::default();
        for selected_source in ["gutendex", "upload", "openalex", "kindle"] {
            filters.source = selected_source.into();
            for source in [
                "gutenberg",
                "gutendex",
                "upload",
                "openalex",
                "kindle",
                "unknown",
            ] {
                let provider = if source == "gutenberg" {
                    "gutendex"
                } else {
                    source
                };
                assert_eq!(
                    filters.matches(&json!({"source": source}), "", now()),
                    provider == selected_source,
                    "selected {selected_source}, record {source}"
                );
            }
        }
    }

    #[test]
    fn invalid_keys_do_not_mean_all_but_valid_alternatives_still_match() {
        let record = json!({"source": "gutendex", "created_at": "2026-10-08T05:00:00Z", "book": {"subjects": ["Fiction"]}});
        for key in ["all", "", "unknown"] {
            for filters in [
                SearchFilters {
                    date: selected(&[key]),
                    ..all_dates()
                },
                SearchFilters {
                    category: selected(&[key]),
                    ..all_dates()
                },
                SearchFilters {
                    kind: selected(&[key]),
                    ..all_dates()
                },
            ] {
                assert!(!filters.matches(&record, "", now()), "{filters:?}");
            }
            let filters = SearchFilters {
                date: selected(&[key, "today"]),
                category: selected(&[key, "subject:Fiction"]),
                kind: selected(&[key, "book"]),
                source: "gutendex".into(),
            };
            assert!(filters.matches(&record, "", now()));
        }
    }

    #[test]
    fn invalid_source_keys_do_not_mean_all_or_combine_providers() {
        for source in ["all", "unknown", "gutendex,upload", "gutendex + upload"] {
            let filters = SearchFilters {
                source: source.into(),
                ..all_dates()
            };
            for record_source in [
                "gutenberg",
                "gutendex",
                "upload",
                "openalex",
                "kindle",
                "unknown",
            ] {
                assert!(
                    !filters.matches(&json!({"source": record_source}), "", now()),
                    "selected {source}, record {record_source}"
                );
            }
            assert!(!filters.matches(&json!({}), "", now()));
        }
    }

    #[test]
    fn sources_accept_gutenberg_aliases_and_keep_other_sources_distinct() {
        for (selected, source, expected) in [
            ("gutendex", "gutenberg", true),
            ("gutendex", "gutendex", true),
            ("gutenberg", "gutendex", true),
            (" GUTENBERG ", " Gutendex ", true),
            ("upload", "upload", true),
            ("openalex", "openalex", true),
            ("kindle", "kindle", true),
            ("gutendex", "upload", false),
            ("upload", "openalex", false),
        ] {
            let filters = SearchFilters {
                source: selected.to_string(),
                ..all_dates()
            };
            assert_eq!(
                filters.matches(&json!({"source": source}), "", now()),
                expected
            );
        }
    }

    #[test]
    fn schema_taxonomy_and_known_missing_schema_sources_classify_conservatively() {
        for (schema, kind) in [
            ("canonical_scripture", "canon"),
            ("essay_or_story_collection", "collection"),
            ("standard_book", "book"),
            ("sectioned_book", "book"),
            ("flat", "book"),
            ("unknown_schema", "other"),
        ] {
            let filters = SearchFilters {
                kind: selected(&[kind]),
                ..all_dates()
            };
            for record in [
                json!({"scan": {"schema": schema}}),
                json!({"structure": {"schema": schema}}),
            ] {
                assert!(filters.matches(&record, "", now()));
            }
        }
        for (source, kind) in [
            ("gutenberg", "book"),
            ("gutendex", "book"),
            ("upload", "book"),
            ("openalex", "academic"),
            ("kindle", "other"),
            ("unknown", "other"),
        ] {
            assert_eq!(record_kind(&json!({"source": source})), kind);
        }
        assert_eq!(record_kind(&json!({})), "other");
        assert_eq!(
            record_kind(&json!({"source": "upload", "scan": {"schema": "unknown"}})),
            "other"
        );
    }

    #[test]
    fn kindle_is_a_source_not_a_document_type() {
        assert_eq!(kind_key("kindle"), None);
        for record in [
            json!({"type": "kindle"}),
            json!({"book": {"type": "kindle"}}),
        ] {
            assert_eq!(record_kind(&record), "other");
            let filters = SearchFilters {
                kind: selected(&["kindle"]),
                ..all_dates()
            };
            assert!(!filters.matches(&record, "", now()));
        }
        let filters = SearchFilters {
            source: "kindle".into(),
            kind: selected(&["book"]),
            ..all_dates()
        };
        assert!(filters.matches(
            &json!({"source": "kindle", "structure": {"schema": "standard_book"}}),
            "",
            now()
        ));
    }

    #[test]
    fn structure_schema_fallback_preserves_existing_schema_and_type_precedence() {
        for record in [
            json!({"scan": {"schema": "standard_book"}, "structure": {"schema": "canonical_scripture"}}),
            json!({"schema": "standard_book", "structure": {"schema": "canonical_scripture"}}),
            json!({"book": {"schema": "standard_book"}, "structure": {"schema": "canonical_scripture"}}),
            json!({"type": "book", "structure": {"schema": "canonical_scripture"}}),
            json!({"book": {"type": "book"}, "structure": {"schema": "canonical_scripture"}}),
        ] {
            assert_eq!(record_kind(&record), "book");
        }
        assert_eq!(
            record_kind(
                &json!({"scan": {"schema": null}, "structure": {"schema": "canonical_scripture"}})
            ),
            "canon"
        );
    }

    #[test]
    fn recognized_explicit_types_override_source_and_schema() {
        for kind in ["book", "academic", "canon", "corpus", "collection", "other"] {
            let filters = SearchFilters {
                kind: selected(&[kind]),
                ..all_dates()
            };
            for record in [
                json!({"type": kind, "source": "openalex", "scan": {"schema": "canonical_scripture"}}),
                json!({"book": {"type": kind}, "source": "openalex", "scan": {"schema": "standard_book"}}),
            ] {
                assert!(filters.matches(&record, "", now()));
            }
        }
        assert_eq!(
            record_kind(&json!({"type": "corpus", "book": {"type": "academic"}})),
            "corpus"
        );
        assert_eq!(
            record_kind(&json!({"type": "unknown", "book": {"type": "canon"}})),
            "canon"
        );
    }

    #[test]
    fn openalex_inference_requires_absent_or_empty_explicit_type() {
        for record in [
            json!({"type": "journal"}),
            json!({"book": {"type": "journal"}}),
        ] {
            let mut record = record;
            record["source"] = json!("openalex");
            assert_eq!(record_kind(&record), "other");
        }
        assert_eq!(
            record_kind(&json!({"source": "openalex", "type": "", "scan": {"schema": "flat"}})),
            "academic"
        );
        assert_eq!(
            record_kind(
                &json!({"source": "openalex", "type": "unknown", "scan": {"schema": "canonical_scripture"}})
            ),
            "canon"
        );
    }

    #[test]
    fn isbn_queries_ignore_hyphens_and_whitespace_in_both_directions() {
        let forms = [
            "9780141439518",
            "978-0-14-143951-8",
            "978 0 14 143951 8",
            "978\t0\u{00a0}14\n143951 8",
        ];
        for isbn in forms {
            let record = json!({"book": {"isbn": isbn}});
            for query in forms {
                assert!(
                    all_dates().matches(&record, query, now()),
                    "{isbn:?} / {query:?}"
                );
            }
            for query in ["978-0-14-143951-9", "-", " - \t - "] {
                assert!(
                    !all_dates().matches(&record, query, now()),
                    "{isbn:?} / {query:?}"
                );
            }
        }
    }

    #[test]
    fn isbn10_check_digit_x_is_case_insensitive() {
        for isbn in ["080442957X", "0-8044-2957-x", "0 8044 2957 X"] {
            let record = json!({"book": {"isbn": isbn}});
            for query in ["080442957x", "0-8044-2957-X", "0 8044 2957 x"] {
                assert!(
                    all_dates().matches(&record, query, now()),
                    "{isbn:?} / {query:?}"
                );
            }
            assert!(!all_dates().matches(&record, "0804429570", now()));
        }
    }

    #[test]
    fn isbn_separator_normalization_does_not_change_title_or_subject_search() {
        let record =
            json!({"book": {"title": "Pride-and Prejudice", "subjects": ["Science Fiction"]}});
        for query in ["PRIDE-AND PREJUDICE", "science fiction"] {
            assert!(all_dates().matches(&record, query, now()));
        }
        for query in ["prideandprejudice", "sciencefiction", "science-fiction"] {
            assert!(!all_dates().matches(&record, query, now()));
        }
        for record in [
            json!({"book": {"title": "9780141439518"}}),
            json!({"book": {"subjects": ["9780141439518"]}}),
        ] {
            assert!(all_dates().matches(&record, "9780141439518", now()));
            assert!(!all_dates().matches(&record, "978-0-14-143951-8", now()));
            assert!(!all_dates().matches(&record, "978 0 14 143951 8", now()));
        }
    }

    #[test]
    fn query_searches_only_supported_metadata_and_normalizes_case_and_whitespace() {
        let record = json!({
            "gutenberg_id": 1342,
            "book": {"title": "Pride  and\nPrejudice", "authors": [{"name": "Austen, Jane", "biography": "not searchable"}, "Second Author"], "isbn": "9780141439518", "subjects": ["Courtship -- Fiction"]},
            "private_notes": "hidden term", "scan": {"schema": "canonical_scripture"}
        });
        for query in [
            "  PRIDE\tand   prejudice \n",
            "jAnE",
            "second author",
            "9780141439518",
            "1342",
            "#1342",
            " # 1342 ",
            "courtship -- fiction",
            " \t ",
        ] {
            assert!(all_dates().matches(&record, query, now()), "{query:?}");
        }
        for query in [
            "missing title",
            "#",
            "#99999",
            "not searchable",
            "hidden term",
            "canonical_scripture",
        ] {
            assert!(!all_dates().matches(&record, query, now()), "{query:?}");
        }
        assert!(all_dates().matches(&json!({"gutenberg_id": "1342"}), "#1342", now()));
    }

    #[test]
    fn combined_filters_require_every_dimension_without_schema_score() {
        let record = json!({"source": "gutenberg", "created_at": "2026-10-07T00:00:00Z", "book": {"title": "Selected Stories", "subjects": ["Fiction"]}, "scan": {"schema": "essay_or_story_collection"}});
        let filters = SearchFilters {
            date: selected(&["today", "7days"]),
            category: selected(&["subject:History", "subject:Fiction"]),
            kind: selected(&["book", "collection"]),
            source: "gutendex".into(),
        };
        assert!(filters.matches(&record, "stories", now()));
        assert!(!filters.matches(&record, "history", now()));
        for mismatched in [
            SearchFilters {
                date: selected(&["today", "unknown"]),
                ..filters.clone()
            },
            SearchFilters {
                category: selected(&["subject:History", "domain:2"]),
                ..filters.clone()
            },
            SearchFilters {
                kind: selected(&["book", "academic"]),
                ..filters.clone()
            },
            SearchFilters {
                source: "upload".into(),
                ..filters.clone()
            },
        ] {
            assert!(!mismatched.matches(&record, "stories", now()));
        }
    }

    #[test]
    fn malformed_shapes_do_not_panic_or_match_nonempty_queries() {
        for record in [
            Value::Null,
            json!([]),
            json!(42),
            json!({"book": {"title": 7, "authors": [null, 4], "subjects": false}, "scan": []}),
        ] {
            assert_eq!(recorded_activity(&record, now()), None);
            assert!(!all_dates().matches(&record, "title", now()));
        }
    }
}
