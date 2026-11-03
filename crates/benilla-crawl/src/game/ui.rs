//! The interface kit: WoW's own fonts and icons read off the install, a grim palette, buttons,
//! tooltips, and the narrator who comments on everything.

use std::collections::{HashMap, VecDeque};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::wow::WowData;

pub const BONE: Color = Color::srgb(0.87, 0.81, 0.69);
pub const DIM: Color = Color::srgb(0.55, 0.5, 0.42);
pub const GOLD: Color = Color::srgb(0.85, 0.68, 0.32);
pub const BLOOD: Color = Color::srgb(0.72, 0.09, 0.07);
pub const HEAL: Color = Color::srgb(0.35, 0.85, 0.35);
pub const STRESS: Color = Color::srgb(0.85, 0.85, 0.95);
pub const PANEL: Color = Color::srgba(0.04, 0.035, 0.03, 0.93);
pub const PANEL_LIGHT: Color = Color::srgba(0.1, 0.085, 0.07, 0.95);
pub const EDGE: Color = Color::srgb(0.36, 0.28, 0.16);
pub const BTN: Color = Color::srgb(0.16, 0.11, 0.08);
pub const BTN_HOVER: Color = Color::srgb(0.3, 0.19, 0.11);
pub const BTN_DOWN: Color = Color::srgb(0.45, 0.24, 0.12);
pub const BTN_OFF: Color = Color::srgb(0.08, 0.07, 0.065);

#[derive(Resource)]
pub struct Fonts {
    /// Morpheus: the carved-stone display face.
    pub title: Handle<Font>,
    /// Friz Quadrata: WoW's body text.
    pub body: Handle<Font>,
}

#[derive(Resource, Default)]
pub struct Icons(HashMap<String, Handle<Image>>);

/// Fonts, icons and the install in one parameter, for screens that build interface.
#[derive(SystemParam)]
pub struct Kit<'w> {
    pub fonts: Res<'w, Fonts>,
    pub icons: ResMut<'w, Icons>,
    pub data: Res<'w, WowData>,
    pub images: ResMut<'w, Assets<Image>>,
}

impl Kit<'_> {
    /// An image off the install by full path (no extension), cached.
    pub fn image(&mut self, path: &str) -> Handle<Image> {
        if let Some(h) = self.icons.0.get(path) {
            return h.clone();
        }
        let h = match self.data.rgba_image(path) {
            Some(img) => self.images.add(img),
            None => {
                warn!("crawl: missing image {path}");
                match self
                    .data
                    .rgba_image("Interface\\Icons\\INV_Misc_QuestionMark")
                {
                    Some(img) => self.images.add(img),
                    None => Handle::default(),
                }
            }
        };
        self.icons.0.insert(path.to_owned(), h.clone());
        h
    }

    /// An `Interface\Icons\…` icon by bare name.
    pub fn icon(&mut self, name: &str) -> Handle<Image> {
        self.image(&format!("Interface\\Icons\\{name}"))
    }

    pub fn text(&self, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
        (
            Text::new(s),
            TextFont {
                font: self.fonts.body.clone(),
                font_size: size,
                ..default()
            },
            TextColor(color),
        )
    }

    pub fn title(&self, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
        (
            Text::new(s),
            TextFont {
                font: self.fonts.title.clone(),
                font_size: size,
                ..default()
            },
            TextColor(color),
        )
    }
}

/// A clickable button's look; disabled buttons ignore clicks.
#[derive(Component, Clone, Copy)]
pub struct BtnLook {
    pub enabled: bool,
    pub selected: bool,
}

/// Hover text for any node that also has an [`Interaction`].
#[derive(Component, Clone)]
pub struct Tip(pub String);

