//! A battle on the stage: the rules' event stream replayed as lunges, wounds, floating numbers and
//! barks, and the skill bar a hero takes orders from.

use std::collections::{HashMap, VecDeque};

use bevy::prelude::*;

use super::explore::{resume_mode, Explore, Mode};
use super::stage::{self, Actor, ActorView, Mark, Plate, Walk};
use super::ui::{self, Kit, Narrator, Tip, BLOOD, BONE, DIM, EDGE, GOLD, HEAL, STRESS};
use super::{Run, Scoped, Screen};
use crate::rules::combat::{
    Battle, DotKind, Event, Order, Outcome, Side, StatusKind, Step, UnitId, UnitKind,
};
use crate::rules::data::{anim, SkillKind, Stat};
use crate::rules::dungeon::{monsters_of, Location};
use crate::rules::rng::Rng;
use crate::wow::{ModelSource, PlayAnim};

/// A fight waiting to start, set by exploration.
#[derive(Resource)]
pub struct PendingBattle {
    pub pack: Vec<String>,
    pub boss: bool,
}

/// The order a selected button stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
    Skill(usize),
    Move,
}

#[derive(Resource)]
pub struct BattleView {
    pub battle: Battle,
    actors: HashMap<UnitId, Entity>,
    queue: VecDeque<Event>,
    wait: f32,
    pending_turn: Option<UnitId>,
    awaiting: Option<UnitId>,
    choice: Option<Choice>,
    dirty: bool,
    over: Option<Outcome>,
    boss: bool,
    /// Floaters stacked on a unit since the last skill, to stagger them.
    stack: HashMap<UnitId, u32>,
    /// The skill being played out, for its impact sounds.
    last: Option<(UnitId, &'static crate::rules::data::SkillDef)>,
    beat: usize,
}

#[derive(Component, Clone, Copy)]
enum Act {
    Choose(Choice),
    Pass,
    Retreat,
}

#[derive(Component)]
struct BattleHud;

/// A dying actor fading off the stage.
#[derive(Component)]
struct Fade(f32);

pub struct BattlePlugin;

impl Plugin for BattlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (start, act, keys, plate_targets, run, sync_views, hud, fade)
                .chain()
                .run_if(in_state(Screen::Dungeon)),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn start(
    mut commands: Commands,
    pending: Option<Res<PendingBattle>>,
    run: Option<ResMut<Run>>,
    mut heroes: Query<(Entity, &mut Actor)>,
    mut narrator: ResMut<Narrator>,
    mut walk: ResMut<Walk>,
    ex: Option<ResMut<Explore>>,
    mut music: ResMut<crate::audio::Music>,
) {
    let (Some(p), Some(mut run), Some(mut ex)) = (pending, run, ex) else {
        return;
    };
    commands.remove_resource::<PendingBattle>();
    ex.mode = Mode::Battle;
    ex.dirty = true;
    let pack = monsters_of(&p.pack);
    let seed = run.0.rng.next_u64();
    let mut battle = Battle::new(run.0.party.clone(), &pack, run.0.light, Rng::new(seed));
    // A curio's blessing carries into the first fight after it.
    for (stat, amt) in std::mem::take(&mut run.0.buffs) {
        for &h in &battle.heroes.clone() {
            battle.units[h].statuses.push(crate::rules::combat::Status {
                kind: StatusKind::Mod(stat, amt),
                turns: 5,
            });
        }
    }
    let mut actors = HashMap::new();
    for &uid in &battle.heroes {
        let UnitKind::Hero(h) = &battle.units[uid].kind else {
            continue;
        };
        if let Some((e, mut a)) = heroes.iter_mut().find(|(_, a)| a.hero == Some(h.id)) {
            a.unit = Some(uid);
            actors.insert(uid, e);
        }
    }
    for &uid in &battle.monsters {
        let u = &battle.units[uid];
        let m = u.monster().expect("monster line");
        let ranks = battle.ranks_of(uid);
        let centre = ranks.iter().sum::<usize>() as f32 / ranks.len().max(1) as f32;
        let home = stage::slot(Side::Monsters, centre);
        let e = stage::spawn_actor(
            &mut commands,
            Actor {
                side: Side::Monsters,
                hero: None,
                unit: Some(uid),
                home,
                lunge: 0.0,
                alive: true,
            },
            ModelSource::Creature(m.display),
            m.height,
            ActorView {
                name: m.name.to_owned(),
                hp: u.hp,
                max_hp: u.max_hp,
                ..default()
            },
            home + Vec3::X * 9.0,
        );
        commands.entity(e).insert(PlayAnim::looped(anim::READY_1H));
        actors.insert(uid, e);
    }
    narrator.say(if p.boss {
        "The Archivist closes its book. It has read how this ends."
    } else {
        *Rng::new(seed)
            .pick(&[
                "Ambush! Steel and bone in the dark!",
                "They were waiting. Of course they were waiting.",
                "The dead rise to defend their rest.",
                "Weapons up. The crypt has noticed you.",
            ])
            .expect("lines")
    });
    walk.zoom_target = 0.0;
    for &uid in &battle.heroes {
        if let Some(e) = actors.get(&uid) {
            commands.entity(*e).insert(PlayAnim::looped(anim::READY_1H));
        }
    }
    commands.insert_resource(BattleView {
        battle,
        actors,
        queue: VecDeque::new(),
        wait: 1.0,
        pending_turn: None,
        awaiting: None,
        choice: None,
        dirty: true,
        over: None,
        boss: p.boss,
        stack: HashMap::new(),
        last: None,
        beat: 0,
    });
    *music = if p.boss {
        crate::audio::Music::Boss
    } else {
        crate::audio::Music::Battle
    };
}

fn first_usable(b: &Battle, id: UnitId) -> Option<Choice> {
    let skills = b.units[id].skills();
    (0..skills.len())
        .find(|&i| b.can_use(id, skills[i]))
        .map(Choice::Skill)
}

fn valid_targets(b: &Battle, id: UnitId, choice: Choice) -> Vec<UnitId> {
    match choice {
        Choice::Skill(i) => {
            let skills = b.units[id].skills();
            match skills.get(i) {
                Some(s) if b.can_use(id, s) => b.targets_for(id, s),
                _ => Vec::new(),
            }
        }
        Choice::Move => {
            let r = b.rank_of(id);
            b.heroes
                .iter()
                .copied()
                .filter(|&h| h != id && b.rank_of(h).abs_diff(r) == 1 && b.units[h].alive)
                .collect()
        }
    }
}

fn drain_events(bv: &mut BattleView) {
    let events = std::mem::take(&mut bv.battle.events);
    bv.queue.extend(events);
}

fn issue(bv: &mut BattleView, id: UnitId, choice: Choice, target: UnitId) -> bool {
    let order = match choice {
        Choice::Skill(i) => Order::Skill { skill: i, target },
        Choice::Move => Order::Move {
            to_rank: bv.battle.rank_of(target),
        },
    };
    if bv.battle.order(id, order) {
        bv.awaiting = None;
        bv.choice = None;
        bv.dirty = true;
        drain_events(bv);
        if let Some(o) = bv.battle.outcome {
            bv.over = Some(o);
        }
        true
    } else {
        false
    }
}

fn act(
    q: Query<(&Interaction, &Act, Option<&ui::BtnLook>), Changed<Interaction>>,
    bv: Option<ResMut<BattleView>>,
) {
    let Some(mut bv) = bv else { return };
    let Some(id) = bv.awaiting else { return };
    for (i, a, look) in &q {
        if *i != Interaction::Pressed || look.is_some_and(|l| !l.enabled) {
            continue;
        }
        match *a {
            Act::Choose(c) => {
                bv.choice = Some(c);
                bv.dirty = true;
                // A skill with no choice of target goes off at once.
                if let Choice::Skill(si) = c {
                    let skills = bv.battle.units[id].skills();
                    if let Some(s) = skills.get(si) {
                        use crate::rules::data::Target;
                        if matches!(s.target, Target::SelfOnly | Target::AllyAll) {
                            issue(&mut bv, id, c, id);
                        }
                    }
                }
            }
            Act::Pass => {
                if bv.battle.order(id, Order::Pass) {
                    bv.awaiting = None;
                    bv.dirty = true;
                    drain_events(&mut bv);
                }
            }
            Act::Retreat => {
                if bv.battle.order(id, Order::Retreat) {
                    bv.awaiting = None;
                    bv.dirty = true;
                    drain_events(&mut bv);
                    if let Some(o) = bv.battle.outcome {
                        bv.over = Some(o);
                    }
                }
            }
        }
    }
}

fn keys(keys: Res<ButtonInput<KeyCode>>, bv: Option<ResMut<BattleView>>) {
    let Some(mut bv) = bv else { return };
    let Some(id) = bv.awaiting else { return };
    for (i, k) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ]
    .iter()
    .enumerate()
    {
        if keys.just_pressed(*k) {
            let skills = bv.battle.units[id].skills();
            if skills.get(i).is_some_and(|s| bv.battle.can_use(id, s)) {
                bv.choice = Some(Choice::Skill(i));
                bv.dirty = true;
            }
        }
    }
    if keys.just_pressed(KeyCode::KeyM) {
        bv.choice = Some(Choice::Move);
        bv.dirty = true;
    }
}

