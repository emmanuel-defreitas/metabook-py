//! Project Ely's live theme onto retained gpui-component widgets.

use std::sync::Arc;

use ely_gpui_component::theme::{Mix, Mode, Palette, Radius, TextSize, Theme as ElyTheme};
use gpui::{App, Global, Pixels, SharedString};
use gpui_component::{
    highlighter::{HighlightTheme, SyntaxColors},
    Theme, ThemeColor, ThemeMode, ThemeTokens,
};

#[derive(PartialEq)]
struct Snapshot {
    colors: Palette,
    mode: Mode,
    font_family: SharedString,
    mono_family: SharedString,
    font_size: Pixels,
    mono_font_size: Pixels,
    radius: Pixels,
    radius_lg: Pixels,
}

impl Global for Snapshot {}

/// Call after both libraries initialize, before opening a window.
pub fn init(cx: &mut App) {
    let mode = Mode::from(cx.window_appearance());
    // Ely initializes Platform::current(); leave that platform policy intact.
    ElyTheme::set_mode_now(mode, cx);
    synchronize(cx);
}

/// Call at the start of app rendering, so retained widgets follow Ely's fade.
/// This publishes tokens only: Ely owns animation and repaint scheduling.
pub fn synchronize(cx: &mut App) {
    let ely = cx.global::<ElyTheme>();
    let snapshot = Snapshot {
        // palette() is the fade target, not the colors currently on screen.
        colors: ely.colors.clone(),
        mode: ely.mode(),
        font_family: ely.font_family.clone(),
        mono_family: ely.mono_family.clone(),
        // Root uses this as the window's rem unit, not the body text size.
        // Keep Ely's dimensions at their intended scale; text uses TextSize.
        font_size: ely.base_rem(),
        mono_font_size: ely.text_size(TextSize::Base).to_pixels(ely.base_rem()),
        radius: ely.radius(Radius::Md).to_pixels(ely.base_rem()),
        radius_lg: ely.radius(Radius::Xl).to_pixels(ely.base_rem()),
    };
    if cx.has_global::<Snapshot>() && cx.global::<Snapshot>() == &snapshot {
        return;
    }

    let theme = Theme::global_mut(cx);
    theme.mode = legacy_mode(snapshot.mode);
    theme.colors = map_colors(&snapshot.colors);
    theme.tokens = ThemeTokens::from(&theme.colors);
    theme.font_family = snapshot.font_family.clone();
    theme.mono_font_family = snapshot.mono_family.clone();
    theme.font_size = snapshot.font_size;
    theme.mono_font_size = snapshot.mono_font_size;
    theme.radius = snapshot.radius;
    theme.radius_lg = snapshot.radius_lg;

    map_highlight(
        Arc::make_mut(&mut theme.highlight_theme),
        &snapshot.colors,
        theme.mode,
    );

    // sync_base also installs text defaults; it does not refresh any window.
    Theme::sync_base(cx);
    cx.set_global(snapshot);
}

fn legacy_mode(mode: Mode) -> ThemeMode {
    match mode {
        Mode::Light => ThemeMode::Light,
        Mode::Dark => ThemeMode::Dark,
    }
}

fn map_highlight(highlight: &mut HighlightTheme, p: &Palette, mode: ThemeMode) {
    highlight.appearance = mode;
    highlight.style.editor_background = Some(p.sunken);
    highlight.style.editor_foreground = Some(p.fg);
    highlight.style.editor_active_line = Some(p.hover);
    highlight.style.editor_line_number = Some(p.fg_subtle);
    highlight.style.editor_active_line_number = Some(p.fg);
    highlight.style.editor_invisible = Some(p.fg_disabled);
    highlight.style.editor_gutter_background = Some(p.sunken);
    map_syntax(&mut highlight.style.syntax, p);
}