pub fn panel(width: Val, height: Val) -> impl Bundle {
    (
        Node {
            width,
            height,
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(2.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(EDGE),
    )
}

/// A text button carrying `action`.
pub fn button<A: Component>(
    p: &mut ChildSpawnerCommands,
    kit: &Kit,
    label: &str,
    action: A,
    enabled: bool,
) -> Entity {
    button_sized(p, kit, label, action, enabled, false, 16.0)
}

pub fn button_sized<A: Component>(
    p: &mut ChildSpawnerCommands,
    kit: &Kit,
    label: &str,
    action: A,
    enabled: bool,
    selected: bool,
    size: f32,
) -> Entity {
    p.spawn((
        Button,
        Node {
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(if !enabled {
            BTN_OFF
        } else if selected {
            BTN_DOWN
        } else {
            BTN
        }),
        BorderColor::all(if selected { GOLD } else { EDGE }),
        BtnLook { enabled, selected },
        action,
    ))
    .with_children(|b| {
        b.spawn(kit.text(label, size, if enabled { BONE } else { DIM }));
    })
    .id()
}

/// An icon button carrying `action`, `size` pixels square.
pub fn icon_button<A: Component>(
    p: &mut ChildSpawnerCommands,
    image: Handle<Image>,
    size: f32,
    action: A,
    enabled: bool,
    selected: bool,
    tip: Option<String>,
) -> Entity {
    let mut e = p.spawn((
        Button,
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::BLACK),
        BorderColor::all(if selected { GOLD } else { EDGE }),
        BtnLook { enabled, selected },
        action,
    ));
    e.with_children(|b| {
        b.spawn((
            ImageNode {
                image,
                color: if enabled {
                    Color::WHITE
                } else {
                    Color::srgb(0.3, 0.3, 0.3)
                },
                ..default()
            },
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ));
    });
    if let Some(t) = tip {
        e.insert(Tip(t));
    }
    e.id()
}

/// A horizontal bar: `frac` of it filled with `color`.
pub fn bar(p: &mut ChildSpawnerCommands, width: f32, height: f32, frac: f32, color: Color) {
    p.spawn((
        Node {
            width: Val::Px(width),
            height: Val::Px(height),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.08, 0.02, 0.02)),
        BorderColor::all(Color::BLACK),
    ))
    .with_children(|b| {
        b.spawn((
            Node {
                width: Val::Percent(frac.clamp(0.0, 1.0) * 100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(color),
        ));
    });
}

/// Ten stress pips, each worth ten; past a hundred they burn red.
pub fn stress_pips(p: &mut ChildSpawnerCommands, stress: i32, pip: f32) {
    p.spawn(Node {
        column_gap: Val::Px(1.0),
        ..default()
    })
    .with_children(|row| {
        for i in 0..10 {
            let lit = stress >= (i + 1) * 10;
            let over = stress >= 100 + (i + 1) * 10;
            let color = if over {
                BLOOD
            } else if lit {
                STRESS
            } else {
                Color::srgb(0.12, 0.12, 0.14)
            };
            row.spawn((
                Node {
                    width: Val::Px(pip),
                    height: Val::Px(pip),
                    ..default()
                },
                BackgroundColor(color),
                BorderColor::all(Color::BLACK),
            ));
        }
    });
}

/// The narrator's line queue, shown one at a time across the top of the screen.
#[derive(Resource, Default)]
pub struct Narrator {
    queue: VecDeque<String>,
    showing: Option<(String, f32)>,
}

impl Narrator {
    pub fn say(&mut self, s: impl Into<String>) {
        let s = s.into();
        if self.queue.len() < 4 && self.queue.back() != Some(&s) {
            self.queue.push_back(s);
        }
    }

    pub fn clear(&mut self) {
        self.queue.clear();
        self.showing = None;
    }
}

#[derive(Component)]
struct NarratorText;

#[derive(Component)]
struct TooltipBox;

#[derive(Component)]
struct TooltipText;

/// Floating combat and event text over the stage.
#[derive(Component)]
pub struct Floater {
    pub world: Vec3,
    pub age: f32,
    pub life: f32,
}

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        let fonts = {
            let world = app.world_mut();
            let read = |world: &World, path: &str| -> Option<Font> {
                let data = world.get_resource::<WowData>()?;
                let bytes = data.chain.lock().ok()?.read_file(path).ok()?;
                Font::try_from_bytes(bytes).ok()
            };
            let title = read(world, "Fonts\\MORPHEUS.TTF");
            let body = read(world, "Fonts\\FRIZQT__.TTF");
            let mut assets = world.resource_mut::<Assets<Font>>();
            Fonts {
                title: title.map(|f| assets.add(f)).unwrap_or_default(),
                body: body.map(|f| assets.add(f)).unwrap_or_default(),
            }
        };
        app.insert_resource(fonts)
            .init_resource::<Icons>()
            .init_resource::<Narrator>()
            .add_systems(Startup, spawn_overlay)
            .add_systems(Update, (button_looks, tooltips, narrate, floaters));
    }
}

fn spawn_overlay(mut commands: Commands, fonts: Res<Fonts>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(70.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(50),
            Pickable::IGNORE,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                TextFont {
                    font: fonts.title.clone(),
                    font_size: 30.0,
                    ..default()
                },
                TextColor(BONE.with_alpha(0.0)),
                TextLayout::new_with_justify(Justify::Center),
                TextShadow {
                    offset: Vec2::splat(2.0),
                    color: Color::BLACK,
                },
                Node {
                    max_width: Val::Percent(56.0),
                    ..default()
                },
                NarratorText,
            ));
        });
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                max_width: Val::Px(360.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.02, 0.96)),
            BorderColor::all(GOLD),
            GlobalZIndex(100),
            Pickable::IGNORE,
            TooltipBox,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                TextFont {
                    font: fonts.body.clone(),
                    font_size: 15.0,
                    ..default()
                },
                TextColor(BONE),
                TooltipText,
            ));
        });
}

