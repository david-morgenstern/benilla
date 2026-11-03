//! Exploring the crypts: walking the halls with the torch burning down, rooms on the map, curios,
//! rubble and traps, and the walk home.

use bevy::prelude::*;

use super::battle::PendingBattle;
use super::looks::{hero_height, hero_look};
use super::stage::{self, Actor, ActorView, Mark, Plate, Walk, SEG_LEN};
use super::ui::{self, Kit, Narrator, Tip, BLOOD, BONE, DIM, EDGE, GOLD, HEAL, STRESS};
use super::{Game, LastDebrief, Run, Scoped, Screen};
use crate::rules::combat::Side;
use crate::rules::data::anim;
use crate::rules::dungeon::{curio, Encounter, Item, Location, Note, RoomKind, Seg, HALL_MOUTH};
use crate::wow::{ModelSource, PlayAnim, WowData, WowModel};

#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    /// Standing still: in a room, or paused in a hall.
    Idle,
    Walking,
    Battle,
    Curio(String),
    Obstacle,
    /// Around the fire, planning the night's camp skills.
    Camp,
}

#[derive(Resource)]
pub struct Explore {
    pub mode: Mode,
    /// The party member items are used on.
    pub selected: usize,
    walked: f32,
    pub dirty: bool,
    /// The curio or rubble standing ahead.
    prop: Option<Entity>,
    /// The hall the party entered its current room by, for a retreat.
    pub came_from: Option<usize>,
    last_light: i32,
    /// The camp skills planned for tonight: (hero index, skill id, target hero index).
    camp: Vec<(usize, &'static str, Option<usize>)>,
}

#[derive(Component, Clone)]
enum Act {
    Go(usize),
    Walk,
    TurnBack,
    Investigate(Option<Item>),
    LeaveCurio,
    Clear,
    UseItem(Item),
    Leave,
    Camp,
    CampUse(usize, &'static str),
    CampUndo,
    CampRest,
}

#[derive(Component)]
struct HudRoot;

/// A camp flame's licking phase.
#[derive(Component)]
struct Flame(f32);

fn lick_flames(time: Res<Time>, mut q: Query<(&mut Transform, &Flame)>) {
    let t = time.elapsed_secs();
    for (mut tr, f) in &mut q {
        let k = 1.0 + 0.18 * (t * 9.0 + f.0).sin() * (t * 3.7 + f.0).cos();
        tr.scale = Vec3::new(1.0 / k.sqrt(), k, 1.0 / k.sqrt());
    }
}

/// Orders queued by `CRAWL_AUTOPLAY`, carried out as if clicked.
#[derive(Resource, Default)]
struct AutoQueue(Vec<Act>, f32);

pub struct ExplorePlugin;

impl Plugin for ExplorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AutoQueue>()
            .add_systems(OnEnter(Screen::Dungeon), enter)
            .add_systems(
                Update,
                (
                    lick_flames,
                    autoplay,
                    act,
                    plate_clicks,
                    walk,
                    sync_hero_views,
                    hud,
                    check_wipe,
                )
                    .chain()
                    .run_if(in_state(Screen::Dungeon).and(resource_exists::<Run>)),
            );
    }
}

fn enter(
    mut commands: Commands,
    run: Res<Run>,
    data: Res<WowData>,
    mut narrator: ResMut<Narrator>,
    mut music: ResMut<crate::audio::Music>,
    mut auto: ResMut<AutoQueue>,
) {
    narrator.clear();
    *music = crate::audio::Music::Explore(run.0.dungeon.quest.region);
    narrator.say(match run.0.dungeon.quest.region {
        crate::rules::regions::Region::Crypts => "The stair winds down into the Sunken Crypts. The air tastes of dust and old candles.",
        crate::rules::regions::Region::Warrens => "Down the old mineshaft. Somewhere ahead, a thousand candles flicker, and something squeaks.",
        crate::rules::regions::Region::Grotto => "The cellar stair ends in black water. The walls sweat brine, and something sings.",
    });
    narrator.say(run.0.dungeon.quest.label());
    for (i, h) in run.0.party.iter().enumerate() {
        spawn_hero(&mut commands, &data, h, i);
    }
    // `CRAWL_AUTOSTART=camp` makes camp at once, for looking at the camp.
    if std::env::var("CRAWL_AUTOSTART").is_ok_and(|m| m == "camp") {
        auto.0.push(Act::Camp);
    }
    commands.insert_resource(Explore {
        mode: Mode::Idle,
        selected: 0,
        walked: 0.0,
        dirty: true,
        prop: None,
        came_from: None,
        last_light: run.0.light,
        camp: Vec::new(),
    });
}

pub fn spawn_hero(
    commands: &mut Commands,
    data: &WowData,
    h: &crate::rules::hero::Hero,
    index: usize,
) -> Entity {
    let look = hero_look(data, h);
    let height = hero_height(&look);
    let home = stage::slot(Side::Heroes, (index + 1) as f32);
    stage::spawn_actor(
        commands,
        Actor {
            side: Side::Heroes,
            hero: Some(h.id),
            unit: None,
            home,
            lunge: 0.0,
            alive: true,
        },
        ModelSource::Character(Box::new(look)),
        height,
        ActorView {
            name: h.name.clone(),
            hp: h.hp,
            max_hp: h.max_hp(),
            stress: Some(h.stress),
            ..default()
        },
        home,
    )
}

