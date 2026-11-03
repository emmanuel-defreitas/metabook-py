//! Live catalog metadata, kept separate from structural analysis and saved books.

use serde_json::Value;

use super::{
    read_body, search_input, status_message, unreachable_message, SearchInput, SearchOutcome,
    TIMEOUT,
};

/// Transport keys are sent unchanged so the API can validate provider capabilities.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CatalogFilters {
    pub date: Vec<String>,
    pub category: Vec<String>,
    pub kind: Vec<String>,
    pub source: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogSource {
    Gutendex(u64),
    OpenAlex(String),
}

impl CatalogSource {
    pub fn key(&self) -> String {
        match self {
            Self::Gutendex(id) => format!("gutendex:{id}"),
            Self::OpenAlex(id) => format!("openalex:{id}"),
        }
    }

    /// Build a canonical link from an ID, never from a provider-supplied URL.
    pub fn openalex_url(&self) -> Option<String> {
        match self {
            Self::OpenAlex(id) if valid_openalex_id(id) => {
                Some(format!("https://openalex.org/{id}"))
            }
            _ => None,
        }
    }
}

/// Catalog metadata only; no structural analysis or heuristic score is inferred.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookMatch {
    pub source: CatalogSource,
    pub title: String,
    pub authors: String,
    pub language: String,
    pub publication_date: Option<String>,
    pub kind: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchPage {
    pub query: String,
    pub filters: CatalogFilters,
    pub count: u64,
    pub page: u64,
    pub next_page: Option<u64>,
    pub previous_page: Option<u64>,
    pub matches: Vec<BookMatch>,
    pub warnings: Vec<String>,
}

/// GET /api/books/catalog/search — one page of live metadata, without scanning.
pub fn search(
    base: &str,
    query: &str,
    page: u64,
    filters: &CatalogFilters,
) -> Result<SearchOutcome, String> {
    match search_request(base, query, page, filters).call() {
        Ok(resp) => {
            let text = read_body(resp)?;
            let value = serde_json::from_str(&text)
                .map_err(|err| format!("Invalid JSON from the API: {err}"))?;
            parse_search_page(&value, query, filters).map(SearchOutcome::Matches)
        }
        Err(ureq::Error::Status(code, resp)) => Err(status_message(code, resp)),
        Err(err) => Err(unreachable_message(base, &err)),
    }
}

fn search_request(base: &str, query: &str, page: u64, filters: &CatalogFilters) -> ureq::Request {
    let request = ureq::get(&format!("{base}/api/books/catalog/search"))
        .query("page", &page.to_string())
        .timeout(TIMEOUT);
    let mut request = match search_input(query) {
        SearchInput::Gutenberg(id) => request.query("gutenberg_id", &id.to_string()),
        SearchInput::Isbn(isbn) => request.query("isbn", &isbn),
        SearchInput::Title(title) => request.query("q", &title),
    };
    for (key, values) in [
        ("date", &filters.date),
        ("category", &filters.category),
        ("kind", &filters.kind),
        ("source", &filters.source),
    ] {
        for value in values {
            request = request.query(key, value);
        }
    }
    request
}

fn parse_search_page(
    value: &Value,
    query: &str,
    filters: &CatalogFilters,
) -> Result<SearchPage, String> {
    let matches = value["results"]
        .as_array()
        .ok_or("The API returned an invalid catalog result list.")?
        .iter()
        .map(parse_match)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SearchPage {
        query: query.to_string(),
        filters: filters.clone(),
        count: value["count"]
            .as_u64()
            .ok_or("Missing search result count.")?,
        page: value["page"]
            .as_u64()
            .filter(|page| *page > 0)
            .ok_or("Missing or invalid search result page.")?,
        next_page: optional_page(value, "next_page")?,
        previous_page: optional_page(value, "previous_page")?,
        matches,
        warnings: string_list(value, "warnings")?
            .into_iter()
            .map(str::to_string)
            .collect(),
    })
}