#[allow(clippy::type_complexity)]
fn button_looks(
    mut sfx: ResMut<crate::audio::Sfx>,
    mut q: Query<
        (&Interaction, &BtnLook, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (i, look, mut bg) in &mut q {
        if !look.enabled {
            bg.0 = BTN_OFF;
            continue;
        }
        if *i == Interaction::Pressed {
            sfx.play(crate::audio::CLICK, -14.0);
        }
        bg.0 = match i {
            Interaction::Pressed => BTN_DOWN,
            Interaction::Hovered => BTN_HOVER,
            Interaction::None if look.selected => BTN_DOWN,
            Interaction::None => BTN,
        };
    }
}

fn tooltips(
    tips: Query<(&Interaction, &Tip)>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut boxq: Query<&mut Node, With<TooltipBox>>,
    mut text: Query<&mut Text, With<TooltipText>>,
) {
    let Ok(mut node) = boxq.single_mut() else {
        return;
    };
    let hovered = tips
        .iter()
        .find(|(i, _)| matches!(i, Interaction::Hovered | Interaction::Pressed));
    let cursor = window
        .single()
        .ok()
        .and_then(|w| w.cursor_position().map(|c| (c, w.width(), w.height())));
    match (hovered, cursor) {
        (Some((_, tip)), Some((c, w, h))) => {
            node.display = Display::Flex;
            let x = if c.x > w - 380.0 {
                c.x - 370.0
            } else {
                c.x + 18.0
            };
            let y = if c.y > h - 200.0 {
                c.y - 160.0
            } else {
                c.y + 18.0
            };
            node.left = Val::Px(x);
            node.top = Val::Px(y);
            if let Ok(mut t) = text.single_mut() {
                if t.0 != tip.0 {
                    t.0.clone_from(&tip.0);
                }
            }
        }
        _ => node.display = Display::None,
    }
}

fn narrate(
    time: Res<Time>,
    mut n: ResMut<Narrator>,
    mut q: Query<(&mut Text, &mut TextColor), With<NarratorText>>,
) {
    let Ok((mut text, mut color)) = q.single_mut() else {
        return;
    };
    const LIFE: f32 = 4.5;
    if n.showing.is_none() {
        if let Some(next) = n.queue.pop_front() {
            n.showing = Some((next, 0.0));
        }
    }
    // Hurry along when more is waiting.
    let life = if n.queue.is_empty() { LIFE } else { LIFE * 0.6 };
    let Some((line, age)) = n.showing.as_mut() else {
        if !text.0.is_empty() {
            text.0.clear();
        }
        return;
    };
    *age += time.delta_secs();
    let a = *age;
    if text.0 != *line {
        text.0.clone_from(line);
    }
    let alpha = (a / 0.4).min(1.0).min(((life - a) / 0.6).max(0.0));
    color.0 = BONE.with_alpha(alpha);
    if a > life {
        n.showing = None;
    }
}

fn floaters(
    mut commands: Commands,
    time: Res<Time>,
    cams: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut q: Query<(Entity, &mut Floater, &mut Node, &mut TextColor)>,
) {
    let Some((cam, cam_t)) = cams.iter().find(|(c, _)| c.is_active) else {
        return;
    };
    for (e, mut f, mut node, mut color) in &mut q {
        f.age += time.delta_secs();
        if f.age > f.life {
            commands.entity(e).despawn();
            continue;
        }
        let rise = f.age * 0.6;
        if let Ok(p) = cam.world_to_viewport(cam_t, f.world + Vec3::Y * rise) {
            node.left = Val::Px(p.x - 60.0);
            node.top = Val::Px(p.y);
        }
        let a = (1.0 - (f.age / f.life).powi(3)).clamp(0.0, 1.0);
        color.0 = color.0.with_alpha(a);
    }
}

/// Pop a floating line of text over a point on the stage.
pub fn float_text(
    commands: &mut Commands,
    fonts: &Fonts,
    world: Vec3,
    s: impl Into<String>,
    color: Color,
    size: f32,
) {
    commands.spawn((
        Text::new(s),
        TextFont {
            font: fonts.title.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
        TextShadow {
            offset: Vec2::splat(2.0),
            color: Color::BLACK,
        },
        TextLayout::new_with_justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(120.0),
            ..default()
        },
        GlobalZIndex(40),
        Pickable::IGNORE,
        Floater {
            world,
            age: 0.0,
            life: 1.6,
        },
    ));
}

#[cfg(test)]
mod tests {
    /// Every icon the source names (`Spell_…`, `Ability_…`, `INV_…`, …) is in the install, so
    /// none draws as the red question mark (skipped with no `WOW_DATA`).
    #[test]
    fn named_icons_exist() {
        let Some(dir) = std::env::var_os("WOW_DATA") else {
            eprintln!("skipped: no WOW_DATA");
            return;
        };
        let mut chain = benilla_formats::open_chain(std::path::Path::new(&dir)).expect("chain");
        let mut names = std::collections::BTreeSet::new();
        let mut stack = vec![std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src"
        ))];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).expect("src dir").flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let text = std::fs::read_to_string(&p).expect("source");
                    for lit in text.split('"').skip(1).step_by(2) {
                        let prefixed = ["Spell_", "Ability_", "INV_", "Trade_", "Racial_", "Temp_"]
                            .iter()
                            .any(|pre| lit.starts_with(pre));
                        if prefixed && lit.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                            names.insert(lit.to_owned());
                        }
                    }
                }
            }
        }
        assert!(names.len() > 50, "found only {} icon names", names.len());
        let missing: Vec<&String> = names
            .iter()
            .filter(|n| {
                chain
                    .read_file(&format!(r"Interface\Icons\{n}.blp"))
                    .is_err()
            })
            .collect();
        assert!(
            missing.is_empty(),
            "icons missing from the install: {missing:#?}"
        );
    }
}