fn notes_to_narrator(notes: &[Note], run: &Run, narrator: &mut Narrator) {
    for n in notes {
        let name = |i: usize| {
            run.0
                .party
                .get(i)
                .map_or("Someone".to_owned(), |h| h.name.clone())
        };
        match n {
            Note::Text(t) => narrator.say(t.clone()),
            Note::Gold(g) => narrator.say(format!("{g} gold, cold and heavy.")),
            Note::Heirloom(h, n) => narrator.say(format!(
                "{} × {n} — pieces of a house long fallen.",
                h.name()
            )),
            Note::Item(i, n) if *n > 0 => narrator.say(format!("Found: {} × {n}.", i.name())),
            Note::Afflicted { hero, name: a } => {
                narrator.say(format!("{}'s resolve is tested… {}!", name(*hero), a))
            }
            Note::Virtuous { hero, name: v } => {
                narrator.say(format!("{}'s resolve is tested… {}!", name(*hero), v))
            }
            Note::Died { .. } => {
                narrator.say("A heart stops in the dark. The others do not look back.")
            }
            Note::Trinket(id) => {
                if let Some(t) = crate::rules::items::trinket(id) {
                    narrator.say(format!("A trinket: {} ({}).", t.name, t.rarity.name()));
                }
            }
            Note::Scouted => narrator.say("The torchlight reaches far down the halls. Scouted."),
            _ => {}
        }
    }
}

