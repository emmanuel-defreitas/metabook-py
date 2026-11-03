//! Heuristic schema support from saved records or analysis responses.

use ely_gpui_component::{
    primitives::Tooltip,
    theme::{ActiveTheme as _, TextSize},
    typography::{format, PercentText},
};
use gpui::{
    div, AnyElement, App, InteractiveElement as _, IntoElement, ParentElement, Role,
    StatefulInteractiveElement as _, Styled,
};
use serde_json::Value;

const SCORE_DESCRIPTION: &str =
    "Heuristic rule support for automatic schema selection, not a calibrated probability. \
     Score unavailable means no valid numerical score was provided.";

fn valid_ratio(value: &f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(value)
}

/// Prefer saved scan support, falling back to an analysis response's structure.
/// Missing, non-numerical, and invalid ratios remain unknown, not zero.
pub(in crate::app) fn schema_score(record: &Value) -> Option<f64> {
    record["scan"]["schema_score"]
        .as_f64()
        .filter(valid_ratio)
        .or_else(|| {
            record["structure"]["schema_score"]
                .as_f64()
                .filter(valid_ratio)
        })
}

/// Render a ratio as schema support, with an explicit unknown state.
/// Repeated rows must sit inside uniquely identified parents to scope tooltip IDs.
pub(in crate::app) fn render_score(score: Option<f64>, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let score = score.filter(valid_ratio);
    let label = match score {
        Some(value) => format!("Schema support: {}", format::percent(value, 0, false)),
        None => "Schema support: Score unavailable".into(),
    };
    let value = match score {
        Some(value) => PercentText::new(value).decimals(0).into_any_element(),
        None => div().child("Score unavailable").into_any_element(),
    };
    div()
        .id("schema-support")
        .role(Role::Group)
        .aria_label(label)
        .aria_description(SCORE_DESCRIPTION)
        .tooltip(Tooltip::text(SCORE_DESCRIPTION))
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .text_size(theme.text_size(TextSize::Sm))
        .text_color(theme.colors.fg_muted)
        .child("Schema support")
        .child(value)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{schema_score, valid_ratio};
    use serde_json::json;

    #[test]
    fn extracts_saved_scan_ratio() {
        assert_eq!(
            schema_score(&json!({"scan": {"schema_score": 0.76}})),
            Some(0.76)
        );
    }

    #[test]
    fn saved_scan_takes_precedence_over_analysis_structure() {
        assert_eq!(
            schema_score(&json!({
                "scan": {"schema_score": 0.42},
                "structure": {"schema_score": 0.9}
            })),
            Some(0.42)
        );
    }

    #[test]
    fn zero_is_a_known_scan_score_not_a_missing_value() {
        assert_eq!(
            schema_score(&json!({
                "scan": {"schema_score": 0.0},
                "structure": {"schema_score": 0.9}
            })),
            Some(0.0)
        );
    }

    #[test]
    fn extracts_analysis_structure_ratio_including_boundaries() {
        for score in [0.0, 0.63, 1.0] {
            assert_eq!(
                schema_score(&json!({"structure": {"schema_score": score}})),
                Some(score)
            );
        }
    }

    #[test]
    fn missing_scores_remain_unknown() {
        for record in [
            json!({}),
            json!(null),
            json!([]),
            json!({"scan": {"scanned": false}}),
            json!({"scan": {"scanned": true}, "structure": {}}),
            json!({"scan": null, "structure": null}),
        ] {
            assert_eq!(schema_score(&record), None, "record: {record}");
        }
    }

    #[test]
    fn non_numerical_scores_remain_unknown() {
        for value in [json!(null), json!("0.5"), json!(true), json!({}), json!([])] {
            assert_eq!(
                schema_score(&json!({
                    "scan": {"schema_score": value},
                    "structure": {"schema_score": value}
                })),
                None,
                "value: {value}"
            );
        }
    }

    #[test]
    fn out_of_range_scores_are_not_clamped_or_scaled() {
        for score in [-0.01, 1.01, 76.0] {
            assert_eq!(
                schema_score(&json!({
                    "scan": {"schema_score": score},
                    "structure": {"schema_score": score}
                })),
                None,
                "score: {score}"
            );
        }
    }

    #[test]
    fn invalid_saved_scan_falls_back_to_valid_analysis_structure() {
        for value in [json!(null), json!("0.5"), json!(-0.1), json!(1.1)] {
            assert_eq!(
                schema_score(&json!({
                    "scan": {"schema_score": value},
                    "structure": {"schema_score": 0.63}
                })),
                Some(0.63)
            );
        }
    }

    #[test]
    fn non_finite_ratios_are_rejected_by_the_shared_validation() {
        // JSON cannot hold non-finite numbers, but a caller can pass one to render_score.
        for score in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(!valid_ratio(&score));
        }
    }
}
