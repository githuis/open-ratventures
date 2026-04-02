pub mod background;
pub mod combat;
pub mod dialogue;
pub mod input;
pub mod inventory;
pub mod left_panel;
pub mod missions;
pub mod party;
pub mod quest_lobby;
pub mod tavern;
pub mod welcome;

use ratatui::style::Color;

/// A 5-color palette. Roles in order: text, alert, accent, panel, bg.
/// All palettes sourced from lospec.com/palette-list
pub struct Palette {
    pub name: &'static str,
    pub text: Color,
    pub alert: Color,
    pub accent: Color,
    pub panel: Color,
    pub bg: Color,
}

macro_rules! pal {
    ($name:expr, $t:expr, $al:expr, $ac:expr, $p:expr, $b:expr) => {
        Palette { name: $name, text: Color::Rgb($t.0,$t.1,$t.2), alert: Color::Rgb($al.0,$al.1,$al.2), accent: Color::Rgb($ac.0,$ac.1,$ac.2), panel: Color::Rgb($p.0,$p.1,$p.2), bg: Color::Rgb($b.0,$b.1,$b.2) }
    };
}

pub const PALETTES: &[Palette] = &[
    pal!("Twilight 5",       (251,187,173), (238,134,149), ( 74,122,150), ( 51, 63, 88), ( 41, 40, 49)),
    pal!("Emerald Eden",     (245,251,213), (179,231,110), ( 95,204, 40), ( 48, 26, 71), ( 61,127, 52)),
    pal!("Blessing",         (247,255,174), (150,251,199), (216,191,216), (255,179,203), (116, 86,155)),
    pal!("CAPP-5",           (240,239,244), (250,221,162), (142,205,230), (102,161,255), (107, 97,255)),
    pal!("Nicole Punk 82",   (250,245,216), (242,171, 55), (216,174,139), (205, 95, 42), ( 33, 24, 27)),
    pal!("Ink",              (234,240,216), (150,162,179), ( 89, 96,112), ( 65, 58, 66), ( 31, 31, 41)),
    pal!("Leopold's Dreams", (140,239,182), (109,188,185), ( 72,136,183), ( 71, 68,118), ( 55, 33, 52)),
    pal!("Slimy 05",         (209,203,149), ( 64,152, 94), ( 26,100, 78), (  4, 55, 59), ( 10, 26, 47)),
    pal!("5 Sheep",          (255,218,232), (255,128,174), (255, 50,124), (180, 19, 96), ( 72, 10, 48)),
    pal!("Marumaru Gum",     (253,169,169), (243,237,237), (185,238,220), (150,190,177), (130,147,155)),
    pal!("Poison",           (129,176,113), ( 90,148,112), ( 47,117,113), ( 69, 74, 77), ( 42, 42, 43)),
    pal!("5yan",             (165,245,245), ( 85,165,165), (  5, 85, 85), (  5,  5, 85), (  5,  5,  5)),
    pal!("Strawberry",       (255,205,178), (255,180,162), (229,152,155), (191,133,143), (121,114,127)),
];
