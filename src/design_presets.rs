//! Curated, opinionated design-principle presets.
//!
//! Inventing a coherent visual system from scratch is nerve's weakest spot —
//! left to improvise it tends to produce generic, inconsistent CSS. These
//! presets skip that guesswork: each is a complete, specific, internally
//! consistent design brief (concrete tokens, a strict spacing scale, a type
//! scale, component rules, do's/don'ts and a pre-finish checklist) that drops
//! into `.nerve/design.md` with one command and is then injected on every
//! UI/design turn (see [`crate::memory_recall`]) and enforced by the linter
//! (see [`crate::design`]).
//!
//! Applied via `/design preset <name>` (see [`crate::commands::project`]).

/// One selectable preset: its lookup name, a one-line description for listings,
/// and the full design-principles document it installs.
struct Preset {
    name: &'static str,
    description: &'static str,
    doc: &'static str,
}

/// All presets, in listing order.
const PRESETS: &[Preset] = &[
    Preset {
        name: "basecamp",
        description: "Calm 37signals editorial — warm paper, hairlines over cards, one quiet blue.",
        doc: BASECAMP,
    },
    Preset {
        name: "apple",
        description: "Clean white space, system font, one Apple blue, precise restraint.",
        doc: APPLE,
    },
    Preset {
        name: "linear",
        description: "Refined dark technical — near-black surfaces, crisp 1px borders, one violet.",
        doc: LINEAR,
    },
];

/// The design-principles document for `name` (case-insensitive), if it exists.
pub fn preset(name: &str) -> Option<&'static str> {
    let name = name.trim();
    PRESETS
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
        .map(|p| p.doc)
}

/// The names of every available preset, in listing order.
pub fn preset_names() -> &'static [&'static str] {
    // Built once; the set is fixed at compile time.
    use std::sync::OnceLock;
    static NAMES: OnceLock<Vec<&'static str>> = OnceLock::new();
    NAMES.get_or_init(|| PRESETS.iter().map(|p| p.name).collect())
}

/// A one-line description for `name` (case-insensitive), for listings.
pub fn preset_description(name: &str) -> Option<&'static str> {
    let name = name.trim();
    PRESETS
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
        .map(|p| p.description)
}

// ── Preset documents ───────────────────────────────────────────────────────

const BASECAMP: &str = r#"# Design principles — Basecamp editorial

A calm, confident, text-first aesthetic in the spirit of 37signals: warm paper,
generous whitespace, quiet typography and hairline rules instead of boxes. The
content is the interface. Nothing decorative, nothing that moves for its own
sake.

## Color tokens (use these exact values; do not invent new ones)
- Background (paper): `#f6f5f1`
- Surface (cards/panels): `#ffffff`
- Ink (primary text): `#1a1a1a`
- Muted (secondary text): `#6c6a64`
- Hairline (borders/rules): `#e4e1d9`
- Accent (the ONE accent): `#1f6feb` — hover `#1a5fd0`
- Accent on paper for links only; never large fills of it.

One accent, full stop. No second brand color. Semantic states may add a red
`#b42318` (destructive) and green `#1a7f4b` (success) as text/icons only.

## Typography
- Family: `Inter, -apple-system, "Segoe UI", Roboto, system-ui, sans-serif`.
- Type scale (px / line-height): 13/18 caption · 15/24 body · 17/26 lead ·
  20/28 h3 · 26/34 h2 · 34/40 h1. Body text is 17px in prose, 15px in UI.
- Weights: 400 body, 500 UI labels, 600 headings. Never use 700+; quiet wins.
- Color: headings and body use Ink; supporting text uses Muted. Never gray-on-gray.
- Letter-spacing: default (0). No uppercase tracking except tiny 12px eyebrow
  labels (Muted, letter-spacing 0.04em).

## Spacing scale (STRICT — every margin/padding/gap is one of these px)
`4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 96`. Never 10, 13, 15, 18, 30. Vertical
rhythm between sections is 48 or 64; within a card 16 or 24.

## Layout
- Content max-width ~`1040px`; prose/reading column max-width ~`680px`.
- Left-align everything. Centered body text is banned; centered short headings ok.
- Separate regions with a 1px hairline (`#e4e1d9`), not with a shadow or a filled box.

## Components
- Buttons: height `42px`, radius `6px`, padding `0 16px`, weight 500. Primary =
  solid accent, white text. Secondary = surface with a 1px hairline, Ink text.
  No gradients, no drop shadow on buttons.
- Inputs: height `46px`, radius `6px`, 1px hairline border, `#ffffff` background,
  16px horizontal padding. Focus = accent border (no glow beyond a 1px ring).
- Cards: white surface, 1px hairline, radius `6px`, padding `24px`. At most a
  single very soft shadow `0 1px 2px rgba(20,20,20,0.04)` — usually none.
- Tables/lists: rows divided by hairlines, ample `12–16px` vertical padding.

## Do
- Trust whitespace; let sections breathe with 48–64px gaps.
- Use hairline rules and type weight/size for hierarchy.
- Keep one accent and use it sparingly (links, the primary action).