fn map_syntax(syntax: &mut SyntaxColors, p: &Palette) {
    // ThemeStyle's fields are private. Its serde representation lets us replace
    // only ink, retaining font_style (including underline) and font_weight.
    // GPUI serializes Hsla as 8-bit RGBA hex, so these private syntax colors
    // follow the live fade at that precision; editor surfaces stay unquantized.
    let mut styles = serde_json::to_value(&*syntax).expect("serialize syntax styles");
    let styles_object = styles.as_object_mut().expect("syntax styles are an object");
    let s = &p.syntax;
    for (name, color) in [
        ("attribute", s.attribute),
        ("boolean", s.constant),
        ("comment", s.comment),
        ("comment_doc", s.comment),
        ("constant", s.constant),
        ("constructor", s.type_name),
        ("embedded", s.variable),
        ("emphasis", s.variable),
        ("emphasis.strong", s.variable),
        ("enum", s.type_name),
        ("function", s.function),
        ("hint", s.comment),
        ("keyword", s.keyword),
        ("label", s.property),
        ("link_text", p.link),
        ("link_uri", p.link),
        ("number", s.number),
        ("operator", s.operator),
        ("predictive", s.comment),
        ("preproc", s.keyword),
        ("primary", s.variable),
        ("property", s.property),
        ("punctuation", s.punctuation),
        ("punctuation.bracket", s.punctuation),
        ("punctuation.delimiter", s.punctuation),
        ("punctuation.list_marker", s.punctuation),
        ("punctuation.special", s.punctuation),
        ("string", s.string),
        ("string.escape", s.string),
        ("string.regex", s.string),
        ("string.special", s.string),
        ("string.special.symbol", s.string),
        ("tag", s.tag),
        ("tag.doctype", s.tag),
        ("text.code.span", s.variable),
        ("text.literal", s.string),
        ("title", s.function),
        ("type", s.type_name),
        ("variable", s.variable),
        ("variable.special", s.variable),
        ("variant", s.constant),
    ] {
        let style = styles_object.get_mut(name).expect("supported syntax field");
        if style.is_null() {
            *style = serde_json::json!({});
        }
        style
            .as_object_mut()
            .expect("syntax style is an object")
            .insert(
                "color".into(),
                serde_json::to_value(color).expect("serialize syntax ink"),
            );
    }
    *syntax = serde_json::from_value(styles).expect("deserialize recolored syntax styles");
}

