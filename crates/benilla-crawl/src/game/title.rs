//! The title screen: continue the campaign, or begin a new one.

use bevy::prelude::*;

use super::ui::{self, Kit, BONE, DIM, GOLD};
use super::{load_save, save, Game, Scoped, Screen};
use crate::rules::town::Campaign;

#[derive(Component, Clone, Copy)]
enum TitleAct {
    Continue,
    New,
    Quit,
}

pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Title), build)
            .add_systems(Update, act.run_if(in_state(Screen::Title)));
    }
}

fn build(mut commands: Commands, mut kit: Kit, mut narrator: ResMut<ui::Narrator>) {
    narrator.clear();
    let bg = kit.image("Interface\\Glues\\LoadingScreens\\LoadScreenScholomance");
    let has_save = super::save_path().exists();
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
            ImageNode { image: bg, color: Color::srgb(0.35, 0.3, 0.3), ..default() },
            Scoped(Screen::Title),
        ))
        .with_children(|p| {
            p.spawn((
                kit.title("DARKENED DEEPS", 96.0, GOLD),
                TextShadow { offset: Vec2::splat(4.0), color: Color::BLACK },
            ));
            p.spawn((
                kit.title("Beneath the hamlet, the old crypts wait.", 26.0, BONE),
                TextShadow { offset: Vec2::splat(2.0), color: Color::BLACK },
            ));
            p.spawn(Node { height: Val::Px(40.0), ..default() });
            if has_save {
                ui::button_sized(p, &kit, "Continue the Campaign", TitleAct::Continue, true, false, 24.0);
            }
            ui::button_sized(p, &kit, "Begin a New Campaign", TitleAct::New, true, false, 24.0);
            ui::button_sized(p, &kit, "Quit", TitleAct::Quit, true, false, 24.0);
            p.spawn(Node { height: Val::Px(60.0), ..default() });
            p.spawn(kit.text(
                "Built on benilla. All art, models and type are read from your own World of Warcraft 1.12.1 install.",
                13.0,
                DIM,
            ));
        });
}

fn act(
    mut commands: Commands,
    q: Query<(&Interaction, &TitleAct), Changed<Interaction>>,
    mut next: ResMut<NextState<Screen>>,
    mut exit: MessageWriter<AppExit>,
    mut narrator: ResMut<ui::Narrator>,
) {
    for (i, a) in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        match a {
            TitleAct::Continue => {
                if let Some(c) = load_save() {
                    commands.insert_resource(Game(c));
                    narrator.say("The hamlet remembers you. Few others do.");
                    next.set(Screen::Hamlet);
                }
            }
            TitleAct::New => {
                let c = Campaign::new(None);
                save(&c);
                commands.insert_resource(Game(c));
                narrator.say(
                    "The deed to the old estate is yours. So is everything buried beneath it.",
                );
                next.set(Screen::Hamlet);
            }
            TitleAct::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}