## Don't
- No gradients. No animations or transitions beyond a 120ms color/opacity fade.
- No emoji in UI copy. No drop shadows as decoration. No pure black `#000`.
- No cards-inside-cards, no heavy borders, no more than one accent hue.

## Pre-finish checklist
1. Every spacing value is on the 4px scale above?
2. Only the documented tokens appear — one accent, no stray hex?
3. Body is 15–17px Ink on paper/surface; secondary text is Muted?
4. Regions separated by hairlines, not boxes or shadows?
5. Reading columns ≤ 680px, page ≤ 1040px, left-aligned?
6. Zero gradients, zero decorative animation, zero emoji?
"#;

const APPLE: &str = r#"# Design principles — Apple precision

Clean, white, deliberate. Enormous negative space, confident typography, one
accent, and restraint everywhere. Depth is implied with the faintest of shadows,
never with gradients or ornament. Precision over decoration.

## Color tokens (use these exact values)
- Background: `#ffffff`
- Near-white surface / grouped background: `#f5f5f7`
- Ink (primary text): `#1d1d1f`
- Muted (secondary text): `#6e6e73`
- Hairline (borders/dividers): `#d2d2d7`
- Accent (the ONE accent): Apple blue `#0071e3` — hover `#0077ed`
- Semantic (text/icons only): red `#e30000`, green `#1d8a34`.

One accent. Never introduce a second brand hue or tint large areas with color.

## Typography
- Family (system stack): `-apple-system, BlinkMacSystemFont, "SF Pro Text",
  "Helvetica Neue", Helvetica, Arial, sans-serif`.
- Type scale (px / line-height / tracking):
  12/16/0 caption · 17/25/0 body · 21/29/-0.01em lead ·
  28/34/-0.02em h3 · 40/44/-0.02em h2 · 56/60/-0.03em h1 (hero).
- Headlines are large and confident with tight negative letter-spacing; body is
  17px, regular weight, comfortable line-height.
- Weights: 400 body, 500 controls, 600 headlines. Avoid 800/900.
- Color: Ink for headings/body, Muted for supporting text.

## Spacing scale (STRICT — 4px grid)
`4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 96`. Section rhythm is generous: 64 or 96
between major blocks. Nothing off-grid (no 10, 15, 18, 30).

## Layout
- Content max-width ~`980–1024px`, centered in the viewport with wide gutters.
- Hero and feature sections are often center-aligned; dense UI stays left-aligned.
- Space is the primary design tool — when in doubt, add more of it.

## Components
- Buttons: solid accent blue, white text, radius `980px` (full pill) for CTAs or
  `12px` for inline actions, height `44px`, padding `0 20px`, weight 500. Hover =
  `#0077ed`, no shadow, no gradient. Secondary = accent-colored text link.
- Inputs: height `44px`, radius `12px`, 1px `#d2d2d7` border, white background,
  16px padding. Focus = 1px accent ring + very soft accent halo (max blur 4px).
- Cards: `#ffffff` or `#f5f5f7`, radius `12–14px`, padding `24–32px`. Depth is at
  most `0 1px 2px rgba(0,0,0,0.06)` — usually none; prefer the grouped-background
  surface to separate content instead of a shadow.
- Corners: 10–14px on cards/media, pill for primary CTAs. Consistent, never mixed
  randomly.

## Do
- Lead with one big, tight-tracked headline and lots of air around it.
- Keep a single accent; reserve it for the primary action and key links.
- Use `#f5f5f7` blocks to group content rather than drawing borders.

## Don't
- No decorative gradients (a functional near-white → white is the only exception).
- No heavy shadows, no glows beyond a subtle focus halo, no emoji as iconography.
- No competing accent colors, no busy textures, no more than 2 radii in a view.

## Pre-finish checklist
1. Exactly one accent (`#0071e3`), used only for actions/links?
2. Every spacing value on the 4px scale; section gaps 64–96?
3. Headlines large with negative tracking; body 17px, Muted for secondary?
4. Shadows ≤ `0 1px 2px rgba(0,0,0,0.06)`; depth mostly from whitespace/surface?
5. Radii limited to 10–14px (pill only for primary CTAs)?
6. Zero decorative gradients, zero emoji, generous negative space everywhere?
"#;

const LINEAR: &str = r#"# Design principles — Linear (dark technical)

A refined, dense-but-calm dark aesthetic for technical products: near-black
surfaces, crisp 1px borders, a single vivid accent, and tight modern type.
Everything is precise and quiet — subtle depth, no noise.

## Color tokens (use these exact values)
- Background (base): `#0b0d10`
- Surface (panels/cards): `#111318`
- Surface raised (menus/popovers): `#171a21`
- Border (crisp 1px lines): `#23262d`
- Text primary: `#e6e8ec`
- Text muted (secondary): `#9198a1`
- Text faint (tertiary/disabled): `#6b7280`
- Accent (the ONE accent): indigo/violet `#5b5bd6` — hover `#6e6ae0`
- Semantic (text/icons/1px accents only): red `#f2555a`, green `#3fb37f`,
  amber `#e0a144`.