fn map_colors(p: &Palette) -> ThemeColor {
    // Match Ely's Primary, Success and Danger button tone policy. Extend the
    // same signal policy to legacy Info and Warning, which Ely does not expose.
    let primary_active = p.accent_hover.mix(&p.on_accent, 0.12);
    let danger_hover = p.danger.mix(&p.fg, 0.14);
    let danger_active = p.danger.mix(&p.fg, 0.24);
    let info_hover = p.info.mix(&p.fg, 0.14);
    let info_active = p.info.mix(&p.fg, 0.24);
    let success_hover = p.success.mix(&p.fg, 0.14);
    let success_active = p.success.mix(&p.fg, 0.24);
    let warning_hover = p.warning.mix(&p.fg, 0.14);
    let warning_active = p.warning.mix(&p.fg, 0.24);

    ThemeColor {
        // Legacy accent means a neutral hover surface, not a primary action.
        accent: p.hover,
        accent_foreground: p.fg,
        accordion: p.surface,
        background: p.bg,
        border: p.border,
        button: p.surface,
        button_active: p.active,
        button_foreground: p.fg,
        button_hover: p.hover,
        button_danger: p.danger,
        button_danger_active: danger_active,
        button_danger_foreground: p.on_accent,
        button_danger_hover: danger_hover,
        button_info: p.info,
        button_info_active: info_active,
        button_info_foreground: p.on_accent,
        button_info_hover: info_hover,
        button_primary: p.accent,
        button_primary_active: primary_active,
        button_primary_foreground: p.on_accent,
        button_primary_hover: p.accent_hover,
        button_secondary: p.sunken,
        button_secondary_active: p.active,
        button_secondary_foreground: p.fg,
        button_secondary_hover: p.hover,
        button_success: p.success,
        button_success_active: success_active,
        button_success_foreground: p.on_accent,
        button_success_hover: success_hover,
        button_warning: p.warning,
        button_warning_active: warning_active,
        button_warning_foreground: p.on_accent,
        button_warning_hover: warning_hover,
        group_box: p.surface,
        group_box_foreground: p.fg,
        caret: p.fg,
        chart_1: p.chart[0],
        chart_2: p.chart[1],
        chart_3: p.chart[2],
        chart_4: p.chart[3],
        chart_5: p.chart[4],
        chart_bullish: p.success,
        chart_bearish: p.danger,
        danger: p.danger,
        danger_active,
        danger_foreground: p.on_accent,
        danger_hover,
        description_list_label: p.sunken,
        description_list_label_foreground: p.fg_muted,
        drag_border: p.focus,
        drop_target: p.selection,
        foreground: p.fg,
        info: p.info,
        info_active,
        info_foreground: p.on_accent,
        info_hover,
        // This legacy token is an input border, not its fill.
        input: p.border,
        link: p.link,
        link_active: p.link.mix(&p.fg, 0.2),
        link_hover: p.link.mix(&p.fg, 0.1),
        list: p.surface,
        list_active: p.active,
        list_active_border: p.border_strong,
        list_even: p.sunken,
        list_head: p.sunken,
        list_hover: p.hover,
        muted: p.sunken,
        muted_foreground: p.fg_muted,
        // Tooltips and notifications also read this shared legacy surface.
        popover: p.overlay,
        popover_foreground: p.fg,
        primary: p.accent,
        primary_active,
        primary_foreground: p.on_accent,
        primary_hover: p.accent_hover,
        progress_bar: p.accent,
        ring: p.focus,
        scrollbar: p.sunken,
        scrollbar_thumb: p.border_strong,
        scrollbar_thumb_hover: p.fg_subtle,
        secondary: p.sunken,
        secondary_active: p.active,
        secondary_foreground: p.fg,
        secondary_hover: p.hover,
        selection: p.selection,
        sidebar: p.sunken,
        sidebar_accent: p.hover,
        sidebar_accent_foreground: p.fg,
        sidebar_border: p.border,
        sidebar_foreground: p.fg,
        sidebar_primary: p.accent,
        sidebar_primary_foreground: p.on_accent,
        skeleton: p.active,
        slider_bar: p.accent,
        slider_thumb: p.surface,
        success: p.success,
        success_foreground: p.on_accent,
        success_hover,
        success_active,
        switch: p.active,
        switch_thumb: p.surface,
        tab: p.sunken,
        tab_active: p.surface,
        tab_active_foreground: p.fg,
        tab_bar: p.bg,
        tab_bar_segmented: p.sunken,
        tab_foreground: p.fg_muted,
        table: p.surface,
        table_active: p.active,
        table_active_border: p.border_strong,
        table_even: p.sunken,
        table_head: p.sunken,
        table_head_foreground: p.fg_muted,
        table_foot: p.sunken,
        table_foot_foreground: p.fg_muted,
        table_hover: p.hover,
        table_row_border: p.border,
        title_bar: p.bg,
        title_bar_border: p.border,
        status_bar: p.sunken,
        status_bar_border: p.border,
        tiles: p.bg,
        warning: p.warning,
        warning_active,
        warning_hover,
        warning_foreground: p.on_accent,
        overlay: p.backdrop,
        window_border: p.border_strong,
        red: p.danger,
        red_light: p.danger_subtle,
        green: p.success,
        green_light: p.success_subtle,
        blue: p.info,
        blue_light: p.info_subtle,
        yellow: p.warning,
        yellow_light: p.warning_subtle,
        magenta: p.chart[4],
        magenta_light: p.chart[4].mix(&p.surface, 0.85),
        cyan: p.chart[7],
        cyan_light: p.chart[7].mix(&p.surface, 0.85),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_and_dark_keep_hover_separate_from_primary() {
        for p in [Palette::light(false), Palette::dark(false)] {
            let colors = map_colors(&p);
            assert_eq!(colors.accent, p.hover);
            assert_eq!(colors.primary, p.accent);
            assert_eq!(colors.primary_foreground, p.on_accent);
            assert_eq!(colors.button_primary_hover, p.accent_hover);
            assert_eq!(colors.button, p.surface);
            assert_eq!(colors.foreground, p.fg);
            assert_eq!(colors.input, p.border);
            assert_eq!(colors.ring, p.focus);
            assert_eq!(colors.selection, p.selection);
            assert_eq!(colors.sidebar, p.sunken);
            assert_eq!(colors.list_hover, p.hover);
            assert_eq!(colors.popover, p.overlay);
            assert_eq!(colors.danger, p.danger);
            assert_eq!(colors.success, p.success);
            assert_eq!(colors.warning, p.warning);
            assert_eq!(colors.info, p.info);
            let tokens = ThemeTokens::from(&colors);
            assert_eq!(tokens.button_primary.color, p.accent);
            assert_eq!(tokens.popover.color, p.overlay);
            assert_eq!(tokens.scrollbar_thumb.color, p.border_strong);
        }
    }

    #[test]
    fn bridge_uses_interpolated_colors_not_a_mode_default() {
        let p = Palette::light(false).mix(&Palette::dark(false), 0.4);
        let colors = map_colors(&p);
        assert_eq!(colors.background, p.bg);
        assert_eq!(colors.primary, p.accent);
        assert_eq!(colors.button_hover, p.hover);
        assert_eq!(colors.scrollbar_thumb, p.border_strong);
        assert_eq!(colors.danger, p.danger);
    }

    #[test]
    fn json_syntax_follows_dark_to_light_fade_and_preserves_metadata() {
        let dark = Palette::dark(false);
        let light = Palette::light(false);
        let mut highlight = (*HighlightTheme::default_dark()).clone();
        let original_name = highlight.name.clone();
        let mut styles = serde_json::to_value(&highlight.style.syntax).unwrap();
        styles["string"] = serde_json::json!({
            "color": dark.syntax.string,
            "font_style": "underline",
            "font_weight": 700,
        });
        styles["comment"] = serde_json::json!({
            "font_style": "italic",
            "font_weight": 400,
        });
        // A previously absent style must get ink too, not retain a fallback.
        styles["property"] = serde_json::Value::Null;
        highlight.style.syntax = serde_json::from_value(styles).unwrap();

        for p in [&dark, &dark.mix(&light, 0.5), &light] {
            // The target mode is already light while the palette still fades.
            map_highlight(&mut highlight, p, ThemeMode::Light);
            for (capture, expected) in [
                ("property", p.syntax.property),
                ("string", p.syntax.string),
                ("string.escape", p.syntax.string),
                ("number", p.syntax.number),
                ("boolean", p.syntax.constant),
                ("constant.builtin", p.syntax.constant),
                ("punctuation", p.syntax.punctuation),
                ("comment", p.syntax.comment),
            ] {
                let serialized_ink =
                    serde_json::from_value(serde_json::to_value(expected).unwrap()).unwrap();
                assert_eq!(
                    highlight.style.syntax.style(capture).unwrap().color,
                    Some(serialized_ink)
                );
            }
            assert_eq!(highlight.appearance, ThemeMode::Light);
            assert_eq!(highlight.style.editor_background, Some(p.sunken));
            assert_eq!(highlight.style.editor_foreground, Some(p.fg));
            assert_eq!(highlight.name, original_name);
            let styles = serde_json::to_value(&highlight.style.syntax).unwrap();
            assert_eq!(styles["string"]["font_style"], "underline");
            assert_eq!(styles["string"]["font_weight"], 700);
            assert_eq!(styles["comment"]["font_style"], "italic");
            assert_eq!(styles["comment"]["font_weight"], 400);
        }
        assert_ne!(dark.syntax.string, light.syntax.string);
        assert_ne!(dark.syntax.property, light.syntax.property);
    }

    #[test]
    fn signal_buttons_match_ely_tones_even_with_custom_background() {
        for mut p in [Palette::light(false), Palette::dark(false)] {
            p.bg = p.hover;
            let colors = map_colors(&p);
            assert_ne!(p.bg, p.on_accent);
            for foreground in [
                colors.button_danger_foreground,
                colors.button_success_foreground,
                colors.button_info_foreground,
                colors.button_warning_foreground,
                colors.danger_foreground,
                colors.success_foreground,
                colors.info_foreground,
                colors.warning_foreground,
            ] {
                assert_eq!(foreground, p.on_accent);
            }
            assert_eq!(colors.button_danger_hover, p.danger.mix(&p.fg, 0.14));
            assert_eq!(colors.button_danger_active, p.danger.mix(&p.fg, 0.24));
            assert_eq!(colors.button_success_hover, p.success.mix(&p.fg, 0.14));
            assert_eq!(colors.button_success_active, p.success.mix(&p.fg, 0.24));
            assert_eq!(
                colors.button_primary_active,
                p.accent_hover.mix(&p.on_accent, 0.12)
            );
        }
    }

    #[test]
    fn modes_match() {
        assert_eq!(legacy_mode(Mode::Light), ThemeMode::Light);
        assert_eq!(legacy_mode(Mode::Dark), ThemeMode::Dark);
    }
}
