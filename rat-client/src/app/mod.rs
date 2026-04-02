use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Clear, Widget},
};
use ratback_types::{
    data::{CharacterWrapper, InventoryItem, ItemEffect, User},
    quest_data::{Dialogue, DialogueOutcome, Encounter, MissionState, MissionStatus, Party, PartySummary, Quest},
    AREA_SEWERS, AREA_SEWER_DEPTHS, AREA_FUNGAL_WARRENS, AREA_ABYSS,
    RENOWN_SEWER_DEPTHS, RENOWN_FUNGAL_WARRENS, RENOWN_ABYSS,
};

use crate::client::Rattp;
use crate::tui;

mod input;
mod input_wasm;
mod quest;
mod combat;
mod dialogue;
mod party;
mod character;

#[derive(Debug, Default)]
pub struct App {
    pub exit: bool,
    pub state: AppState,
    pub active_user: Option<User>,
    pub active_character: Option<CharacterWrapper>,
    pub active_quest: Option<Quest>,
    pub active_party: Option<Party>,
    pub party_members: Vec<CharacterWrapper>,
    pub text_input: Option<String>,
    pub client: Rattp,
    pub last_combat_damage: Option<(i32, String)>,
    pub inventory: Vec<InventoryItem>,
    pub backend_version: Option<String>,
    pub missions: Vec<MissionStatus>,
    pub clue_notification: Option<String>,
    pub is_processing: bool,
    pub spinner_tick: u8,
    pub palette_index: usize,
}

#[derive(Debug, Default)]
pub enum TavernState {
    #[default]
    Main,
    Shop { items: Vec<ratback_types::data::ShopItem>, selected: usize, scroll: usize },
    Options,
    PaletteSelect { selected: usize },
}

#[derive(Debug)]
pub enum EncounterState {
    Combat { cleared: bool },
    Dialogue { dialogue: Dialogue, current_node: String, cleared: bool },
}

#[derive(Debug)]
pub enum AppState {
    Welcome,
    Tavern(TavernState),
    Main,
    TextInput(Reason),
    FinishInput(Reason),
    Party,
    Encounter(EncounterState),
    PartyLobby { parties: Vec<PartySummary> },
    AdventureMenu,
    MissionSelect { missions: Vec<MissionStatus>, selected: usize },
    Inventory { scroll: usize, selected: usize, previous: Box<AppState> },
    TargetSelect { item_index: usize, target_selected: usize, inv_scroll: usize, inv_item_selected: usize, return_to: Box<AppState> },
    GameOver,
    Victory,
}

impl Default for AppState {
    fn default() -> Self {
        AppState::Welcome
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub enum Reason {
    #[default]
    Register,
    CreateCharacter,
    Rename,
}

impl App {
    pub fn c_text(&self)   -> ratatui::style::Color { crate::ui::PALETTES[self.palette_index].text }
    pub fn c_alert(&self)  -> ratatui::style::Color { crate::ui::PALETTES[self.palette_index].alert }
    pub fn c_accent(&self) -> ratatui::style::Color { crate::ui::PALETTES[self.palette_index].accent }
    pub fn c_panel(&self)  -> ratatui::style::Color { crate::ui::PALETTES[self.palette_index].panel }
    pub fn c_bg(&self)     -> ratatui::style::Color { crate::ui::PALETTES[self.palette_index].bg }

    pub fn render_frame(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let text_style = Style::default()
            .fg(self.c_text())
            .add_modifier(Modifier::BOLD);

        if matches!(self.state, AppState::Welcome) {
            self.render_welcome(area, buf, text_style);
            return;
        }

        let parent_layout = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        let left_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4), Constraint::Min(6)])
            .split(parent_layout[0]);

        self.render_main(area, buf, text_style);
        if self.active_character.is_some() {
            self.render_left_panel(left_layout[0], buf);
            self.render_party(left_layout[1], buf, text_style);
        }