/// Floating numbers over the heroes for what the notes did to them.
fn notes_to_floaters(
    commands: &mut Commands,
    fonts: &ui::Fonts,
    notes: &[Note],
    run: &Run,
    actors: &Query<(&Actor, &GlobalTransform)>,
) {
    let pos = |i: usize| -> Option<Vec3> {
        let id = run.0.party.get(i)?.id;
        actors
            .iter()
            .find(|(a, _)| a.hero == Some(id))
            .map(|(_, t)| t.translation() + Vec3::Y * 2.2)
    };
    for n in notes {
        match n {
            Note::HeroStress { hero, amount } => {
                if let Some(p) = pos(*hero) {
                    ui::float_text(
                        commands,
                        fonts,
                        p,
                        format!("{amount:+} Stress"),
                        STRESS,
                        22.0,
                    );
                }
            }
            Note::HeroHurt { hero, amount } => {
                if let Some(p) = pos(*hero) {
                    ui::float_text(commands, fonts, p, format!("-{amount}"), BLOOD, 28.0);
                }
            }
            Note::HeroHeal { hero, amount } => {
                if let Some(p) = pos(*hero) {
                    ui::float_text(commands, fonts, p, format!("+{amount}"), HEAL, 28.0);
                }
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_encounter(
    commands: &mut Commands,
    ex: &mut Explore,
    run: &Run,
    enc: Encounter,
    narrator: &mut Narrator,
) {
    match enc {
        Encounter::None => {
            if matches!(run.0.dungeon.at, Location::Room(_)) {
                ex.mode = Mode::Idle;
                narrator.say(room_line(run));
            }
        }
        Encounter::Battle(pack) => {
            let boss = matches!(run.0.dungeon.at, Location::Room(r) if matches!(run.0.dungeon.rooms[r].kind, RoomKind::Boss(_)));
            commands.insert_resource(PendingBattle { pack, boss });
            ex.mode = Mode::Battle;
        }
        Encounter::Curio(id) => {
            if let Some(c) = curio(&id) {
                narrator.say(format!("{}. {}", c.name, c.desc));
                let e = commands
                    .spawn((
                        Transform::from_xyz(2.8, 0.0, -0.6)
                            .with_rotation(Quat::from_rotation_y(0.4)),
                        Visibility::default(),
                        WowModel {
                            source: ModelSource::Prop(c.model.to_owned(), None),
                            height: Some(1.5),
                        },
                        Scoped(Screen::Dungeon),
                    ))
                    .id();
                ex.prop = Some(e);
            }
            ex.mode = Mode::Curio(id);
        }
        Encounter::Obstacle => {
            narrator.say("Fallen masonry chokes the passage.");
            let e = commands
                .spawn((
                    Transform::from_xyz(3.0, 0.0, -0.4),
                    Visibility::default(),
                    WowModel {
                        source: ModelSource::Prop("World\\Azeroth\\Karazahn\\PassiveDoodads\\Rubble\\KarazahnRockRubble01.m2".into(), None),
                        height: Some(1.6),
                    },
                    Scoped(Screen::Dungeon),
                ))
                .id();
            ex.prop = Some(e);
            ex.mode = Mode::Obstacle;
        }
        Encounter::Trap => {
            // Sprung by the caller, which owns the run mutably.
        }
    }
}

fn room_line(run: &Run) -> String {
    let lines = [
        "An empty chamber. Empty of the living, at least.",
        "Dust, bones, and the echo of your own breathing.",
        "This room has been quiet a long time. It would like to stay that way.",
        "Cobwebs, and the shapes of things that spun them.",
    ];
    lines[run.0.dungeon.steps as usize % lines.len()].to_owned()
}

#[allow(clippy::too_many_arguments)]
fn walk(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut ex: ResMut<Explore>,
    mut run: ResMut<Run>,
    mut w: ResMut<Walk>,
    mut narrator: ResMut<Narrator>,
    fonts: Res<ui::Fonts>,
    actors: Query<(&Actor, &GlobalTransform)>,
    mut anims: Query<(&Actor, &mut PlayAnim)>,
) {
    if keys.just_pressed(KeyCode::Space) && matches!(run.0.dungeon.at, Location::Hall { .. }) {
        ex.mode = match ex.mode {
            Mode::Walking => Mode::Idle,
            Mode::Idle => Mode::Walking,
            ref m => m.clone(),
        };
        ex.dirty = true;
    }
    let in_hall = matches!(run.0.dungeon.at, Location::Hall { .. });
    let held = keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight);
    let walking = ex.mode == Mode::Walking || (held && in_hall && ex.mode == Mode::Idle);
    for (a, mut p) in &mut anims {
        if a.side == Side::Heroes {
            let want = if ex.mode == Mode::Camp {
                anim::SIT_GROUND
            } else if walking {
                anim::WALK
            } else {
                anim::STAND
            };
            let idle = [anim::WALK, anim::STAND, anim::SIT_GROUND];
            if p.looping && p.id != want && idle.contains(&p.id) {
                *p = PlayAnim::looped(want);
            }
        }
    }
    if !walking {
        return;
    }
    const SPEED: f32 = 3.2;
    let d = SPEED * time.delta_secs();
    w.offset += d;
    ex.walked += d;
    if ex.walked < SEG_LEN {
        return;
    }
    ex.walked = 0.0;
    let (enc, notes) = run.0.advance();
    notes_to_narrator(&notes, &run, &mut narrator);
    notes_to_floaters(&mut commands, &fonts, &notes, &run, &actors);
    ex.dirty = true;
    if run.0.light != ex.last_light {
        let old = ex.last_light;
        ex.last_light = run.0.light;
        for (threshold, line) in [
            (75, "The torch gutters. Shadows lengthen."),
            (50, "The light dims. Something shifts at its edge."),
            (25, "Darkness presses close. Every sound is a footstep."),
            (0, "Pitch black. Only the breathing of things unseen."),
        ] {
            if old > threshold && run.0.light <= threshold {
                narrator.say(line);
            }
        }
    }
    if enc == Encounter::Trap {
        let notes = run.0.spring_trap();
        notes_to_narrator(&notes, &run, &mut narrator);
        notes_to_floaters(&mut commands, &fonts, &notes, &run, &actors);
        return;
    }
    let in_room = matches!(run.0.dungeon.at, Location::Room(_));
    if in_room {
        ex.mode = Mode::Idle;
    }
    handle_encounter(&mut commands, &mut ex, &run, enc, &mut narrator);
}

fn sync_hero_views(
    mut commands: Commands,
    run: Res<Run>,
    ex: Res<Explore>,
    mut q: Query<(Entity, &mut Actor, &mut ActorView)>,
) {
    if ex.mode == Mode::Battle {
        return;
    }
    for (e, mut a, mut v) in &mut q {
        let Some(id) = a.hero else {
            continue;
        };
        let Some((i, h)) = run.0.party.iter().enumerate().find(|(_, h)| h.id == id) else {
            // Lost outside battle (a heart gave out): off the stage.
            commands.entity(e).try_despawn();
            continue;
        };
        let home = stage::slot(Side::Heroes, (i + 1) as f32);
        if a.home != home {
            a.home = home;
        }
        v.hp = h.hp;
        v.max_hp = h.max_hp();
        v.stress = Some(h.stress);
        v.deaths_door = false;
        v.status = h
            .affliction
            .map(|a| a.name().to_owned())
            .or(h.virtue.map(|v| v.name().to_owned()))
            .unwrap_or_default();
        v.mark = if i == ex.selected {
            Mark::Selected
        } else {
            Mark::None
        };
    }
}

fn plate_clicks(
    q: Query<(&Interaction, &Plate), Changed<Interaction>>,
    actors: Query<&Actor>,
    run: Res<Run>,
    mut ex: ResMut<Explore>,
) {
    if ex.mode == Mode::Battle {
        return;
    }
    for (i, p) in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        if let Ok(a) = actors.get(p.actor) {
            if let Some(id) = a.hero {
                if let Some(idx) = run.0.party.iter().position(|h| h.id == id) {
                    ex.selected = idx;
                    ex.dirty = true;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn act(
    mut commands: Commands,
    q: Query<(&Interaction, &Act, Option<&ui::BtnLook>), Changed<Interaction>>,
    mut ex: ResMut<Explore>,
    mut run: ResMut<Run>,
    mut game: ResMut<Game>,
    mut narrator: ResMut<Narrator>,
    mut next: ResMut<NextState<Screen>>,
    mut last: ResMut<LastDebrief>,
    fonts: Res<ui::Fonts>,
    actors: Query<(&Actor, &GlobalTransform)>,
    mut auto: ResMut<AutoQueue>,
    offer: Res<super::ActiveOffer>,
    mut sfx: ResMut<crate::audio::Sfx>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let mut acts: Vec<Act> = q
        .iter()
        .filter(|(i, _, look)| **i == Interaction::Pressed && !look.is_some_and(|l| !l.enabled))
        .map(|(_, a, _)| a.clone())
        .collect();
    acts.append(&mut auto.0);
    for a in acts {
        ex.dirty = true;
        match a {
            Act::Go(hall) => {
                if run.0.enter_hall(hall) {
                    ex.mode = Mode::Walking;
                    ex.walked = 0.0;
                    ex.came_from = Some(hall);
                }
            }
            Act::Walk => {
                ex.mode = if ex.mode == Mode::Walking {
                    Mode::Idle
                } else {
                    Mode::Walking
                };
            }
            Act::TurnBack => {
                run.0.turn_back();
                narrator.say("Back the way we came. Nobody argues.");
                if matches!(run.0.dungeon.at, Location::Room(_)) {
                    ex.mode = Mode::Idle;
                } else {
                    ex.mode = Mode::Walking;
                    ex.walked = SEG_LEN - ex.walked;
                }
            }
            Act::Investigate(item) => {
                if let Mode::Curio(id) = ex.mode.clone() {
                    let hero = ex.selected.min(run.0.party.len().saturating_sub(1));
                    let notes = run.0.interact(&id, item, hero);
                    notes_to_narrator(&notes, &run, &mut narrator);
                    notes_to_floaters(&mut commands, &fonts, &notes, &run, &actors);
                    if let Some(e) = ex.prop.take() {
                        commands.entity(e).despawn();
                    }
                    ex.mode = resume_mode(&run);
                }
            }
            Act::LeaveCurio => {
                if let Some(e) = ex.prop.take() {
                    commands.entity(e).despawn();
                }
                narrator.say("Some things are best left undisturbed.");
                run.0.clear_here();
                ex.mode = resume_mode(&run);
            }
            Act::Clear => {
                let notes = run.0.clear_obstacle();
                notes_to_narrator(&notes, &run, &mut narrator);
                notes_to_floaters(&mut commands, &fonts, &notes, &run, &actors);
                if let Some(e) = ex.prop.take() {
                    commands.entity(e).despawn();
                }
                ex.mode = resume_mode(&run);
            }
            Act::UseItem(item) => {
                if item == Item::Torch {
                    if run.0.use_torch() {
                        narrator.say("A fresh torch flares. The shadows retreat, sullenly.");
                    }
                } else {
                    let notes = run.0.use_on_hero(item, ex.selected);
                    notes_to_floaters(&mut commands, &fonts, &notes, &run, &actors);
                }
            }
            Act::Leave => {
                let success = run.0.dungeon.quest_done();
                let mut exp = run.0.clone();
                if !success {
                    // Abandoning the expedition shames the party.
                    let mut notes = Vec::new();
                    for h in 0..exp.party.len() {
                        exp.stress_hero(h, 20, &mut notes);
                    }
                }
                let d = game.0.debrief(exp, success, offer.0.as_ref());
                last.0 = Some(d);
                commands.remove_resource::<Run>();
                next.set(Screen::Debrief);
                return;
            }
            Act::Camp => {
                if run.0.can_camp() {
                    ex.mode = Mode::Camp;
                    ex.camp.clear();
                    narrator.say("The party makes camp. The fire is small, and the dark is large.");
                    let e = commands
                        .spawn((
                            Transform::from_xyz(1.5, 0.0, 0.5),
                            Visibility::default(),
                            WowModel {
                                source: ModelSource::Prop("World\\Azeroth\\Elwynn\\PassiveDoodads\\Campfire\\ElwynnCampfire.m2".into(), None),
                                height: Some(0.8),
                            },
                            PointLight { color: Color::srgb(1.0, 0.55, 0.2), intensity: 400_000.0, range: 10.0, ..default() },
                            stage::Sconce { base: 400_000.0, phase: 0.0 },
                            Scoped(Screen::Dungeon),
                        ))
                        .id();
                    // The model's flames are particles, which the stage does not draw: a glowing
                    // tongue of fire in their place, licked about by `lick_flames`.
                    for (i, (r, h, color)) in [
                        (0.2, 0.55, Color::srgba(1.0, 0.45, 0.1, 0.75)),
                        (0.11, 0.38, Color::srgba(1.0, 0.85, 0.4, 0.9)),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let flame = commands
                            .spawn((
                                Mesh3d(meshes.add(Cone {
                                    radius: r,
                                    height: h,
                                })),
                                MeshMaterial3d(mats.add(StandardMaterial {
                                    base_color: color,
                                    emissive: color.to_linear() * 6.0,
                                    unlit: true,
                                    alpha_mode: AlphaMode::Add,
                                    ..default()
                                })),
                                Transform::from_xyz(0.0, 0.12 + h / 2.0, 0.0),
                                Flame(i as f32 * 1.9),
                                bevy::light::NotShadowCaster,
                            ))
                            .id();
                        commands.entity(e).add_child(flame);
                    }
                    ex.prop = Some(e);
                }
            }
            Act::CampUse(hero, skill) => {
                let used: u8 = ex
                    .camp
                    .iter()
                    .filter_map(|(_, id, _)| crate::rules::camp::camp_skill(id))
                    .map(|s| s.hours)
                    .sum();
                if let Some(cs) = crate::rules::camp::camp_skill(skill) {
                    let dup = ex.camp.iter().any(|(h, id, _)| *h == hero && *id == skill);
                    if !dup && used + cs.hours <= crate::rules::camp::CAMP_HOURS {
                        let target = (cs.target == crate::rules::camp::CampTarget::Ally)
                            .then_some(ex.selected);
                        ex.camp.push((hero, skill, target));
                    }
                }
            }
            Act::CampUndo => {
                ex.camp.pop();
            }
            Act::CampRest => {
                let uses: Vec<_> = ex
                    .camp
                    .iter()
                    .filter_map(|(h, id, t)| {
                        crate::rules::camp::camp_skill(id).map(|s| (*h, s, *t))
                    })
                    .collect();
                let report = run.0.camp(&uses);
                notes_to_narrator(&report.notes, &run, &mut narrator);
                notes_to_floaters(&mut commands, &fonts, &report.notes, &run, &actors);
                if let Some(e) = ex.prop.take() {
                    commands.entity(e).despawn();
                }
                ex.camp.clear();
                if let Some(pack) = report.ambush {
                    sfx.play(crate::audio::AFFLICTION, -4.0);
                    commands.insert_resource(PendingBattle { pack, boss: false });
                    ex.mode = Mode::Battle;
                } else {
                    ex.mode = Mode::Idle;
                }
            }
        }
    }
}

/// After an interruption, walk on in a hall or stand in a room.
pub fn resume_mode(run: &Run) -> Mode {
    match run.0.dungeon.at {
        Location::Hall { .. } => Mode::Walking,
        Location::Room(_) => Mode::Idle,
    }
}

fn check_wipe(
    mut commands: Commands,
    run: Res<Run>,
    ex: Res<Explore>,
    mut game: ResMut<Game>,
    mut last: ResMut<LastDebrief>,
    mut next: ResMut<NextState<Screen>>,
    offer: Res<super::ActiveOffer>,
) {
    if ex.mode == Mode::Battle || !run.0.party.is_empty() {
        return;
    }
    let d = game.0.debrief(run.0.clone(), false, offer.0.as_ref());
    last.0 = Some(d);
    commands.remove_resource::<Run>();
    next.set(Screen::Debrief);
}

fn light_label(light: i32) -> (&'static str, Color) {
    match light {
        76.. => ("Radiant", Color::srgb(1.0, 0.85, 0.5)),
        51..=75 => ("Dim", Color::srgb(0.9, 0.65, 0.35)),
        26..=50 => ("Shadowy", Color::srgb(0.7, 0.45, 0.25)),
        1..=25 => ("Dark", Color::srgb(0.5, 0.28, 0.2)),
        _ => ("Pitch Black", Color::srgb(0.4, 0.1, 0.1)),
    }
}

#[allow(clippy::too_many_lines)]
fn hud(
    mut commands: Commands,
    mut kit: Kit,
    mut ex: ResMut<Explore>,
    run: Res<Run>,
    old: Query<Entity, With<HudRoot>>,
    mut portrait: ResMut<super::portrait::Portrait>,
) {
    // Keep the torch meter honest even when nothing else changed.
    if ex.mode == Mode::Battle {
        if !old.is_empty() {
            for e in &old {
                commands.entity(e).despawn();
            }
            ex.dirty = true;
        }
        return;
    }
    if !ex.dirty {
        return;
    }
    ex.dirty = false;
    for e in &old {
        commands.entity(e).despawn();
    }
    let r = &run.0;
    ex.selected = ex.selected.min(r.party.len().saturating_sub(1));
    portrait.want = r.party.get(ex.selected).map(|h| h.id);
    let portrait_img = portrait.image.clone();
    let (label, lcolor) = light_label(r.light);
    // Top bar.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(16.0)),
                ..default()
            },
            HudRoot,
            Scoped(Screen::Dungeon),
        ))
        .with_children(|top| {
            top.spawn(Node {
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|c| {
                c.spawn(kit.title(r.dungeon.quest.title(), 24.0, GOLD));
                c.spawn(kit.text(
                    format!(
                        "{} — {}{}  ·  firewood {}",
                        r.dungeon.quest.label(),
                        r.dungeon.progress(),
                        if r.dungeon.quest_done() {
                            " (complete)"
                        } else {
                            ""
                        },
                        r.firewood
                    ),
                    14.0,
                    if r.dungeon.quest_done() { HEAL } else { DIM },
                ));
            });
            top.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|c| {
                c.spawn(kit.title(format!("Torchlight: {label} ({})", r.light), 20.0, lcolor));
                ui::bar(c, 360.0, 14.0, r.light as f32 / 100.0, lcolor);
            });
            top.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                ..default()
            })
            .with_children(|c| {
                c.spawn(kit.title(format!("{} gold", r.gold), 22.0, GOLD));
                let heir: i32 = r.heirlooms.values().sum();
                c.spawn(kit.text(format!("{heir} heirlooms"), 14.0, DIM));
            });
        });
    // Bottom panel.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(200.0),
                padding: UiRect::all(Val::Px(10.0)),
                column_gap: Val::Px(14.0),
                border: UiRect::top(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(ui::PANEL),
            BorderColor::all(EDGE),
            HudRoot,
            Scoped(Screen::Dungeon),
        ))
        .with_children(|bottom| {
            // Selected hero.
            if !r.party.is_empty() {
                portrait_frame(bottom, portrait_img.clone(), 178.0);
            }
            bottom
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Px(360.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|c| {
                    if let Some(h) = r.party.get(ex.selected) {
                        c.spawn(kit.title(
                            format!("{} the {}", h.name, h.class_name()),
                            24.0,
                            GOLD,
                        ));
                        c.spawn(kit.text(
                            format!(
                                "HP {}/{}   Stress {}/200   Resolve {}",
                                h.hp,
                                h.max_hp(),
                                h.stress,
                                h.level
                            ),
                            15.0,
                            BONE,
                        ));
                        let st = h.stats();
                        c.spawn(kit.text(
                            format!(
                                "ACC {:+}  CRIT {}%  DMG {}–{}  DODGE {}  PROT {}%  SPD {}",
                                st.acc, st.crit, st.dmg.0, st.dmg.1, st.dodge, st.prot, st.speed
                            ),
                            13.0,
                            DIM,
                        ));
                        if let Some(a) = h.affliction {
                            c.spawn(kit.title(format!("Afflicted: {}", a.name()), 18.0, BLOOD));
                        } else if let Some(v) = h.virtue {
                            c.spawn(kit.title(format!("Virtuous: {}", v.name()), 18.0, GOLD));
                        }
                        c.spawn(Node {
                            column_gap: Val::Px(4.0),
                            ..default()
                        })
                        .with_children(|row| {
                            for s in h.equipped_skills() {
                                let icon = kit.icon(s.icon);
                                row.spawn((
                                    ImageNode::new(icon),
                                    Node {
                                        width: Val::Px(40.0),
                                        height: Val::Px(40.0),
                                        ..default()
                                    },
                                    Interaction::default(),
                                    Tip(super::hamlet::skill_tooltip_ranked(s, h.rank(s.id))),
                                ));
                            }
                            for t in h
                                .trinkets
                                .iter()
                                .flatten()
                                .filter_map(|t| crate::rules::items::trinket(t))
                            {
                                let icon = kit.icon(t.icon);
                                let (r, g, b) = t.rarity.color();
                                row.spawn((
                                    ImageNode::new(icon),
                                    Node {
                                        width: Val::Px(40.0),
                                        height: Val::Px(40.0),
                                        border: UiRect::all(Val::Px(2.0)),
                                        margin: UiRect::left(Val::Px(6.0)),
                                        ..default()
                                    },
                                    BorderColor::all(Color::srgb(r, g, b)),
                                    Interaction::default(),
                                    Tip(t.summary()),
                                ));
                            }
                        });
                    }
                });
            // Pack.
            bottom
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Px(330.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|c| {
                    c.spawn(kit.title("Pack", 20.0, BONE));
                    c.spawn(Node {
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(4.0),
                        row_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|row| {
                        for item in Item::ALL {
                            let n = r.count(item);
                            if n == 0 {
                                continue;
                            }
                            let usable = matches!(
                                item,
                                Item::Torch
                                    | Item::Food
                                    | Item::Bandage
                                    | Item::Herbs
                                    | Item::HolyWater
                            );
                            let icon = kit.icon(item.icon());
                            row.spawn(Node {
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                ..default()
                            })
                            .with_children(|cell| {
                                ui::icon_button(
                                    cell,
                                    icon,
                                    44.0,
                                    Act::UseItem(item),
                                    usable,
                                    false,
                                    Some(format!(
                                        "{} ({n})\n{}{}",
                                        item.name(),
                                        item.desc(),
                                        if usable {
                                            "\nClick to use on the selected hero."
                                        } else {
                                            ""
                                        }
                                    )),
                                );
                                cell.spawn(kit.text(format!("{n}"), 13.0, BONE));
                            });
                        }
                    });
                });
            // Actions.
            bottom
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|c| {
                    match &ex.mode {
                        Mode::Curio(id) => {
                            if let Some(cd) = curio(id) {
                                c.spawn(kit.title(cd.name, 22.0, GOLD));
                                c.spawn(kit.text(cd.desc, 14.0, DIM));
                                c.spawn(Node {
                                    column_gap: Val::Px(6.0),
                                    flex_wrap: FlexWrap::Wrap,
                                    row_gap: Val::Px(4.0),
                                    ..default()
                                })
                                .with_children(|row| {
                                    ui::button(
                                        row,
                                        &kit,
                                        "Investigate",
                                        Act::Investigate(None),
                                        true,
                                    );
                                    for item in Item::ALL {
                                        if r.count(item) > 0
                                            && !matches!(item, Item::Torch | Item::Food)
                                        {
                                            ui::button(
                                                row,
                                                &kit,
                                                &format!("Use {}", item.name()),
                                                Act::Investigate(Some(item)),
                                                true,
                                            );
                                        }
                                    }
                                    ui::button(row, &kit, "Leave it", Act::LeaveCurio, true);
                                });
                            }
                        }
                        Mode::Obstacle => {
                            c.spawn(kit.title("Rubble", 22.0, GOLD));
                            let label = if r.count(Item::Shovel) > 0 {
                                "Dig through (Shovel)"
                            } else {
                                "Claw through by hand (stress, light)"
                            };
                            ui::button(c, &kit, label, Act::Clear, true);
                        }
                        Mode::Walking | Mode::Idle => {
                            if let Location::Hall { .. } = r.dungeon.at {
                                c.spawn(Node {
                                    column_gap: Val::Px(6.0),
                                    ..default()
                                })
                                .with_children(|row| {
                                    ui::button(
                                        row,
                                        &kit,
                                        if ex.mode == Mode::Walking {
                                            "Halt (Space)"
                                        } else {
                                            "Advance (Space)"
                                        },
                                        Act::Walk,
                                        true,
                                    );
                                    ui::button(row, &kit, "Turn Back", Act::TurnBack, true);
                                });
                            } else {
                                c.spawn(kit.text("Choose a passage on the map.", 15.0, DIM));
                                if r.firewood > 0 {
                                    ui::button(
                                        c,
                                        &kit,
                                        "Make Camp (firewood)",
                                        Act::Camp,
                                        r.can_camp(),
                                    );
                                }
                            }
                        }
                        Mode::Camp => {
                            c.spawn(kit.title("Camp", 22.0, GOLD));
                            c.spawn(kit.text("Plan the night above, then rest.", 14.0, DIM));
                        }
                        Mode::Battle => {}
                    }
                    c.spawn(Node {
                        flex_grow: 1.0,
                        ..default()
                    });
                    let done = r.dungeon.quest_done();
                    ui::button(
                        c,
                        &kit,
                        if done {
                            "Return Home in Triumph"
                        } else {
                            "Abandon the Expedition"
                        },
                        Act::Leave,
                        matches!(ex.mode, Mode::Idle | Mode::Walking),
                    );
                });
            // Map.
            map(bottom, &kit, r, matches!(ex.mode, Mode::Idle));
        });
    if ex.mode == Mode::Camp {
        camp_panel(&mut commands, &mut kit, &ex, r);
    }
}

/// The camp planner: each hero's camp skills, the hours left, and the rest button.
fn camp_panel(
    commands: &mut Commands,
    kit: &mut Kit,
    ex: &Explore,
    r: &crate::rules::dungeon::Expedition,
) {
    use crate::rules::camp::{camp_skill, known, CampTarget, CAMP_HOURS};
    let used: u8 = ex
        .camp
        .iter()
        .filter_map(|(_, id, _)| camp_skill(id))
        .map(|s| s.hours)
        .sum();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(110.0),
                left: Val::Percent(18.0),
                width: Val::Percent(64.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(ui::PANEL),
            BorderColor::all(EDGE),
            GlobalZIndex(20),
            HudRoot,
            Scoped(Screen::Dungeon),
        ))
        .with_children(|p| {
            p.spawn(kit.title(format!("The Night's Camp — {} of {CAMP_HOURS} hours left", CAMP_HOURS - used), 26.0, GOLD));
            p.spawn(kit.text(
                "Each hero may use each of their camp skills once. Skills on an ally target the hero selected on the stage. The party eats on resting.",
                13.0,
                DIM,
            ));
            for (i, h) in r.party.iter().enumerate() {
                p.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|row| {
                    row.spawn((kit.title(h.name.clone(), 17.0, if i == ex.selected { GOLD } else { BONE }), Node { width: Val::Px(120.0), ..default() }));
                    for cs in known(h.class) {
                        let done = ex.camp.iter().any(|(hh, id, _)| *hh == i && *id == cs.id);
                        let fits = used + cs.hours <= CAMP_HOURS;
                        let icon = kit.icon(cs.icon);
                        let tgt = match cs.target {
                            CampTarget::Myself => "self",
                            CampTarget::Ally => "an ally",
                            CampTarget::Party => "the party",
                        };
                        ui::icon_button(row, icon, 40.0, Act::CampUse(i, cs.id), !done && fits, done, Some(format!("{} ({}h, {tgt})\n{}", cs.name, cs.hours, cs.desc)));
                    }
                });
            }
            let plan: Vec<String> = ex
                .camp
                .iter()
                .filter_map(|(h, id, t)| {
                    let s = camp_skill(id)?;
                    let who = r.party.get(*h)?.name.clone();
                    let on = t.and_then(|t| r.party.get(t)).map_or(String::new(), |x| format!(" on {}", x.name));
                    Some(format!("{who}: {}{on}", s.name))
                })
                .collect();
            p.spawn(kit.text(if plan.is_empty() { "Nothing planned.".to_owned() } else { plan.join("  ·  ") }, 14.0, BONE));
            p.spawn(Node { column_gap: Val::Px(10.0), ..default() }).with_children(|row| {
                ui::button(row, kit, "Undo", Act::CampUndo, !ex.camp.is_empty());
                ui::button_sized(row, kit, "Rest Until Morning", Act::CampRest, true, false, 20.0);
            });
        });
}