#[allow(clippy::type_complexity)]
fn plate_targets(
    mut commands: Commands,
    q: Query<(Entity, &Interaction, &Plate)>,
    changed: Query<(), Changed<Interaction>>,
    actors: Query<&Actor>,
    bv: Option<ResMut<BattleView>>,
) {
    let Some(mut bv) = bv else { return };
    let (Some(id), Some(choice)) = (bv.awaiting, bv.choice) else {
        for (e, _, _) in &q {
            commands.entity(e).remove::<Tip>();
        }
        return;
    };
    let targets = valid_targets(&bv.battle, id, choice);
    for (e, i, p) in &q {
        let Ok(a) = actors.get(p.actor) else { continue };
        let Some(uid) = a.unit else { continue };
        if !targets.contains(&uid) {
            commands.entity(e).remove::<Tip>();
            continue;
        }
        // The odds, on hover.
        let b = &bv.battle;
        let tip = match choice {
            Choice::Skill(si) => {
                let s = b.units[id].skills()[si];
                if s.target.is_enemy() {
                    let (lo, hi) = b.damage_preview(id, s, uid);
                    let dmg = if s.dmg_mod <= -100 {
                        "—".to_owned()
                    } else {
                        format!("{lo}–{hi}")
                    };
                    format!(
                        "{}\nHit {}%   Crit {}%   Damage {dmg}",
                        b.units[uid].name(),
                        b.hit_chance(id, s, uid),
                        b.crit_chance(id, s)
                    )
                } else {
                    format!("{} on {}", s.name, b.units[uid].name())
                }
            }
            Choice::Move => format!("Swap places with {}", b.units[uid].name()),
        };
        commands.entity(e).insert(Tip(tip));
        if *i == Interaction::Pressed && changed.get(e).is_ok() {
            issue(&mut bv, id, choice, uid);
            return;
        }
    }
}