fn parse_match(entry: &Value) -> Result<BookMatch, String> {
    Ok(BookMatch {
        source: parse_source(entry)?,
        title: entry["title"]
            .as_str()
            .filter(|title| !title.is_empty())
            .ok_or("The API returned an invalid catalog title.")?
            .to_string(),
        authors: string_list(entry, "authors")?.join(", "),
        language: text_field(entry, "language")?.to_string(),
        publication_date: match entry.get("publication_date") {
            None | Some(Value::Null) => None,
            Some(date) => Some(
                date.as_str()
                    .ok_or("The API returned an invalid catalog publication date.")?
                    .to_string(),
            ),
        },
        kind: text_field(entry, "kind")?.to_string(),
    })
}

fn parse_source(entry: &Value) -> Result<CatalogSource, String> {
    let invalid = || "The API returned an invalid catalog source or identifier.".to_string();
    let id = entry["id"].as_str().ok_or_else(invalid)?;
    match entry["source"].as_str() {
        Some("gutendex") => {
            let gutenberg_id = id
                .parse::<u64>()
                .ok()
                .filter(|id| *id > 0)
                .ok_or_else(invalid)?;
            if !id.bytes().all(|byte| byte.is_ascii_digit())
                || entry["gutenberg_id"].as_u64() != Some(gutenberg_id)
            {
                return Err(invalid());
            }
            Ok(CatalogSource::Gutendex(gutenberg_id))
        }
        Some("openalex") if valid_openalex_id(id) && entry["gutenberg_id"].is_null() => {
            Ok(CatalogSource::OpenAlex(id.to_string()))
        }
        _ => Err(invalid()),
    }
}

fn valid_openalex_id(id: &str) -> bool {
    let Some(digits) = id.strip_prefix('W') else {
        return false;
    };
    matches!(digits.as_bytes().first(), Some(b'1'..=b'9'))
        && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn string_list<'a>(value: &'a Value, field: &str) -> Result<Vec<&'a str>, String> {
    match value.get(field) {
        None => Ok(Vec::new()),
        Some(Value::Array(entries)) => entries
            .iter()
            .map(|entry| {
                entry
                    .as_str()
                    .ok_or_else(|| format!("The API returned an invalid catalog {field} list."))
            })
            .collect(),
        _ => Err(format!("The API returned an invalid catalog {field} list.")),
    }
}

fn text_field<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    match value.get(field) {
        None => Ok(""),
        Some(text) => text
            .as_str()
            .ok_or_else(|| format!("The API returned an invalid catalog {field}.")),
    }
}