fn map(
    p: &mut ChildSpawnerCommands,
    kit: &Kit,
    r: &crate::rules::dungeon::Expedition,
    can_go: bool,
) {
    let d = &r.dungeon;
    let (minx, maxx) = d.rooms.iter().fold((i32::MAX, i32::MIN), |(a, b), rm| {
        (a.min(rm.grid.0), b.max(rm.grid.0))
    });
    let (miny, maxy) = d.rooms.iter().fold((i32::MAX, i32::MIN), |(a, b), rm| {
        (a.min(rm.grid.1), b.max(rm.grid.1))
    });
    let cell = 34.0_f32
        .min(170.0 / ((maxy - miny + 1) as f32))
        .min(420.0 / ((maxx - minx + 1) as f32));
    let room_px = cell * 0.5;
    let w = (maxx - minx + 1) as f32 * cell;
    let h = (maxy - miny + 1) as f32 * cell;
    let here_room = match d.at {
        Location::Room(i) => Some(i),
        Location::Hall { .. } => None,
    };
    let exits: Vec<(usize, usize)> = here_room.map(|i| d.exits(i)).unwrap_or_default();
    p.spawn((Node { width: Val::Px(w + 8.0), height: Val::Px(h + 8.0), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5))))
        .with_children(|m| {
            let center = |g: (i32, i32)| -> Vec2 {
                Vec2::new((g.0 - minx) as f32 * cell + cell / 2.0 + 4.0, (g.1 - miny) as f32 * cell + cell / 2.0 + 4.0)
            };
            for (hi, hall) in d.halls.iter().enumerate() {
                let a = &d.rooms[hall.a];
                let b = &d.rooms[hall.b];
                if !(a.scouted || b.scouted) {
                    continue;
                }
                let (ca, cb) = (center(a.grid), center(b.grid));
                let n = hall.segs.len();
                for s in 0..n {
                    let t = (s as f32 + 1.0) / (n as f32 + 1.0);
                    let pos = ca.lerp(cb, t);
                    let here = matches!(d.at, Location::Hall { hall: hh, seg, toward } if hh == hi && seg != HALL_MOUTH && d.seg_index(hh, seg, toward) == s);
                    let seen = hall.visited[s] || hall.scouted.get(s).copied().unwrap_or(false);
                    let color = if here {
                        GOLD
                    } else if seen {
                        match hall.segs[s] {
                            Seg::Empty => Color::srgb(0.35, 0.3, 0.25),
                            Seg::Battle(_) => BLOOD,
                            Seg::Curio(_) => Color::srgb(0.5, 0.4, 0.8),
                            _ => Color::srgb(0.6, 0.5, 0.3),
                        }
                    } else {
                        Color::srgb(0.16, 0.14, 0.12)
                    };
                    let sz = cell * 0.16;
                    m.spawn((
                        Node { position_type: PositionType::Absolute, left: Val::Px(pos.x - sz / 2.0), top: Val::Px(pos.y - sz / 2.0), width: Val::Px(sz), height: Val::Px(sz), ..default() },
                        BackgroundColor(color),
                    ));
                }
            }
            for (i, room) in d.rooms.iter().enumerate() {
                if !room.scouted {
                    continue;
                }
                let c = center(room.grid);
                let exit = exits.iter().find(|(_, to)| *to == i).map(|(h, _)| *h);
                let color = if Some(i) == here_room {
                    GOLD
                } else if !room.visited {
                    Color::srgb(0.2, 0.18, 0.16)
                } else {
                    match (&room.kind, room.cleared) {
                        (RoomKind::Boss(_), false) => Color::srgb(0.6, 0.0, 0.0),
                        (RoomKind::Battle(_), false) => BLOOD,
                        (RoomKind::Curio(_), false) => Color::srgb(0.5, 0.4, 0.8),
                        (RoomKind::Entrance, _) => Color::srgb(0.3, 0.5, 0.3),
                        _ => Color::srgb(0.42, 0.37, 0.3),
                    }
                };
                let mut e = m.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(c.x - room_px / 2.0),
                        top: Val::Px(c.y - room_px / 2.0),
                        width: Val::Px(room_px),
                        height: Val::Px(room_px),
                        border: UiRect::all(Val::Px(if exit.is_some() && can_go { 2.0 } else { 1.0 })),
                        ..default()
                    },
                    BackgroundColor(color),
                    BorderColor::all(if exit.is_some() && can_go { BONE } else { Color::BLACK }),
                ));
                if let (Some(h), true) = (exit, can_go) {
                    e.insert((Button, Act::Go(h), Tip(format!("Travel to this {}", if room.visited { "room" } else { "unexplored room" }))));
                }
            }
        });
    let _ = kit;
}