fn layout(bv: &BattleView, actors: &mut Query<&mut Actor>) {
    for side in [Side::Heroes, Side::Monsters] {
        for &uid in bv.battle.line(side) {
            let ranks = bv.battle.ranks_of(uid);
            if ranks.is_empty() {
                continue;
            }
            let centre = ranks.iter().sum::<usize>() as f32 / ranks.len() as f32;
            if let Some(&e) = bv.actors.get(&uid) {
                if let Ok(mut a) = actors.get_mut(e) {
                    a.home = stage::slot(side, centre);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run(
    mut commands: Commands,
    time: Res<Time>,
    bv: Option<ResMut<BattleView>>,
    fonts: Res<ui::Fonts>,
    mut narrator: ResMut<Narrator>,
    mut walk: ResMut<Walk>,
    mut actors: Query<&mut Actor>,
    mut views: Query<&mut ActorView>,
    mut anims: Query<&mut PlayAnim>,
    xforms: Query<&GlobalTransform>,
    ex: Option<ResMut<Explore>>,
    runres: Option<ResMut<Run>>,
    mut sfx: ResMut<crate::audio::Sfx>,
    mut music: ResMut<crate::audio::Music>,
) {
    let Some(mut bv) = bv else { return };
    let (Some(mut ex), Some(mut runres)) = (ex, runres) else {
        return;
    };
    bv.wait -= time.delta_secs();
    if bv.wait > 0.0 {
        return;
    }
    if let Some(ev) = bv.queue.pop_front() {
        let wait = present(
            &mut sfx,
            &mut commands,
            &mut bv,
            &ev,
            &fonts,
            &mut narrator,
            &mut walk,
            &mut actors,
            &mut views,
            &mut anims,
            &xforms,
        );
        bv.wait = wait;
        return;
    }
    walk.zoom_target = 0.0;
    if let Some(id) = bv.awaiting {
        // `CRAWL_AUTOPLAY=1`: heroes pick their own orders, for unattended runs.
        if std::env::var("CRAWL_AUTOPLAY").is_ok() {
            let skills = bv.battle.units[id].skills();
            let mut picks: Vec<(usize, UnitId)> = Vec::new();
            for (i, s) in skills.iter().enumerate() {
                if bv.battle.can_use(id, s) {
                    for t in bv.battle.targets_for(id, s) {
                        picks.push((i, t));
                    }
                }
            }
            let roll = (bv.battle.round as usize * 7 + id * 13 + picks.len()) % picks.len().max(1);
            match picks.get(roll) {
                Some(&(i, t)) => {
                    issue(&mut bv, id, Choice::Skill(i), t);
                }
                None => {
                    if bv.battle.order(id, Order::Pass) {
                        bv.awaiting = None;
                        drain_events(&mut bv);
                    }
                }
            }
        }
        return;
    }
    if let Some(id) = bv.pending_turn.take() {
        bv.awaiting = Some(id);
        bv.choice = first_usable(&bv.battle, id);
        bv.dirty = true;
        return;
    }
    if let Some(o) = bv.over {
        *music = crate::audio::Music::Crypt;
        sfx.play(crate::audio::COIN, -6.0);
        finish(
            &mut commands,
            &mut bv,
            o,
            &mut narrator,
            &mut actors,
            &mut anims,
            &mut ex,
            &mut runres,
            &mut walk,
        );
        return;
    }
    let step = bv.battle.step();
    let events = std::mem::take(&mut bv.battle.events);
    bv.queue.extend(events);
    match step {
        Step::HeroTurn(id) => bv.pending_turn = Some(id),
        Step::Acted => {}
        Step::Over(o) => bv.over = Some(o),
    }
}

fn pos_of(bv: &BattleView, uid: UnitId, xforms: &Query<&GlobalTransform>) -> Vec3 {
    bv.actors
        .get(&uid)
        .and_then(|e| xforms.get(*e).ok())
        .map_or(Vec3::ZERO, |t| t.translation())
}

/// Play one event on the stage; returns how long to dwell on it.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn present(
    sfx: &mut crate::audio::Sfx,
    commands: &mut Commands,
    bv: &mut BattleView,
    ev: &Event,
    fonts: &ui::Fonts,
    narrator: &mut Narrator,
    walk: &mut Walk,
    actors: &mut Query<&mut Actor>,
    views: &mut Query<&mut ActorView>,
    anims: &mut Query<&mut PlayAnim>,
    xforms: &Query<&GlobalTransform>,
) -> f32 {
    let float = |commands: &mut Commands,
                 bv: &mut BattleView,
                 uid: UnitId,
                 s: String,
                 color: Color,
                 size: f32| {
        let n = bv.stack.get(&uid).copied().unwrap_or(0);
        let p = pos_of(bv, uid, xforms) + Vec3::Y * (2.3 + n as f32 * 0.42);
        bv.stack.insert(uid, n + 1);
        ui::float_text(commands, fonts, p, s, color, size);
    };
    let play = |anims: &mut Query<&mut PlayAnim>, bv: &BattleView, uid: UnitId, a: PlayAnim| {
        if let Some(e) = bv.actors.get(&uid) {
            if let Ok(mut p) = anims.get_mut(*e) {
                *p = a;
            }
        }
    };
    let view = |views: &mut Query<&mut ActorView>,
                bv: &BattleView,
                uid: UnitId,
                f: &mut dyn FnMut(&mut ActorView)| {
        if let Some(e) = bv.actors.get(&uid) {
            if let Ok(mut v) = views.get_mut(*e) {
                f(&mut v);
            }
        }
    };
    match ev {
        Event::RoundStart(_) => 0.05,
        Event::TurnStart(_) => 0.2,
        Event::Skill {
            actor,
            skill,
            targets,
        } => {
            bv.stack.clear();
            bv.last = Some((*actor, *skill));
            bv.beat += 1;
            if let Some(m) = bv.battle.units[*actor].monster() {
                let s = crate::audio::monster_sound(m.id, crate::audio::Voice::Attack, bv.beat);
                sfx.play(s, -4.0);
            } else if !matches!(skill.kind, SkillKind::Melee | SkillKind::Ranged) {
                sfx.play(
                    crate::audio::skill_sound(skill.id, skill.kind, false, bv.beat),
                    -6.0,
                );
            }
            let melee = skill.kind == SkillKind::Melee;
            if let Some(e) = bv.actors.get(actor) {
                if let Ok(mut a) = actors.get_mut(*e) {
                    a.lunge = if melee { 1.4 } else { 0.45 };
                }
            }
            let ready = anim::READY_1H;
            play(anims, bv, *actor, PlayAnim::once_then(skill.anim, ready));
            float(commands, bv, *actor, skill.name.to_owned(), GOLD, 22.0);
            walk.zoom_target = if targets.len() == 1 { 0.45 } else { 0.3 };
            for t in targets {
                bv.stack.entry(*t).or_insert(0);
            }
            0.75
        }
        Event::Miss(t) => {
            sfx.play(crate::audio::MISS, -6.0);
            float(commands, bv, *t, "Miss".into(), BONE, 24.0);
            play(
                anims,
                bv,
                *t,
                PlayAnim::once_then(anim::PARRY_UNARMED, anim::READY_1H),
            );
            0.4
        }
        Event::Damage {
            target,
            amount,
            crit,
        } => {
            let a = *amount;
            view(views, bv, *target, &mut |v| v.hp = (v.hp - a).max(0));
            bv.beat += 1;
            if let Some((_, s)) = bv.last {
                let physical = matches!(s.kind, SkillKind::Melee | SkillKind::Ranged);
                if physical || bv.battle.units[*target].side == Side::Heroes {
                    sfx.play(
                        crate::audio::skill_sound(s.id, s.kind, *crit, bv.beat),
                        -3.0,
                    );
                }
            }
            if let Some(m) = bv.battle.units[*target].monster() {
                if bv.battle.units[*target].alive {
                    let s = crate::audio::monster_sound(m.id, crate::audio::Voice::Wound, bv.beat);
                    sfx.play(s, -7.0);
                }
            }
            if *crit {
                float(
                    commands,
                    bv,
                    *target,
                    format!("CRIT! {a}"),
                    Color::srgb(1.0, 0.75, 0.1),
                    38.0,
                );
                walk.shake = 1.0;
            } else {
                float(commands, bv, *target, format!("{a}"), BLOOD, 30.0);
                walk.shake = walk.shake.max(0.3);
            }
            if bv.battle.units[*target].alive && !bv.battle.units[*target].corpse {
                play(
                    anims,
                    bv,
                    *target,
                    PlayAnim::once_then(anim::WOUND, anim::READY_1H),
                );
            }
            0.5
        }
        Event::Heal {
            target,
            amount,
            crit,
        } => {
            sfx.play("Sound\\Spells\\Heal_Low_Base.wav", -8.0);
            let a = *amount;
            view(views, bv, *target, &mut |v| {
                v.hp = (v.hp + a).min(v.max_hp);
                v.deaths_door = false;
            });
            float(
                commands,
                bv,
                *target,
                if *crit {
                    format!("CRIT! +{a}")
                } else {
                    format!("+{a}")
                },
                HEAL,
                if *crit { 34.0 } else { 28.0 },
            );
            0.45
        }
        Event::Status { target, label } => {
            float(commands, bv, *target, label.clone(), GOLD, 18.0);
            0.3
        }
        Event::Resisted { target, label } => {
            float(
                commands,
                bv,
                *target,
                format!("{label} Resisted"),
                DIM,
                18.0,
            );
            0.3
        }
        Event::Tick {
            target,
            amount,
            kind,
        } => {
            let a = *amount;
            view(views, bv, *target, &mut |v| v.hp = (v.hp - a).max(0));
            let (c, s) = match kind {
                DotKind::Bleed => (BLOOD, "Bleed"),
                DotKind::Blight => (Color::srgb(0.55, 0.75, 0.2), "Blight"),
            };
            float(commands, bv, *target, format!("{s} {a}"), c, 24.0);
            play(
                anims,
                bv,
                *target,
                PlayAnim::once_then(anim::WOUND, anim::READY_1H),
            );
            0.5
        }
        Event::Stress { target, amount } => {
            let a = *amount;
            view(views, bv, *target, &mut |v| {
                v.stress = v.stress.map(|s| (s + a).clamp(0, 200))
            });
            if a > 0 {
                float(commands, bv, *target, format!("+{a} Stress"), STRESS, 22.0);
            } else {
                float(
                    commands,
                    bv,
                    *target,
                    format!("{a} Stress"),
                    Color::srgb(0.6, 0.85, 1.0),
                    20.0,
                );
            }
            0.35
        }
        Event::DeathsDoor(t) => {
            view(views, bv, *t, &mut |v| {
                v.hp = 0;
                v.deaths_door = true;
            });
            float(commands, bv, *t, "DEATH'S DOOR".into(), BLOOD, 32.0);
            narrator.say(
                *Rng::new(*t as u64 + bv.battle.round as u64)
                    .pick(&[
                        "Teetering on the brink, staring into the abyss.",
                        "One more blow. That is all it would take.",
                        "The light dims in their eyes, but does not go out.",
                    ])
                    .expect("lines"),
            );
            walk.shake = 0.8;
            0.8
        }
        Event::DeathBlowResisted(t) => {
            float(commands, bv, *t, "Death Blow Resisted!".into(), GOLD, 26.0);
            narrator.say("Not today. Not like this.");
            0.7
        }
        Event::Died(t) => {
            if let Some(m) = bv.battle.units[*t].monster() {
                sfx.play(
                    crate::audio::monster_sound(m.id, crate::audio::Voice::Death, 0),
                    -3.0,
                );
            }
            play(anims, bv, *t, PlayAnim::hold(anim::DEATH));
            let hero = bv.battle.units[*t].side == Side::Heroes;
            if hero {
                narrator.say(format!(
                    "{} is dead. The crypt keeps what it takes.",
                    bv.battle.units[*t].name()
                ));
                walk.shake = 1.2;
            }
            if let Some(e) = bv.actors.get(t) {
                if let Ok(mut a) = actors.get_mut(*e) {
                    a.alive = false;
                }
                // A corpse event follows if the remains stay; otherwise it fades.
                if !bv.battle.units[*t].corpse {
                    commands.entity(*e).insert(Fade(1.6));
                }
            }
            view(views, bv, *t, &mut |v| v.hp = 0);
            if hero {
                1.4
            } else {
                0.7
            }
        }
        Event::Corpse(t) => {
            let hp = bv.battle.units[*t].max_hp / 3;
            view(views, bv, *t, &mut |v| {
                v.status = "Corpse".into();
                v.hp = hp.max(1);
            });
            if let Some(e) = bv.actors.get(t) {
                commands.entity(*e).remove::<Fade>();
                if let Ok(mut a) = actors.get_mut(*e) {
                    a.alive = true;
                }
            }
            0.2
        }
        Event::CorpseCleared(t) => {
            if let Some(e) = bv.actors.get(t) {
                commands.entity(*e).insert(Fade(1.0));
                if let Ok(mut a) = actors.get_mut(*e) {
                    a.alive = false;
                }
            }
            0.3
        }
        Event::Moved => {
            layout(bv, actors);
            0.35
        }
        Event::Afflicted(t, a) => {
            sfx.play(crate::audio::AFFLICTION, -4.0);
            float(
                commands,
                bv,
                *t,
                format!("AFFLICTED: {}", a.name().to_uppercase()),
                BLOOD,
                34.0,
            );
            narrator.say(format!(
                "{}'s resolve is tested… {}.",
                bv.battle.units[*t].name(),
                a.name()
            ));
            walk.shake = 1.0;
            walk.zoom_target = 0.6;
            1.6
        }
        Event::Virtuous(t, v) => {
            sfx.play(crate::audio::VIRTUE, -4.0);
            float(
                commands,
                bv,
                *t,
                format!("VIRTUOUS: {}", v.name().to_uppercase()),
                GOLD,
                34.0,
            );
            narrator.say(format!(
                "{}'s resolve is tested… {}!",
                bv.battle.units[*t].name(),
                v.name()
            ));
            walk.zoom_target = 0.6;
            1.6
        }
        Event::HeartAttack(t) => {
            float(commands, bv, *t, "HEART ATTACK".into(), BLOOD, 36.0);
            narrator.say("The heart can only bear so much.");
            walk.shake = 1.4;
            1.2
        }
        Event::Stunned(t) => {
            float(commands, bv, *t, "Stunned".into(), BONE, 22.0);
            play(
                anims,
                bv,
                *t,
                PlayAnim::once_then(anim::STUN, anim::READY_1H),
            );
            0.6
        }
        Event::Bark(t, text) => {
            let p = pos_of(bv, *t, xforms) + Vec3::Y * 2.9;
            commands.spawn((
                Text::new(format!("\u{201c}{text}\u{201d}")),
                TextFont {
                    font: fonts.body.clone(),
                    font_size: 17.0,
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.92, 0.85)),
                TextLayout::new_with_justify(Justify::Center),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(240.0),
                    padding: UiRect::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
                GlobalZIndex(45),
                Pickable::IGNORE,
                ui::Floater {
                    world: p,
                    age: 0.0,
                    life: 3.0,
                },
            ));
            1.6
        }
        Event::Pass(t) => {
            float(commands, bv, *t, "…".into(), DIM, 26.0);
            0.35
        }
        Event::Retreat(ok) => {
            narrator.say(if *ok {
                "Retreat! Back, back into the dark!"
            } else {
                "There is no escape!"
            });
            0.6
        }
        Event::Over(_) => 0.4,
    }
}

#[allow(clippy::too_many_arguments)]
fn finish(
    commands: &mut Commands,
    bv: &mut BattleView,
    o: Outcome,
    narrator: &mut Narrator,
    actors: &mut Query<&mut Actor>,
    anims: &mut Query<&mut PlayAnim>,
    ex: &mut Explore,
    run: &mut Run,
    walk: &mut Walk,
) {
    walk.zoom_target = 0.0;
    let battle = std::mem::replace(
        &mut bv.battle,
        Battle::new(Vec::new(), &[], 100, Rng::new(0)),
    );
    let boss = bv.boss;
    let (living, dead) = battle.into_heroes();
    run.0.party = living;
    run.0.fallen.extend(dead);
    // Release the hero actors from battle and stand them down; clear the monsters off.
    for (uid, e) in bv.actors.drain() {
        let _ = uid;
        if let Ok(mut a) = actors.get_mut(e) {
            if a.side == Side::Monsters {
                commands.entity(e).insert(Fade(0.8));
            } else {
                a.unit = None;
                if !a.alive {
                    commands.entity(e).insert(Fade(1.0));
                }
            }
        }
        if let Ok(mut p) = anims.get_mut(e) {
            if p.id != anim::DEATH {
                *p = PlayAnim::looped(anim::STAND);
            }
        }
    }
    // Re-rank the surviving heroes in their battle order.
    for mut a in actors.iter_mut() {
        if let Some(id) = a.hero {
            if let Some(i) = run.0.party.iter().position(|h| h.id == id) {
                a.home = stage::slot(Side::Heroes, (i + 1) as f32);
            }
        }
    }
    match o {
        Outcome::Victory => {
            narrator.say(if boss {
                "The Archivist's last page turns. The crypt falls silent."
            } else {
                "The last of them falls. For now, the dark recedes."
            });
            let notes = run.0.battle_loot(boss);
            for n in &notes {
                match n {
                    crate::rules::dungeon::Note::Gold(g) => {
                        narrator.say(format!("Spoils: {g} gold."))
                    }
                    crate::rules::dungeon::Note::Heirloom(h, n) => {
                        narrator.say(format!("Spoils: {} × {n}.", h.name()))
                    }
                    _ => {}
                }
            }
            run.0.clear_here();
            ex.mode = resume_mode(run);
        }
        Outcome::Retreated => match run.0.dungeon.at {
            Location::Hall { .. } => {
                run.0.turn_back();
                ex.mode = resume_mode(run);
            }
            Location::Room(_) => {
                if let Some(h) = ex.came_from {
                    run.0.enter_hall(h);
                    ex.mode = Mode::Walking;
                } else {
                    ex.mode = Mode::Idle;
                }
            }
        },
        Outcome::Defeat => {
            narrator.say("The torch gutters out over the bodies. No one will come for them.");
            ex.mode = Mode::Idle;
        }
    }
    ex.dirty = true;
    commands.remove_resource::<BattleView>();
}

fn sync_views(bv: Option<Res<BattleView>>, mut views: Query<(&Actor, &mut ActorView)>) {
    let Some(bv) = bv else { return };
    let targets = match (bv.awaiting, bv.choice) {
        (Some(id), Some(c)) if bv.queue.is_empty() => valid_targets(&bv.battle, id, c),
        _ => Vec::new(),
    };
    let current = bv.battle.current.or(bv.awaiting);
    for (a, mut v) in &mut views {
        let Some(uid) = a.unit else { continue };
        // A battle just finished leaves its fading monsters pointing past the emptied roster.
        let Some(u) = bv.battle.units.get(uid) else {
            continue;
        };
        // Hit points and stress follow the replay; statuses and marks follow the rules' state.
        if bv.queue.is_empty() && bv.wait <= 0.0 {
            v.hp = u.hp;
            v.deaths_door = u.deaths_door;
            if let Some(h) = u.hero() {
                v.stress = Some(h.stress);
            }
        }
        let mut tags: Vec<String> = Vec::new();
        if u.corpse {
            tags.push("Corpse".into());
        }
        for s in &u.statuses {
            let t = match s.kind {
                StatusKind::Bleed(_) => "Bleed".to_owned(),
                StatusKind::Blight(_) => "Blight".to_owned(),
                StatusKind::Stun => "Stun".to_owned(),
                StatusKind::Mark => "Marked".to_owned(),
                StatusKind::GuardedBy(_) => "Guarded".to_owned(),
                StatusKind::Riposte => "Riposte".to_owned(),
                StatusKind::Taunt => "Taunt".to_owned(),
                StatusKind::Mod(stat, n) => {
                    format!("{}{}", if n > 0 { "+" } else { "-" }, stat.label())
                }
            };
            if !tags.contains(&t) {
                tags.push(t);
            }
        }
        if let Some(h) = u.hero() {
            if let Some(af) = h.affliction {
                tags.insert(0, af.name().to_owned());
            } else if let Some(vt) = h.virtue {
                tags.insert(0, vt.name().to_owned());
            }
        }
        v.status = tags.join(" ");
        v.mark = if targets.contains(&uid) {
            if u.side == Side::Heroes {
                Mark::Friend
            } else {
                Mark::Target
            }
        } else if current == Some(uid) {
            Mark::Active
        } else {
            Mark::None
        };
    }
}

#[allow(clippy::too_many_lines)]
fn hud(
    mut commands: Commands,
    mut kit: Kit,
    bv: Option<ResMut<BattleView>>,
    old: Query<Entity, With<BattleHud>>,
    mut portrait: ResMut<super::portrait::Portrait>,
) {
    let Some(mut bv) = bv else {
        for e in &old {
            commands.entity(e).despawn();
        }
        return;
    };
    if !bv.dirty {
        return;
    }
    bv.dirty = false;
    for e in &old {
        commands.entity(e).despawn();
    }
    let b = &bv.battle;
    if let Some(h) = bv.awaiting.and_then(|id| b.units[id].hero()) {
        portrait.want = Some(h.id);
    }
    let portrait_img = portrait.image.clone();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(190.0),
                padding: UiRect::all(Val::Px(10.0)),
                column_gap: Val::Px(20.0),
                border: UiRect::top(Val::Px(2.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(ui::PANEL),
            BorderColor::all(EDGE),
            BattleHud,
            Scoped(Screen::Dungeon),
        ))
        .with_children(|p| {
            let Some(id) = bv.awaiting else {
                p.spawn(kit.title(format!("Round {}", b.round.max(1)), 26.0, DIM));
                return;
            };
            let u = &b.units[id];
            let Some(h) = u.hero() else { return };
            portrait_frame(p, portrait_img.clone(), 170.0);
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                width: Val::Px(330.0),
                row_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|c| {
                c.spawn(kit.title(format!("{} the {}", h.name, h.class_name()), 26.0, GOLD));
                c.spawn(kit.text(
                    format!(
                        "HP {}/{}{}   Stress {}",
                        u.hp,
                        u.max_hp,
                        if u.deaths_door { " (Death's Door)" } else { "" },
                        h.stress
                    ),
                    15.0,
                    if u.deaths_door { BLOOD } else { BONE },
                ));
                c.spawn(kit.text(
                    format!(
                        "ACC {:+}  CRIT {}%  DODGE {}  PROT {}%  SPD {}",
                        b.stat(id, Stat::Acc),
                        b.stat(id, Stat::Crit),
                        b.stat(id, Stat::Dodge),
                        b.stat(id, Stat::Prot),
                        b.stat(id, Stat::Speed)
                    ),
                    13.0,
                    DIM,
                ));
                c.spawn(kit.text(
                    format!("Rank {} — Round {}", b.rank_of(id), b.round),
                    13.0,
                    DIM,
                ));
            });
            p.spawn(Node {
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                for (i, s) in u.skills().iter().enumerate() {
                    let ok = b.can_use(id, s);
                    let icon = kit.icon(s.icon);
                    let sel = bv.choice == Some(Choice::Skill(i));
                    row.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|cell| {
                        ui::icon_button(
                            cell,
                            icon,
                            72.0,
                            Act::Choose(Choice::Skill(i)),
                            ok,
                            sel,
                            Some(super::hamlet::skill_tooltip(s)),
                        );
                        cell.spawn(kit.text(
                            format!("{} {}", i + 1, s.name),
                            12.0,
                            if ok { BONE } else { DIM },
                        ));
                    });
                }
                let mv = kit.icon("Ability_Rogue_Sprint");
                let can_move = !valid_targets(b, id, Choice::Move).is_empty();
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|cell| {
                    ui::icon_button(
                        cell,
                        mv,
                        56.0,
                        Act::Choose(Choice::Move),
                        can_move,
                        bv.choice == Some(Choice::Move),
                        Some("Move (M)\nSwap places with an adjacent ally.".into()),
                    );
                    cell.spawn(kit.text("Move", 12.0, BONE));
                });
            });
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|c| {
                let hint = match bv.choice {
                    Some(Choice::Skill(i)) => format!("{}: choose a target.", u.skills()[i].name),
                    Some(Choice::Move) => "Choose an ally to swap with.".to_owned(),
                    None => "No skill can be used from this rank. Move or pass.".to_owned(),
                };
                c.spawn(kit.text(hint, 15.0, BONE));
                c.spawn(Node {
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|r| {
                    ui::button(r, &kit, "Pass", Act::Pass, true);
                    ui::button(r, &kit, "Retreat", Act::Retreat, !bv.boss);
                });
            });
        });
}

fn fade(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Fade, &mut Transform)>,
) {
    for (e, mut f, mut t) in &mut q {
        f.0 -= time.delta_secs();
        if f.0 < 0.5 {
            t.translation.y -= time.delta_secs() * 1.5;
        }
        if f.0 <= 0.0 {
            commands.entity(e).despawn();
        }
    }
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
