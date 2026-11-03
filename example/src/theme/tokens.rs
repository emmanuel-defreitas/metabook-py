//! Semantic colors from the checked-in DesignSystems.one token export.
use ely_gpui_component::theme::{Mode, Palette};
use gpui::{rgb, Hsla};

fn color(value: &str) -> Hsla {
    rgb(u32::from_str_radix(value.trim_start_matches('#'), 16).expect("valid exported hex color"))
        .into()
}

pub(super) fn palette(mode: Mode) -> Palette {
    let export: serde_json::Value =
        serde_json::from_str(include_str!("../../assets/design-tokens.json"))
            .expect("valid token export");
    let mode_name = match mode {
        Mode::Light => "light",
        Mode::Dark => "dark",
    };
    let tokens = &export["tokens"];
    let colors = &tokens["themes"][mode_name];
    let mut palette = match mode {
        Mode::Light => Palette::light(false),
        Mode::Dark => Palette::dark(false),
    };
    // Ely names its primary action `accent`; map the export’s blue brand here.
    for (ely, exported) in [
        ("bg", "bg"),
        ("surface", "surface"),
        ("sunken", "bgSubtle"),
        ("overlay", "surfaceRaised"),
        ("hover", "bgSubtle"),
        ("active", "bgMuted"),
        ("border", "border"),
        ("border_strong", "borderStrong"),
        ("fg", "fg"),
        ("fg_muted", "fgMuted"),
        ("fg_subtle", "fgSubtle"),
        ("fg_disabled", "fgSubtle"),
        ("accent", "brand"),
        ("accent_hover", "brandHover"),
        ("on_accent", "brandFg"),
        ("focus", "ring"),
        ("link", "brand"),
        ("selection", "brandSubtle"),
        ("success", "success"),
        ("warning", "warning"),
        ("danger", "danger"),
        ("info", "info"),
        ("success_subtle", "successSubtle"),
        ("warning_subtle", "warningSubtle"),
        ("danger_subtle", "dangerSubtle"),
        ("info_subtle", "infoSubtle"),
        ("shimmer", "bgMuted"),
        ("glass", "surfaceRaised"),
        ("tooltip_bg", "fg"),
        ("tooltip_fg", "surface"),
        ("paper", "surface"),
        ("ink", "fg"),
    ] {
        *palette.token_mut(ely) =
            color(colors[exported]["value"].as_str().expect("semantic color"));
    }
    let chart = tokens["chart"][mode_name].as_array().expect("chart colors");
    for (index, value) in chart.iter().enumerate() {
        palette.chart[index] = color(value.as_str().expect("chart hex"));
    }
    // Ely has eight chart slots; complete the six-color export with its green accent and info.
    palette.chart[6] = color(colors["accent"]["value"].as_str().unwrap());
    palette.chart[7] = palette.info;
    palette.syntax.string = palette.chart[6];
    palette.syntax.keyword = palette.accent;
    palette.syntax.comment = palette.fg_subtle;
    palette.syntax.variable = palette.fg;
    palette.syntax.property = palette.link;
    palette.syntax.number = palette.warning;
    palette.syntax.constant = palette.info;
    palette.syntax.punctuation = palette.fg_muted;
    palette.syntax.function = palette.chart[2];
    palette.syntax.type_name = palette.chart[1];
    palette.syntax.tag = palette.accent;
    palette.syntax.attribute = palette.chart[3];
    palette.syntax.operator = palette.fg_muted;
    palette
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exported_colors_reach_both_ely_modes() {
        let light = palette(Mode::Light);
        let dark = palette(Mode::Dark);
        assert_eq!(light.bg, color("#f9fafd"));
        assert_eq!(dark.bg, color("#0f1217"));
        assert_eq!(light.accent, color("#0061ff"));
        assert_eq!(dark.accent, color("#b0cdff"));
        assert_eq!(dark.chart[6], color("#00f900"));
        assert_eq!(light.border, color("#dde1ea"));
        assert_eq!(dark.fg, color("#f9fafd"));
    }
}
