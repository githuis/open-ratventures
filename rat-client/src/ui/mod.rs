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

pub const C_TEXT: Color    = Color::Rgb(251, 187, 173); // #fbbbad
pub const C_ALERT: Color   = Color::Rgb(238, 134, 149); // #ee8695
pub const C_ACCENT: Color  = Color::Rgb(74, 122, 150);  // #4a7a96
pub const C_PANEL: Color   = Color::Rgb(51, 63, 88);    // #333f58
pub const C_BG: Color      = Color::Rgb(41, 40, 49);    // #292831
