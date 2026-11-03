//! The game around the rules: screens, the 3D stage, and the interface.

use bevy::prelude::*;

use crate::rules::dungeon::Expedition;
use crate::rules::town::{Campaign, Debrief};

pub mod battle;
pub mod explore;
pub mod hamlet;
pub mod looks;
pub mod portrait;
pub mod stage;
pub mod title;
pub mod ui;

#[derive(States, Default, Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum Screen {
    #[default]
    Title,
    Hamlet,
    Provision,
    Dungeon,
    Debrief,
}

/// The campaign in progress.
#[derive(Resource)]
pub struct Game(pub Campaign);

/// The expedition under way.
#[derive(Resource)]
pub struct Run(pub Expedition);

/// What the last expedition brought home.
#[derive(Resource, Default)]
pub struct LastDebrief(pub Option<Debrief>);

/// Everything spawned for one screen, despawned on leaving it.
#[derive(Component, Clone, Copy)]
pub struct Scoped(pub Screen);

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<Screen>()
            .init_resource::<LastDebrief>()
            .add_plugins((
                ui::UiPlugin,
                title::TitlePlugin,
                hamlet::HamletPlugin,
                stage::StagePlugin,
                explore::ExplorePlugin,
                battle::BattlePlugin,
                portrait::PortraitPlugin,
            ))
            .add_systems(Update, autostart.run_if(in_state(Screen::Title)));
        for s in [
            Screen::Title,
            Screen::Hamlet,
            Screen::Provision,
            Screen::Dungeon,
            Screen::Debrief,
        ] {
            app.add_systems(OnExit(s), despawn_scoped(s));
        }
    }
}

fn despawn_scoped(screen: Screen) -> impl Fn(Commands, Query<(Entity, &Scoped)>) {
    move |mut commands: Commands, q: Query<(Entity, &Scoped)>| {
        for (e, s) in &q {
            if s.0 == screen {
                commands.entity(e).try_despawn();
            }
        }
    }
}

/// Where the campaign is saved: `benilla-config/darkened-deeps.json` at the workspace root in a
/// dev build, else beside the binary. Never inside the WoW install.
pub fn save_path() -> std::path::PathBuf {
    let dev_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let base = if dev_root.join("Cargo.toml").exists() {
        dev_root
    } else {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_default()
    };
    base.join("benilla-config").join("darkened-deeps.json")
}

pub fn save(c: &Campaign) {
    let path = save_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match c.to_json() {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                warn!("crawl: could not save to {}: {e}", path.display());
            }
        }
        Err(e) => warn!("crawl: could not serialise the campaign: {e:#}"),
    }
}

pub fn load_save() -> Option<Campaign> {
    let s = std::fs::read_to_string(save_path()).ok()?;
    Campaign::from_json(&s).ok()
}

/// `CRAWL_AUTOSTART=hamlet|dungeon|battle` jumps past the title with a seeded campaign, for
/// screenshots and smoke runs; it never touches the save.
fn autostart(mut commands: Commands, mut next: ResMut<NextState<Screen>>, mut done: Local<bool>) {
    if *done {
        return;
    }
    *done = true;
    let Ok(mode) = std::env::var("CRAWL_AUTOSTART") else {
        return;
    };
    let mut c = Campaign::new(Some(7));
    match mode.as_str() {
        "hamlet" => {
            commands.insert_resource(Game(c));
            next.set(Screen::Hamlet);
        }
        "dungeon" | "battle" => {
            let ids: Vec<u32> = c.roster.iter().map(|h| h.id).collect();
            let quest = crate::rules::dungeon::Quest::Explore(90);
            if let Some(run) = c.embark(&ids, crate::rules::town::default_pack(), quest) {
                commands.insert_resource(Run(run));
            }
            commands.insert_resource(Game(c));
            if mode == "battle" {
                commands.insert_resource(battle::PendingBattle {
                    pack: crate::rules::data::CRYPT_PACKS[1]
                        .iter()
                        .map(|s| (*s).to_owned())
                        .collect(),
                    boss: false,
                });
            }
            next.set(Screen::Dungeon);
        }
        _ => {}
    }
}
