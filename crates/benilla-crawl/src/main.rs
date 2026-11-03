//! Darkened Deeps: a turn-based, stress-driven dungeon crawler that dresses itself in the player's
//! own World of Warcraft 1.12.1 install.

pub mod audio;
pub mod game;
pub mod rules;
pub mod wow;

use bevy::prelude::*;

use wow::{WowData, WowPlugin};

fn main() -> AppExit {
    let Some(data_dir) = benilla_formats::wow_data() else {
        eprintln!(
            "Darkened Deeps needs a WoW 1.12.1 install: set WOW_DATA to its Data folder. Looked in {:?}",
            benilla_formats::candidates()
        );
        return AppExit::error();
    };
    let data = match WowData::load(&data_dir) {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "could not read the install at {}: {e:#}",
                data_dir.display()
            );
            return AppExit::error();
        }
    };

    let mut app = App::new();
    if let Err(e) = benilla_assets::register_mpq_source(&mut app, &data_dir) {
        eprintln!("mpq:// source unavailable: {e:#}");
        return AppExit::error();
    }
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Darkened Deeps".into(),
            resolution: (1600u32, 900u32).into(),
            ..default()
        }),
        ..default()
    }));
    benilla_assets::register_asset_loaders(&mut app);
    app.insert_resource(data)
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(WowPlugin)
        .add_plugins(audio::AudioPlugin)
        .add_plugins(game::GamePlugin)
        .add_systems(Update, shoot);
    app.run()
}

fn shoot(
    time: Res<Time>,
    mut commands: Commands,
    mut next: Local<usize>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(path) = std::env::var("CRAWL_SHOT") else {
        return;
    };
    let times: Vec<f32> = std::env::var("CRAWL_SHOT_AT")
        .unwrap_or_else(|_| "6".into())
        .split(',')
        .filter_map(|t| t.trim().parse().ok())
        .collect();
    let now = time.elapsed_secs();
    if let Some(&at) = times.get(*next) {
        if now > at {
            let file = if times.len() == 1 {
                path
            } else {
                path.replace(".png", &format!("_{}.png", *next))
            };
            commands
                .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(file));
            *next += 1;
        }
    } else if times.last().is_some_and(|l| now > l + 1.5) {
        exit.write(AppExit::Success);
    }
}