fn optional_page(value: &Value, field: &str) -> Result<Option<u64>, String> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(page) => page
            .as_u64()
            .filter(|page| *page > 0)
            .map(Some)
            .ok_or_else(|| format!("The API returned an invalid catalog {field}.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn search_page_preserves_metadata_navigation_and_warnings() {
        let value = json!({
            "count": 70, "page": 2, "next_page": 3, "previous_page": 1,
            "results": [{
                "source": "gutendex", "id": "1342", "gutenberg_id": 1342,
                "title": "Pride and Prejudice", "authors": ["Austen, Jane", "Second Author"],
                "language": "en,fr", "publication_date": null, "kind": "book"
            }, {
                "source": "openalex", "id": "W1342", "gutenberg_id": null,
                "title": "A Study", "authors": ["Researcher"], "language": "en",
                "publication_date": "2024-05-12", "kind": "article"
            }],
            "warnings": ["Paging limit reached.", "Narrow the query or filters."]
        });
        let page = parse_search_page(&value, "Austen", &CatalogFilters::default()).unwrap();
        assert_eq!((page.count, page.page), (70, 2));
        assert_eq!((page.next_page, page.previous_page), (Some(3), Some(1)));
        assert_eq!(page.query, "Austen");
        assert_eq!(page.matches[0].source, CatalogSource::Gutendex(1342));
        assert_eq!(page.matches[0].title, "Pride and Prejudice");
        assert_eq!(page.matches[0].authors, "Austen, Jane, Second Author");
        assert_eq!(page.matches[0].language, "en,fr");
        assert_eq!(page.matches[0].publication_date, None);
        assert_eq!(page.matches[0].kind, "book");
        assert_eq!(
            page.matches[1].source,
            CatalogSource::OpenAlex("W1342".into())
        );
        assert_eq!(
            page.matches[1].publication_date.as_deref(),
            Some("2024-05-12")
        );
        assert_eq!(page.matches[1].kind, "article");
        assert_eq!(
            page.warnings,
            ["Paging limit reached.", "Narrow the query or filters."]
        );
    }

    #[test]
    fn catalog_results_without_scores_remain_metadata_only() {
        let entry = json!({
            "source": "openalex", "id": "W42", "title": "A Study",
            "authors": [], "language": "", "publication_date": null, "kind": "article"
        });
        assert_eq!(
            parse_match(&entry).unwrap(),
            BookMatch {
                source: CatalogSource::OpenAlex("W42".into()),
                title: "A Study".into(),
                authors: String::new(),
                language: String::new(),
                publication_date: None,
                kind: "article".into(),
            }
        );
    }

    #[test]
    fn search_page_accepts_empty_results_and_rejects_missing_list() {
        let filters = CatalogFilters::default();
        let empty = json!({"count": 0, "page": 1, "results": []});
        let page = parse_search_page(&empty, "unknown", &filters).unwrap();
        assert!(page.matches.is_empty());
        assert!(page.warnings.is_empty());
        assert_eq!(page.next_page, None);
        assert_eq!(page.previous_page, None);
        assert!(parse_search_page(&json!({"count": 0, "page": 1}), "unknown", &filters).is_err());
    }

    #[test]
    fn search_page_rejects_invalid_metadata_instead_of_discarding_it() {
        for (field, invalid) in [
            ("count", json!(-1)),
            ("page", json!(0)),
            ("next_page", json!("3")),
            ("previous_page", json!(0)),
            ("results", json!({})),
            ("warnings", json!(["Valid warning", 7])),
        ] {
            let mut value = json!({"count": 0, "page": 1, "results": []});
            value[field] = invalid;
            assert!(parse_search_page(&value, "query", &CatalogFilters::default()).is_err());
        }
    }

    #[test]
    fn source_mismatches_and_invalid_identifiers_fail_the_entire_result_list() {
        for identity in [
            json!({"source": "gutendex", "id": "1342", "gutenberg_id": 42}),
            json!({"source": "gutendex", "id": "W1342", "gutenberg_id": 1342}),
            json!({"source": "gutendex", "id": "1342"}),
            json!({"source": "gutendex", "id": "0", "gutenberg_id": 0}),
            json!({"source": "gutendex", "id": "+1342", "gutenberg_id": 1342}),
            json!({"source": "gutendex", "id": 1342, "gutenberg_id": 1342}),
            json!({"source": "openalex", "id": "1342"}),
            json!({"source": "openalex", "id": "W1342", "gutenberg_id": 1342}),
            json!({"source": "openalex", "id": "W1342", "gutenberg_id": "1342"}),
            json!({"source": "unknown", "id": "W1342"}),
            json!({"id": "W1342"}),
        ] {
            let mut invalid = identity;
            invalid["title"] = json!("Invalid match");
            let value = json!({
                "count": 2, "page": 1,
                "results": [
                    {"source": "gutendex", "id": "1342", "gutenberg_id": 1342, "title": "Valid"},
                    invalid
                ]
            });
            assert!(parse_search_page(&value, "query", &CatalogFilters::default()).is_err());
        }
    }

    #[test]
    fn malformed_result_fields_are_not_silently_defaulted_or_filtered() {
        for (field, invalid) in [
            ("title", json!(null)),
            ("authors", json!(["Valid author", 7])),
            ("language", json!(7)),
            ("publication_date", json!(7)),
            ("kind", json!([])),
        ] {
            let mut entry = json!({"source": "openalex", "id": "W42", "title": "A Study"});
            entry[field] = invalid;
            assert!(parse_match(&entry).is_err());
        }
    }

    #[test]
    fn equal_numeric_ids_have_distinct_stable_source_keys() {
        let gutendex = CatalogSource::Gutendex(1342);
        let openalex = CatalogSource::OpenAlex("W1342".into());
        assert_eq!(gutendex.key(), "gutendex:1342");
        assert_eq!(openalex.key(), "openalex:W1342");
        assert_ne!(gutendex.key(), openalex.key());
        assert_eq!(gutendex.openalex_url(), None);
    }

    #[test]
    fn openalex_links_ignore_malicious_returned_urls() {
        for url in [
            "javascript:alert(1)",
            "https://evil.example/W1342",
            "https://openalex.org.evil.example/W1342",
            "file:///etc/passwd",
        ] {
            let entry = json!({
                "source": "openalex", "id": "W1342", "title": "A Study", "url": url
            });
            let book = parse_match(&entry).unwrap();
            assert_eq!(
                book.source.openalex_url().as_deref(),
                Some("https://openalex.org/W1342")
            );
        }
    }

    #[test]
    fn openalex_ids_require_exact_nonzero_work_digits() {
        for id in [
            "",
            "W",
            "W0",
            "W01",
            "W-1",
            "W+1",
            "w1",
            "W1x",
            "W1/evil",
            "W1?url=evil",
            "W1#evil",
            "W1\n",
            "W１",
            "https://openalex.org/W1",
        ] {
            let entry = json!({"source": "openalex", "id": id, "title": "A Study"});
            assert!(parse_match(&entry).is_err(), "accepted invalid ID: {id:?}");
            assert_eq!(CatalogSource::OpenAlex(id.into()).openalex_url(), None);
        }
        for id in ["W1", "W1342", "W18446744073709551616"] {
            let entry = json!({"source": "openalex", "id": id, "title": "A Study"});
            assert!(parse_match(&entry).is_ok());
            assert_eq!(
                CatalogSource::OpenAlex(id.into()).openalex_url(),
                Some(format!("https://openalex.org/{id}"))
            );
        }
    }

    #[test]
    fn request_repeats_each_filter_key_and_encodes_values_before_call() {
        let filters = CatalogFilters {
            date: vec!["7days".into(), "year".into()],
            category: vec!["subject:Science & art".into(), "domain:2".into()],
            kind: vec!["book".into(), "academic".into()],
            source: vec!["gutendex".into(), "openalex".into()],
        };
        let request = search_request("http://localhost:8000", "Austen & Co", 2, &filters);
        assert_eq!(
            request.url(),
            concat!(
                "http://localhost:8000/api/books/catalog/search?page=2&q=Austen+%26+Co",
                "&date=7days&date=year",
                "&category=subject%3AScience+%26+art&category=domain%3A2",
                "&kind=book&kind=academic&source=gutendex&source=openalex"
            )
        );
    }

    #[test]
    fn request_preserves_title_isbn_and_gutenberg_input_routing() {
        for (query, expected) in [
            (" Pride and Prejudice ", "q=Pride+and+Prejudice"),
            ("978-0-14-143951-8", "isbn=9780141439518"),
            ("0-8044-2957-x", "isbn=080442957X"),
            ("#1342", "gutenberg_id=1342"),
            ("1342", "gutenberg_id=1342"),
        ] {
            let request = search_request(
                "http://localhost:8000",
                query,
                1,
                &CatalogFilters::default(),
            );
            assert_eq!(
                request.url(),
                format!("http://localhost:8000/api/books/catalog/search?page=1&{expected}")
            );
        }
    }

    #[test]
    fn submitted_query_and_filters_are_an_owned_snapshot() {
        let mut query = String::from(" Submitted query ");
        let mut filters = CatalogFilters {
            date: vec!["year".into()],
            category: vec!["domain:2".into()],
            kind: vec!["academic".into()],
            source: vec!["openalex".into()],
        };
        let submitted = filters.clone();
        let value = json!({"count": 0, "page": 1, "results": []});
        let page = parse_search_page(&value, &query, &filters).unwrap();
        query.clear();
        filters.date.clear();
        filters.category.push("domain:3".into());
        filters.kind[0] = "book".into();
        filters.source[0] = "gutendex".into();
        assert_eq!(page.query, " Submitted query ");
        assert_eq!(page.filters, submitted);
        assert_ne!(page.filters, filters);
    }
}