        match &self.state {
            AppState::Tavern(sub) => {
                self.render_tavern(parent_layout[1], buf, text_style, sub);
            }
            AppState::TextInput(Reason::Rename) => {
                self.render_tavern(parent_layout[1], buf, text_style, &TavernState::Options);
            }
            AppState::TextInput(_) => {
                let popup_width = 100.min(area.width.saturating_sub(4));
                let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
                let popup_y = area.y + area.height / 2;
                let rect = Rect::new(popup_x, popup_y, popup_width, 3);
                Clear::default().render(rect, buf);
                self.render_input(rect, buf, text_style);
            }
            AppState::Encounter(enc_state) => {
                let cleared = match enc_state {
                    EncounterState::Combat { cleared } => {
                        self.render_combat(parent_layout[1], buf, text_style);
                        *cleared
                    }
                    EncounterState::Dialogue { dialogue, current_node, cleared } => {
                        self.render_dialogue(parent_layout[1], buf, text_style, dialogue, current_node);
                        *cleared
                    }
                };
                if cleared {
                    let base = parent_layout[1];
                    let popup_width = 40.min(base.width.saturating_sub(4));
                    let popup_height = 6.min(base.height.saturating_sub(4));
                    let popup_x = base.x + (base.width.saturating_sub(popup_width)) / 2;
                    let popup_y = base.y + (base.height.saturating_sub(popup_height)) / 2;
                    let rect = Rect::new(popup_x, popup_y, popup_width, popup_height);
                    let from_combat = matches!(enc_state, EncounterState::Combat { .. });
                    self.render_encounter_cleared(rect, buf, text_style, from_combat);
                }
            }
            AppState::AdventureMenu => {
                self.render_adventure_menu(parent_layout[1], buf, text_style);
            }
            AppState::PartyLobby { parties } => {
                self.render_party_lobby(parent_layout[1], buf, text_style, parties);
            }
            AppState::Party => {
                self.render_party_screen(parent_layout[1], buf, text_style);
            }
            AppState::Inventory { scroll, selected, previous } => {
                let popup_width = 60.min(area.width.saturating_sub(4));
                let popup_height = 20.min(area.height.saturating_sub(4));
                let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
                let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
                let rect = Rect::new(popup_x, popup_y, popup_width, popup_height);
                Clear::default().render(rect, buf);
                let in_combat = matches!(**previous, AppState::Encounter(EncounterState::Combat { .. }));
                self.render_inventory_popup(rect, buf, text_style, *scroll, *selected, in_combat);
            }

            AppState::GameOver => {
                self.render_combat(parent_layout[1], buf, text_style);
                let popup_width = 40.min(area.width.saturating_sub(4));
                let popup_height = 8.min(area.height.saturating_sub(4));
                let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
                let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
                let rect = Rect::new(popup_x, popup_y, popup_width, popup_height);
                Clear::default().render(rect, buf);
                self.render_game_over(rect, buf, text_style);
            }
            AppState::TargetSelect { target_selected, .. } => {
                let popup_width = 40.min(area.width.saturating_sub(4));
                let popup_height = 10.min(area.height.saturating_sub(4));
                let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
                let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
                let rect = Rect::new(popup_x, popup_y, popup_width, popup_height);
                Clear::default().render(rect, buf);
                self.render_target_select(rect, buf, text_style, *target_selected);
            }
            AppState::MissionSelect { missions, selected } => {
                self.render_mission_select(parent_layout[1], buf, text_style, missions, *selected);
            }
            AppState::Victory => {
                self.render_victory(parent_layout[1], buf, text_style);
            }
            _ => {}
        }

        if self.is_processing {
            use ratatui::widgets::StatefulWidget;
            use throbber_widgets_tui::{Throbber, ThrobberState, BRAILLE_SIX};
            let throbber_area = Rect::new(
                area.right().saturating_sub(2),
                area.bottom().saturating_sub(1),
                1, 1,
            );
            let throbber = Throbber::default()
                .throbber_set(BRAILLE_SIX)
                .style(Style::default().fg(ratatui::style::Color::DarkGray));
            let mut state = ThrobberState::default();
            state.calc_step(self.spinner_tick as i8);
            StatefulWidget::render(throbber, throbber_area, buf, &mut state);
        }
    }
}