fn portrait_frame(p: &mut ChildSpawnerCommands, image: Handle<Image>, size: f32) {
    p.spawn((
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            border: UiRect::all(Val::Px(3.0)),
            flex_shrink: 0.0,
            ..default()
        },
        BorderColor::all(super::ui::GOLD),
        BackgroundColor(Color::BLACK),
    ))
    .with_children(|f| {
        f.spawn((
            ImageNode::new(image),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ));
    });
}

/// `CRAWL_AUTOPLAY=1`: walk the crypt unattended (the battle side picks its own skills).
fn autoplay(time: Res<Time>, mut auto: ResMut<AutoQueue>, ex: Res<Explore>, run: Res<Run>) {
    if std::env::var("CRAWL_AUTOPLAY").is_err() || !auto.0.is_empty() {
        return;
    }
    auto.1 -= time.delta_secs();
    if auto.1 > 0.0 {
        return;
    }
    auto.1 = 1.2;
    let r = &run.0;
    match &ex.mode {
        Mode::Curio(_) => auto.0.push(Act::Investigate(None)),
        Mode::Camp => {
            if ex.camp.is_empty() {
                if let Some(h) = r.party.first() {
                    if let Some(cs) = crate::rules::camp::known(h.class)
                        .into_iter()
                        .find(|c| c.class.is_some())
                    {
                        auto.0.push(Act::CampUse(0, cs.id));
                        return;
                    }
                }
            }
            auto.0.push(Act::CampRest);
        }
        Mode::Obstacle => auto.0.push(Act::Clear),
        Mode::Idle => {
            if r.light < 40 && r.count(Item::Torch) > 0 {
                auto.0.push(Act::UseItem(Item::Torch));
                return;
            }
            match r.dungeon.at {
                Location::Hall { .. } => auto.0.push(Act::Walk),
                Location::Room(room) => {
                    if r.dungeon.quest_done() {
                        auto.0.push(Act::Leave);
                        return;
                    }
                    let avg =
                        r.party.iter().map(|h| h.stress).sum::<i32>() / r.party.len().max(1) as i32;
                    if r.can_camp() && avg > 35 {
                        auto.0.push(Act::Camp);
                        return;
                    }
                    let exits = r.dungeon.exits(room);
                    let fresh = exits.iter().find(|(_, to)| !r.dungeon.rooms[*to].visited);
                    let pick =
                        fresh.or_else(|| exits.get(r.dungeon.steps as usize % exits.len().max(1)));
                    if let Some(&(hall, _)) = pick {
                        auto.0.push(Act::Go(hall));
                    }
                }
            }
        }
        Mode::Walking | Mode::Battle => {}
    }
}