One accent. Color is scarce and intentional — the accent marks focus, the
active item and the primary action, nothing else.

## Typography
- Family: `"Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui,
  sans-serif`. Monospace: `"JetBrains Mono", ui-monospace, SFMono-Regular,
  Menlo, monospace`.
- Type scale (px / line-height): 12/16 caption · 13/20 small · 14/22 body ·
  16/24 lead · 20/28 h3 · 24/32 h2 · 32/40 h1. UI defaults to 13–14px — dense.
- Weights: 400 body, 500 UI labels/links, 600 headings. Never below 400.
- Letter-spacing: headings `-0.01em` to `-0.02em` (tight, modern). Tiny 12px
  section labels may use `+0.04em` uppercase in Text muted.
- Color: primary text for content, muted for secondary, faint for hints only.

## Spacing scale (STRICT — 4px grid)
`4, 8, 12, 16, 20, 24, 32, 40, 48, 64`. Dense UI leans on 8/12/16; no off-grid
values (no 10, 15, 18, 30). Keep it tight but never cramped.

## Layout
- App shell max content width ~`1120px`; reading/prose column ~`680px`.
- Separate regions with a 1px `#23262d` border, not with shadow. Elevation is a
  one-step lighter surface (`#171a21`), plus at most `0 1px 2px rgba(0,0,0,0.4)`.
- Left-aligned, grid-disciplined, information-dense but with calm breathing room.

## Components
- Buttons: height `32px` (compact) or `36px` (default), radius `6px`, padding
  `0 12px`, weight 500. Primary = solid accent `#5b5bd6`, `#ffffff` text, hover
  `#6e6ae0`. Secondary = surface with 1px `#23262d` border, primary text.
  Ghost = transparent, muted text, border/bg appears on hover. No gradients.
- Inputs: height `34px`, radius `6px`, 1px `#23262d` border, `#0b0d10` background,
  12px padding. Focus = accent border + 1px accent ring (no large glow).
- Cards/panels: `#111318` surface, 1px `#23262d` border, radius `8px`, padding
  `16–20px`. Raised surfaces step to `#171a21`.
- Menus/popovers: `#171a21`, 1px border, radius `8px`, soft `0 4px 12px
  rgba(0,0,0,0.5)` shadow — the only place shadow is meaningful.
- Focus states are always visible and use the accent; keyboard-first.

## Do
- Build hierarchy from 1px borders and one-step surface elevation, not shadows.
- Keep the accent rare — active nav item, focus ring, primary button.
- Stay dense but calm: tight type, disciplined grid, consistent 6–8px radii.

## Don't
- No gradients (a barely-there surface tint is the only exception).
- No pure-white `#fff` text (use `#e6e8ec`) and no glows/neon.
- No emoji as UI iconography, no more than one accent, no big drop shadows on cards.

## Pre-finish checklist
1. Backgrounds/surfaces/borders use only the documented near-black tokens?
2. Exactly one accent (`#5b5bd6`), reserved for focus/active/primary?
3. Every spacing value on the 4px scale; UI density from 8/12/16?
4. Elevation via a lighter surface + 1px border — shadow only on popovers?
5. Text is `#e6e8ec`/`#9198a1`/`#6b7280`, never pure white; headings tight-tracked?
6. Radii consistent at 6–8px; zero gradients, zero glow, zero emoji?
"#;

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_resolves_each_name_case_insensitively() {
        for name in ["basecamp", "apple", "linear"] {
            assert!(preset(name).is_some(), "{name} should resolve");
            assert!(preset(&name.to_uppercase()).is_some(), "{name} upper");
            // Surrounding whitespace is tolerated.
            assert!(preset(&format!("  {name}  ")).is_some(), "{name} padded");
        }
    }

    #[test]
    fn preset_returns_none_for_unknown() {
        assert!(preset("nope").is_none());
        assert!(preset("").is_none());
    }

    #[test]
    fn there_are_exactly_three_presets() {
        assert_eq!(preset_names().len(), 3);
        assert_eq!(preset_names(), &["basecamp", "apple", "linear"]);
    }

    #[test]
    fn every_named_preset_has_a_description() {
        for name in preset_names() {
            assert!(
                preset_description(name).is_some_and(|d| !d.is_empty()),
                "{name} needs a description"
            );
        }
    }

    #[test]
    fn every_doc_is_well_formed_and_specific() {
        for name in preset_names() {
            let doc = preset(name).unwrap();
            // Structured markdown with the expected sections.
            assert!(
                doc.starts_with("# Design principles"),
                "{name} needs a heading"
            );
            assert!(doc.contains("## Spacing scale"), "{name} spacing section");
            assert!(
                doc.contains("## Pre-finish checklist"),
                "{name} checklist section"
            );
            // Concrete: mentions the 4px scale and at least one hex token.
            assert!(doc.contains('#'), "{name} should carry hex tokens");
            // Focused, not a giant blob.
            let lines = doc.lines().count();
            assert!((30..=90).contains(&lines), "{name} has {lines} lines");
        }
    }
}
