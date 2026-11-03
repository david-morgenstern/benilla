//! The hamlet's interface over the town scene, the world map an expedition is chosen on, the
//! provisioner, and the reckoning after a return.

use std::collections::HashMap;

use bevy::prelude::*;

use super::looks::{class_icon_rect, CLASS_ATLAS};
use super::town::{Clicked, Spot};
use super::ui::{self, Kit, Tip, BLOOD, BONE, DIM, EDGE, GOLD, HEAL};
use super::{save, ActiveOffer, Game, LastDebrief, Run, Scoped, Screen};
use crate::rules::combat::bonus_label;
use crate::rules::data::{has_rank, SkillDef, Target};
use crate::rules::dungeon::{Heirloom, Item};
use crate::rules::hero::{quirk, Hero, LEVEL_XP, MAX_LEVEL};
use crate::rules::items::{trinket, TrinketDef};
use crate::rules::regions::{level_name, max_resolve_for, QuestOffer, Region};
use crate::rules::talents::{talent, tree, TIER_LEVELS};
use crate::rules::town::{default_pack, Building, Campaign, Relief, MAX_BUILDING};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Panel {
    Spot(Spot),
    Hero,
}

#[derive(Resource)]
struct HamletUi {
    open: Option<Panel>,
    selected: Option<u32>,
    dirty: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    Map,
    Party,
}

#[derive(Resource)]
struct ProvisionUi {
    step: Step,
    region: Region,
    offer: Option<usize>,
    party: Vec<u32>,
    pack: HashMap<Item, i32>,
    dirty: bool,
}

#[derive(Component, Clone)]
enum Act {
    Close,
    Select(u32),
    Recruit(usize),
    Relieve(Relief),
    Treat(String),
    Upgrade(bool),
    Train(String),
    ToggleSkill(String),
    Learn(String),
    ResetTalents,
    BuildingUp(Building),
    BuyTrinket(usize),
    SellTrinket(usize),
    Equip(usize, usize),
    Unequip(usize),
    Dismiss,
    Embark,
    // The world map and provisioner.
    Region(Region),
    Offer(usize),
    ToParty,
    ToMap,
    PartyToggle(u32),
    Buy(Item, i32),
    Depart,
    Back,
    // Debrief.
    Home,
    Noop,
}

#[derive(Component)]
struct UiRoot;

pub struct HamletPlugin;

impl Plugin for HamletPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Hamlet), enter_hamlet)
            .add_systems(OnEnter(Screen::Provision), enter_provision)
            .add_systems(OnEnter(Screen::Debrief), build_debrief)
            .add_systems(
                Update,
                (
                    town_clicks.run_if(in_state(Screen::Hamlet)),
                    act,
                    rebuild_hamlet.run_if(in_state(Screen::Hamlet)),
                    rebuild_provision.run_if(in_state(Screen::Provision)),
                )
                    .chain(),
            )
            .add_systems(Update, autoplay_town);
    }
}

fn enter_hamlet(
    mut commands: Commands,
    game: Option<Res<Game>>,
    mut narrator: ResMut<ui::Narrator>,
) {
    let selected = game.as_ref().and_then(|g| g.0.roster.first().map(|h| h.id));
    if let Some(g) = &game {
        if g.0.week == 1 && g.0.runs == 0 {
            narrator.say(
                "Click the buildings of the hamlet. When the party is ready, click the estate.",
            );
        } else {
            narrator.say(g.0.event.text());
        }
    }
    commands.insert_resource(HamletUi {
        open: None,
        selected,
        dirty: true,
    });
}

fn enter_provision(mut commands: Commands) {
    commands.insert_resource(ProvisionUi {
        step: Step::Map,
        region: Region::Crypts,
        offer: None,
        party: Vec::new(),
        pack: HashMap::new(),
        dirty: true,
    });
}

fn town_clicks(
    mut clicked: ResMut<Clicked>,
    mut ui: ResMut<HamletUi>,
    mut next: ResMut<NextState<Screen>>,
) {
    if let Some(spot) = clicked.0.take() {
        if spot == Spot::Estate {
            next.set(Screen::Provision);
        } else {
            ui.open = Some(Panel::Spot(spot));
            ui.dirty = true;
        }
    }
}

fn clear(commands: &mut Commands, old: &Query<Entity, With<UiRoot>>) {
    for e in old {
        commands.entity(e).despawn();
    }
}

fn heirloom_strip(
    p: &mut ChildSpawnerCommands,
    kit: &mut Kit,
    gold: i32,
    heirlooms: &dyn Fn(Heirloom) -> i32,
) {
    p.spawn(Node {
        column_gap: Val::Px(14.0),
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|row| {
        let coin = kit.icon("INV_Misc_Coin_02");
        row.spawn((
            ImageNode::new(coin),
            Node {
                width: Val::Px(26.0),
                height: Val::Px(26.0),
                ..default()
            },
        ));
        row.spawn(kit.title(format!("{gold}"), 24.0, GOLD));
        for h in Heirloom::ALL {
            let icon = kit.icon(h.icon());
            row.spawn((
                ImageNode::new(icon),
                Node {
                    width: Val::Px(26.0),
                    height: Val::Px(26.0),
                    ..default()
                },
                Interaction::default(),
                Tip(format!(
                    "{}\nSpent to rebuild the hamlet's buildings.",
                    h.name()
                )),
            ));
            row.spawn(kit.text(format!("{}", heirlooms(h)), 18.0, BONE));
        }
    });
}

fn class_icon(p: &mut ChildSpawnerCommands, kit: &mut Kit, hero: &Hero, size: f32) {
    let atlas = kit.image(CLASS_ATLAS);
    p.spawn((
        ImageNode {
            image: atlas,
            rect: Some(class_icon_rect(hero.class)),
            ..default()
        },
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            flex_shrink: 0.0,
            ..default()
        },
    ));
}

fn roman(n: u8) -> &'static str {
    ["", "I", "II", "III", "IV", "V"][usize::from(n.min(5))]
}

fn hero_tip(h: &Hero) -> String {
    let st = h.stats();
    let skills: Vec<String> = h
        .equipped_skills()
        .iter()
        .map(|s| format!("{} {}", s.name, roman(h.rank(s.id))))
        .collect();
    let quirks: Vec<&str> = h
        .quirks
        .iter()
        .filter_map(|q| quirk(q))
        .map(|q| q.name)
        .collect();
    format!(
        "{} the {} — resolve {}\nHP {}/{}  Stress {}\nACC {:+}  CRIT {}%  DMG {}–{}  DODGE {}  PROT {}%  SPD {}\nSkills: {}\nQuirks: {}",
        h.name, h.class_name(), h.level, h.hp, h.max_hp(), h.stress, st.acc, st.crit, st.dmg.0, st.dmg.1, st.dodge, st.prot, st.speed,
        skills.join(", "),
        if quirks.is_empty() { "none".to_owned() } else { quirks.join(", ") }
    )
}

fn hero_card(
    p: &mut ChildSpawnerCommands,
    kit: &mut Kit,
    hero: &Hero,
    act: Act,
    selected: bool,
    note: Option<&str>,
) {
    p.spawn((
        Button,
        Node {
            width: Val::Px(326.0),
            padding: UiRect::all(Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.0)),
            column_gap: Val::Px(8.0),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(if selected { ui::BTN_DOWN } else { ui::BTN }),
        BorderColor::all(if selected { GOLD } else { EDGE }),
        ui::BtnLook {
            enabled: true,
            selected,
        },
        Tip(hero_tip(hero)),
        act,
    ))
    .with_children(|c| {
        class_icon(c, kit, hero, 40.0);
        c.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .with_children(|col| {
            col.spawn(kit.title(hero.name.clone(), 18.0, BONE));
            let status = if let Some(n) = note {
                format!(" — {n}")
            } else if hero.resting {
                " — resting".to_owned()
            } else if let Some(a) = hero.affliction {
                format!(" — {}", a.name())
            } else if !hero.open_talent_tiers().is_empty() {
                " — talent ready".to_owned()
            } else {
                String::new()
            };
            col.spawn(kit.text(
                format!("Lv {} {}{}", hero.level, hero.class_name(), status),
                12.0,
                if hero.affliction.is_some() || note.is_some() {
                    BLOOD
                } else {
                    DIM
                },
            ));
            col.spawn(Node {
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|r| {
                ui::bar(r, 86.0, 7.0, hero.hp as f32 / hero.max_hp() as f32, BLOOD);
                ui::stress_pips(r, hero.stress, 8.0);
            });
        });
    });
}

fn skill_tip(s: &SkillDef, rank: u8) -> String {
    let dots = |mask: u8| -> String {
        (1..=4)
            .rev()
            .map(|r| if has_rank(mask, r) { '●' } else { '○' })
            .collect()
    };
    let mut t = format!(
        "{} {}\n{}\n\nUse from: {}",
        s.name,
        roman(rank),
        s.desc,
        dots(s.launch)
    );
    match s.target {
        Target::Enemy(m) => {
            t += &format!("\nTarget: {}", dots(m).chars().rev().collect::<String>())
        }
        Target::EnemyAll(m) => {
            t += &format!(
                "\nTargets all: {}",
                dots(m).chars().rev().collect::<String>()
            )
        }
        Target::Ally(_) | Target::AllyOther(_) => t += "\nTarget: an ally",
        Target::AllyAll => t += "\nTarget: the whole party",
        Target::SelfOnly => t += "\nTarget: self",
    }
    let r = i32::from(rank) - 1;
    if s.target.is_enemy() && s.dmg_mod > -100 {
        t += &format!(
            "\nAccuracy {}  Damage {:+}%  Crit {:+}%",
            s.acc + 4 * r,
            s.dmg_mod + 8 * r,
            s.crit_mod + r
        );
    }
    if let Some((lo, hi)) = s.heal {
        t += &format!("\nHeals {lo}–{hi} (+{}%)", 15 * r);
    }
    if let Some(b) = bonus_label(s) {
        t += &format!("\n{b}");
    }
    if s.limit > 0 {
        t += &format!("\nLimit {} per battle", s.limit);
    }
    t
}

pub fn skill_tooltip(s: &SkillDef) -> String {
    skill_tip(s, 1)
}

pub fn skill_tooltip_ranked(s: &SkillDef, rank: u8) -> String {
    skill_tip(s, rank)
}

fn trinket_slot(
    p: &mut ChildSpawnerCommands,
    kit: &mut Kit,
    t: Option<&TrinketDef>,
    act: Option<Act>,
    size: f32,
) {
    let (img, tip, color) = match t {
        Some(t) => {
            let (r, g, b) = t.rarity.color();
            (kit.icon(t.icon), t.summary(), Color::srgb(r, g, b))
        }
        None => (
            kit.icon("INV_Misc_QuestionMark"),
            "Empty trinket slot".to_owned(),
            EDGE,
        ),
    };
    let mut e = p.spawn((
        Button,
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BorderColor::all(color),
        BackgroundColor(Color::BLACK),
        Tip(tip),
    ));
    e.with_children(|b| {
        b.spawn((
            ImageNode {
                image: img,
                color: if t.is_some() {
                    Color::WHITE
                } else {
                    Color::srgb(0.2, 0.2, 0.2)
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
    if let Some(a) = act {
        e.insert(a);
    }
}

fn divider(p: &mut ChildSpawnerCommands) {
    p.spawn((
        Node {
            height: Val::Px(1.0),
            width: Val::Percent(100.0),
            margin: UiRect::vertical(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(EDGE),
    ));
}

#[allow(clippy::too_many_lines)]
fn rebuild_hamlet(
    mut commands: Commands,
    mut kit: Kit,
    mut ui_state: ResMut<HamletUi>,
    game: Res<Game>,
    old: Query<Entity, With<UiRoot>>,
    mut portrait: ResMut<super::portrait::Portrait>,
) {
    if !ui_state.dirty {
        return;
    }
    ui_state.dirty = false;
    clear(&mut commands, &old);
    let c = &game.0;
    if ui_state
        .selected
        .is_none_or(|id| !c.roster.iter().any(|h| h.id == id))
    {
        ui_state.selected = c.roster.first().map(|h| h.id);
    }
    portrait.want = ui_state.selected;
    let sel = ui_state
        .selected
        .and_then(|id| c.roster.iter().find(|h| h.id == id));
    let scoped = Scoped(Screen::Hamlet);
    // Top bar.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            UiRoot,
            scoped,
        ))
        .with_children(|top| {
            top.spawn(Node {
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|t| {
                t.spawn((
                    kit.title(format!("The Hamlet — Week {}", c.week), 34.0, GOLD),
                    TextShadow::default(),
                ));
                t.spawn((kit.text(c.event.text(), 13.0, BONE), TextShadow::default()));
            });
            top.spawn(Node {
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|r| {
                ui::button_sized(
                    r,
                    &kit,
                    "Embark >",
                    Act::Embark,
                    c.roster.iter().any(|h| !h.resting),
                    false,
                    18.0,
                );
            });
            heirloom_strip(top, &mut kit, c.gold, &|h| c.heirloom(h));
        });
    // Roster.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(56.0),
                right: Val::Px(10.0),
                width: Val::Px(342.0),
                max_height: Val::Percent(88.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(6.0)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.015, 0.01, 0.82)),
            BorderColor::all(EDGE),
            UiRoot,
            scoped,
        ))
        .with_children(|col| {
            col.spawn(kit.title(
                format!("Roster ({}/{})", c.roster.len(), c.roster_cap()),
                22.0,
                BONE,
            ));
            for h in &c.roster {
                hero_card(
                    col,
                    &mut kit,
                    h,
                    Act::Select(h.id),
                    Some(h.id) == ui_state.selected,
                    None,
                );
            }
            if c.roster.is_empty() {
                col.spawn(kit.text("Empty. Visit the stagecoach.", 14.0, DIM));
            }
        });
    // The open panel.
    let Some(panel) = ui_state.open else {
        commands.spawn((
            Node { position_type: PositionType::Absolute, bottom: Val::Px(14.0), left: Val::Px(16.0), ..default() },
            kit.text("Click a building to enter it. Click the estate on the hill to embark. Click a hero for their sheet.", 15.0, BONE),
            TextShadow::default(),
            UiRoot,
            scoped,
        ));
        return;
    };
    let portrait_img = portrait.image.clone();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(60.0),
                left: Val::Px(16.0),
                width: Val::Px(800.0),
                max_height: Val::Percent(88.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(2.0)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(ui::PANEL),
            BorderColor::all(EDGE),
            UiRoot,
            scoped,
        ))
        .with_children(|p| {
            p.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                width: Val::Percent(100.0),
                ..default()
            })
            .with_children(|r| {
                let title = match panel {
                    Panel::Spot(s) => s.name().to_owned(),
                    Panel::Hero => "Character".to_owned(),
                };
                r.spawn(kit.title(title, 32.0, GOLD));
                ui::button(r, &kit, "Close", Act::Close, true);
            });
            match panel {
                Panel::Hero => {
                    if let Some(h) = sel {
                        hero_sheet(p, &mut kit, c, h, portrait_img.clone());
                    }
                }
                Panel::Spot(Spot::Memorial) => memorial(p, &kit, c),
                Panel::Spot(Spot::Building(b)) => {
                    if let Some(h) = sel {
                        if b != Building::Stagecoach && b != Building::Wagon {
                            p.spawn(Node {
                                column_gap: Val::Px(10.0),
                                align_items: AlignItems::Center,
                                ..default()
                            })
                            .with_children(|r| {
                                class_icon(r, &mut kit, h, 30.0);
                                r.spawn(kit.text(
                                    format!(
                                        "Serving: {} the {} (pick another in the roster)",
                                        h.name,
                                        h.class_name()
                                    ),
                                    15.0,
                                    BONE,
                                ));
                            });
                        }
                    }
                    building_panel(p, &mut kit, b, c, sel);
                    divider(p);
                    upgrade_block(p, &mut kit, b, c);
                }
                Panel::Spot(Spot::Estate) => {}
            }
        });
}

fn memorial(p: &mut ChildSpawnerCommands, kit: &Kit, c: &Campaign) {
    if c.graveyard.is_empty() {
        p.spawn(kit.text("No names on the stone. Not yet.", 16.0, DIM));
    }
    for g in c.graveyard.iter().rev().take(18) {
        p.spawn(kit.text(
            format!(
                "{} the {} (resolve {}) — {}, week {}",
                g.hero.name,
                g.hero.class_name(),
                g.hero.level,
                g.cause,
                g.week
            ),
            15.0,
            BONE,
        ));
    }
}

fn upgrade_block(p: &mut ChildSpawnerCommands, kit: &mut Kit, b: Building, c: &Campaign) {
    let lvl = c.level(b);
    p.spawn(kit.title(
        format!("Rebuilding — level {lvl}/{MAX_BUILDING}"),
        20.0,
        BONE,
    ));
    p.spawn(kit.text(format!("Now: {}", b.upgrade_effect(lvl)), 14.0, DIM));
    if lvl < MAX_BUILDING {
        let cost = Building::upgrade_cost(lvl);
        let (h1, h2) = b.heirlooms();
        let ok = c.heirloom(h1) >= cost && c.heirloom(h2) >= cost;
        p.spawn(Node {
            column_gap: Val::Px(10.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|r| {
            ui::button(
                r,
                kit,
                &format!("Upgrade: {cost} {}s + {cost} {}s", h1.name(), h2.name()),
                Act::BuildingUp(b),
                ok,
            );
            r.spawn(kit.text(
                format!("→ {}", b.upgrade_effect(lvl + 1)),
                14.0,
                if ok { HEAL } else { DIM },
            ));
        });
    }
}

#[allow(clippy::too_many_lines)]
fn building_panel(
    p: &mut ChildSpawnerCommands,
    kit: &mut Kit,
    b: Building,
    c: &Campaign,
    sel: Option<&Hero>,
) {
    match b {
        Building::Stagecoach => {
            p.spawn(kit.text("Fresh faces off the coach road, each sure they will be the one to come back rich. Recruits leave at the week's end.", 15.0, DIM));
            for (i, h) in c.stagecoach.iter().enumerate() {
                p.spawn((
                    Node {
                        column_gap: Val::Px(10.0),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Interaction::default(),
                    Tip(hero_tip(h)),
                ))
                .with_children(|r| {
                    class_icon(r, kit, h, 40.0);
                    let quirks: Vec<&str> = h
                        .quirks
                        .iter()
                        .filter_map(|q| quirk(q))
                        .map(|q| q.name)
                        .collect();
                    r.spawn((
                        kit.text(
                            format!(
                                "{} the {} — resolve {}\n{}",
                                h.name,
                                h.class_name(),
                                h.level,
                                quirks.join(", ")
                            ),
                            15.0,
                            BONE,
                        ),
                        Node {
                            width: Val::Px(440.0),
                            ..default()
                        },
                    ));
                    ui::button(
                        r,
                        kit,
                        "Hire",
                        Act::Recruit(i),
                        c.roster.len() < c.roster_cap(),
                    );
                });
            }
            if c.stagecoach.is_empty() {
                p.spawn(kit.text("The coach is empty this week.", 15.0, DIM));
            }
        }
        Building::Tavern | Building::Chapel => {
            let relief = if b == Building::Tavern {
                Relief::Tavern
            } else {
                Relief::Chapel
            };
            let (lo, hi) = c.relief_amount(relief);
            p.spawn(kit.text(
                format!(
                    "{}\nRemoves {lo}–{hi} stress and breaks an affliction; the hero sits out the next expedition.\nCost {} gold. Places left this week: {}.",
                    if b == Building::Tavern {
                        "Cheap ale, loaded dice, and a fire that never quite warms."
                    } else {
                        "Candles, silence, and a priest who has stopped asking questions."
                    },
                    c.relief_cost(relief),
                    c.relief_slots(relief)
                ),
                15.0,
                DIM,
            ));
            if let Some(h) = sel {
                let ok = !h.resting
                    && (h.stress > 0 || h.affliction.is_some())
                    && c.gold >= c.relief_cost(relief)
                    && c.relief_slots(relief) > 0;
                ui::button(
                    p,
                    kit,
                    &format!("Send {} (stress {})", h.name, h.stress),
                    Act::Relieve(relief),
                    ok,
                );
            }
            let resting: Vec<&str> = c
                .roster
                .iter()
                .filter(|h| h.resting)
                .map(|h| h.name.as_str())
                .collect();
            if !resting.is_empty() {
                p.spawn(kit.text(
                    format!("Resting this week: {}", resting.join(", ")),
                    14.0,
                    DIM,
                ));
            }
        }
        Building::Sanitarium => {
            p.spawn(kit.text(
                format!(
                    "The physicians cut a bad habit out of a hero. {} gold, and a week abed.",
                    c.treat_cost()
                ),
                15.0,
                DIM,
            ));
            if let Some(h) = sel {
                let neg: Vec<_> = h
                    .quirks
                    .iter()
                    .filter_map(|q| quirk(q))
                    .filter(|q| !q.positive)
                    .collect();
                if neg.is_empty() {
                    p.spawn(kit.text(
                        format!("{} has nothing the physicians can treat.", h.name),
                        15.0,
                        BONE,
                    ));
                }
                for q in neg {
                    ui::button(
                        p,
                        kit,
                        &format!("Treat {} ({}g)", q.name, c.treat_cost()),
                        Act::Treat(q.id.to_owned()),
                        !h.resting && c.gold >= c.treat_cost(),
                    );
                }
            }
        }
        Building::Blacksmith => {
            p.spawn(kit.text(
                format!(
                    "Better steel, for gold. The smith can forge up to tier {}.",
                    c.max_gear_tier()
                ),
                15.0,
                DIM,
            ));
            if let Some(h) = sel {
                for weapon in [true, false] {
                    let tier = if weapon { h.weapon } else { h.armor };
                    let what = if weapon { "Weapon" } else { "Armour" };
                    if tier >= 5 {
                        p.spawn(kit.text(format!("{what}: tier 5 (masterwork)"), 16.0, BONE));
                        continue;
                    }
                    let gold = Hero::upgrade_cost(tier);
                    let capped = tier >= c.max_gear_tier();
                    let ok = c.gold >= gold && !capped;
                    let gain = if weapon {
                        "+damage, +crit"
                    } else {
                        "+max HP, +dodge"
                    };
                    let label = format!(
                        "{what} tier {tier} → {} ({gain}) — {gold}g{}",
                        tier + 1,
                        if capped {
                            " — rebuild the smithy"
                        } else {
                            ""
                        }
                    );
                    ui::button(p, kit, &label, Act::Upgrade(weapon), ok);
                }
            }
        }
        Building::Guild => guild(p, kit, c, sel),
        Building::Wagon => {
            p.spawn(kit.text("A painted wagon, a toothless trader, and charms of uncertain provenance. New stock each week.", 15.0, DIM));
            p.spawn(kit.title("For sale", 20.0, BONE));
            for (i, id) in c.wagon.iter().enumerate() {
                if let Some(t) = trinket(id) {
                    p.spawn(Node {
                        column_gap: Val::Px(10.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|r| {
                        trinket_slot(r, kit, Some(t), None, 42.0);
                        let (cr, cg, cb) = t.rarity.color();
                        r.spawn((
                            kit.text(
                                format!("{} ({})", t.name, t.rarity.name()),
                                15.0,
                                Color::srgb(cr, cg, cb),
                            ),
                            Node {
                                width: Val::Px(360.0),
                                ..default()
                            },
                        ));
                        let price = c.wagon_price(id);
                        ui::button(
                            r,
                            kit,
                            &format!("Buy {price}g"),
                            Act::BuyTrinket(i),
                            c.gold >= price,
                        );
                    });
                }
            }
            divider(p);
            p.spawn(kit.title(
                format!("The hamlet's stash ({})", c.trinkets.len()),
                20.0,
                BONE,
            ));
            p.spawn(Node {
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(8.0),
                row_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|w| {
                for (i, id) in c.trinkets.iter().enumerate() {
                    if let Some(t) = trinket(id) {
                        w.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|cell| {
                            trinket_slot(cell, kit, Some(t), None, 42.0);
                            ui::button_sized(
                                cell,
                                kit,
                                &format!("Sell {}", t.price() / 4),
                                Act::SellTrinket(i),
                                true,
                                false,
                                11.0,
                            );
                        });
                    }
                }
            });
            p.spawn(kit.text("Equip trinkets from a hero's character sheet.", 13.0, DIM));
        }
    }
}

fn guild(p: &mut ChildSpawnerCommands, kit: &mut Kit, c: &Campaign, sel: Option<&Hero>) {
    p.spawn(kit.text(
        format!("Masters drill heroes in their craft. Skills train up to rank {}. Click an icon to equip or unequip it (four at most).", c.max_skill_rank()),
        15.0,
        DIM,
    ));
    let Some(h) = sel else { return };
    for s in h.class.def().skills {
        let on = h.skills.iter().any(|x| x == s.id);
        let rank = h.rank(s.id);
        p.spawn(Node {
            column_gap: Val::Px(10.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|r| {
            let icon = kit.icon(s.icon);
            ui::icon_button(
                r,
                icon,
                46.0,
                Act::ToggleSkill(s.id.to_owned()),
                true,
                on,
                Some(skill_tip(s, rank)),
            );
            r.spawn((
                kit.text(
                    format!(
                        "{} {}{}",
                        s.name,
                        roman(rank),
                        if on { "  (equipped)" } else { "" }
                    ),
                    15.0,
                    if on { GOLD } else { BONE },
                ),
                Node {
                    width: Val::Px(320.0),
                    ..default()
                },
            ));
            if rank < 5 {
                let cost = Campaign::skill_cost(rank + 1);
                let capped = rank + 1 > c.max_skill_rank();
                let label = if capped {
                    "Rebuild the guild".to_owned()
                } else {
                    format!("Train to {} — {cost}g", roman(rank + 1))
                };
                ui::button(
                    r,
                    kit,
                    &label,
                    Act::Train(s.id.to_owned()),
                    !capped && c.gold >= cost,
                );
            } else {
                r.spawn(kit.text("Mastered", 14.0, HEAL));
            }
        });
    }
    divider(p);
    p.spawn(kit.title("Talents", 22.0, BONE));
    p.spawn(kit.text(
        "A talent opens at resolve 1, 3 and 5. Choose one of two in each tier.",
        13.0,
        DIM,
    ));
    let open = h.open_talent_tiers();
    for (tier, pair) in tree(h.class).iter().enumerate() {
        p.spawn(Node {
            column_gap: Val::Px(12.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|r| {
            r.spawn((
                kit.text(format!("Resolve {}", TIER_LEVELS[tier]), 14.0, DIM),
                Node {
                    width: Val::Px(80.0),
                    ..default()
                },
            ));
            for t in pair {
                let taken = h.talents.iter().any(|x| x == t.id);
                let can = open.contains(&tier);
                let icon = kit.icon(t.icon);
                ui::icon_button(
                    r,
                    icon,
                    40.0,
                    Act::Learn(t.id.to_owned()),
                    can || taken,
                    taken,
                    Some(format!("{}\n{}", t.name, t.desc)),
                );
                r.spawn((
                    kit.text(
                        t.name,
                        14.0,
                        if taken {
                            GOLD
                        } else if can {
                            BONE
                        } else {
                            DIM
                        },
                    ),
                    Node {
                        width: Val::Px(200.0),
                        ..default()
                    },
                ));
            }
        });
    }
    ui::button(
        p,
        kit,
        &format!("Unlearn all talents ({}g)", Campaign::TALENT_RESET),
        Act::ResetTalents,
        !h.talents.is_empty() && c.gold >= Campaign::TALENT_RESET,
    );
}

fn hero_sheet(
    p: &mut ChildSpawnerCommands,
    kit: &mut Kit,
    c: &Campaign,
    h: &Hero,
    portrait: Handle<Image>,
) {
    let st = h.stats();
    p.spawn(Node {
        column_gap: Val::Px(14.0),
        ..default()
    })
    .with_children(|r| {
        r.spawn((
            Node {
                width: Val::Px(150.0),
                height: Val::Px(150.0),
                border: UiRect::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BorderColor::all(GOLD),
            BackgroundColor(Color::BLACK),
        ))
        .with_children(|f| {
            f.spawn((
                ImageNode::new(portrait),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
            ));
        });
        r.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .with_children(|col| {
            col.spawn(kit.title(format!("{} the {}", h.name, h.class_name()), 28.0, GOLD));
            let next = if h.level < MAX_LEVEL {
                format!("{}/{} xp", h.xp, LEVEL_XP[usize::from(h.level + 1)])
            } else {
                "max".to_owned()
            };
            col.spawn(kit.text(
                format!(
                    "Resolve {} ({next}) — weapon {}, armour {} — {} expeditions, {} kills",
                    h.level, h.weapon, h.armor, h.expeditions, h.kills
                ),
                14.0,
                DIM,
            ));
            col.spawn(kit.text(
                format!("HP {}/{}   Stress {}/200", h.hp, h.max_hp(), h.stress),
                16.0,
                BONE,
            ));
            col.spawn(kit.text(
                format!(
                    "ACC {:+}  CRIT {}%  DMG {}–{}  DODGE {}  PROT {}%  SPD {}",
                    st.acc, st.crit, st.dmg.0, st.dmg.1, st.dodge, st.prot, st.speed
                ),
                15.0,
                BONE,
            ));
            col.spawn(kit.text(
                format!(
                    "Resist: stun {}%  blight {}%  bleed {}%  debuff {}%  move {}%  deathblow {}%",
                    st.res.stun,
                    st.res.blight,
                    st.res.bleed,
                    st.res.debuff,
                    st.res.moves,
                    st.res.death_blow
                ),
                13.0,
                DIM,
            ));
            let quirks: Vec<String> = h
                .quirks
                .iter()
                .filter_map(|q| quirk(q))
                .map(|q| format!("{}{}", if q.positive { "+" } else { "−" }, q.name))
                .collect();
            col.spawn(kit.text(
                format!(
                    "Quirks: {}",
                    if quirks.is_empty() {
                        "none".into()
                    } else {
                        quirks.join(", ")
                    }
                ),
                13.0,
                DIM,
            ));
            if let Some(a) = h.affliction {
                col.spawn(kit.title(format!("Afflicted: {}", a.name()), 18.0, BLOOD));
            }
        });
    });
    divider(p);
    p.spawn(kit.title("Skills", 20.0, BONE));
    p.spawn(Node {
        column_gap: Val::Px(6.0),
        ..default()
    })
    .with_children(|r| {
        for s in h.class.def().skills {
            let on = h.skills.iter().any(|x| x == s.id);
            let icon = kit.icon(s.icon);
            ui::icon_button(
                r,
                icon,
                44.0,
                Act::ToggleSkill(s.id.to_owned()),
                true,
                on,
                Some(skill_tip(s, h.rank(s.id))),
            );
        }
    });
    let talents: Vec<&str> = h
        .talents
        .iter()
        .filter_map(|t| talent(h.class, t))
        .map(|t| t.name)
        .collect();
    p.spawn(kit.text(
        format!(
            "Talents: {}",
            if talents.is_empty() {
                "none yet".into()
            } else {
                talents.join(", ")
            }
        ),
        14.0,
        DIM,
    ));
    divider(p);
    p.spawn(kit.title("Trinkets", 20.0, BONE));
    p.spawn(Node {
        column_gap: Val::Px(14.0),
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|r| {
        for slot in 0..2 {
            let t = h.trinkets[slot].as_deref().and_then(trinket);
            r.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|cell| {
                trinket_slot(cell, kit, t, None, 56.0);
                ui::button_sized(
                    cell,
                    kit,
                    "Remove",
                    Act::Unequip(slot),
                    t.is_some(),
                    false,
                    11.0,
                );
            });
        }
    });
    if !c.trinkets.is_empty() {
        p.spawn(kit.text("From the stash (equip to slot 1 or 2):", 13.0, DIM));
        p.spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(8.0),
            row_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|w| {
            for (i, id) in c.trinkets.iter().enumerate() {
                let Some(t) = trinket(id) else { continue };
                let fits = t.class.is_none_or(|cl| cl == h.class);
                w.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|cell| {
                    trinket_slot(cell, kit, Some(t), None, 40.0);
                    cell.spawn(Node {
                        column_gap: Val::Px(2.0),
                        ..default()
                    })
                    .with_children(|bb| {
                        ui::button_sized(bb, kit, "1", Act::Equip(0, i), fits, false, 11.0);
                        ui::button_sized(bb, kit, "2", Act::Equip(1, i), fits, false, 11.0);
                    });
                });
            }
        });
    }
    divider(p);
    ui::button(
        p,
        kit,
        "Dismiss from the roster",
        Act::Dismiss,
        c.roster.len() > 1,
    );
}

// --------------------------------------------------------------------------- the world map

fn region_marker(r: Region) -> (f32, f32, &'static str) {
    match r {
        Region::Crypts => (0.24, 0.40, "Spell_Shadow_RaiseDead"),
        Region::Warrens => (0.58, 0.20, "INV_Pick_02"),
        Region::Grotto => (0.42, 0.80, "Spell_Frost_SummonWaterElemental"),
    }
}

#[allow(clippy::too_many_lines)]
fn rebuild_provision(
    mut commands: Commands,
    mut kit: Kit,
    mut pv: ResMut<ProvisionUi>,
    game: Res<Game>,
    old: Query<Entity, With<UiRoot>>,
) {
    if !pv.dirty {
        return;
    }
    pv.dirty = false;
    clear(&mut commands, &old);
    let c = &game.0;
    let bg = kit.image("Interface\\Glues\\LoadingScreens\\LoadScreenDungeon");
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            ImageNode {
                image: bg,
                color: Color::srgb(0.18, 0.16, 0.16),
                ..default()
            },
            Scoped(Screen::Provision),
            UiRoot,
        ))
        .id();
    let offer = pv.offer.and_then(|i| c.offers.get(i)).cloned();
    commands.entity(root).with_children(|p| {
        p.spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            width: Val::Percent(100.0),
            ..default()
        })
        .with_children(|top| {
            top.spawn(kit.title(
                if pv.step == Step::Map {
                    "Choose an Expedition"
                } else {
                    "Prepare the Expedition"
                },
                36.0,
                GOLD,
            ));
            heirloom_strip(top, &mut kit, c.gold, &|h| c.heirloom(h));
        });
        match pv.step {
            Step::Map => world_map(p, &mut kit, c, &pv, offer.as_ref()),
            Step::Party => party_step(p, &mut kit, c, &pv, offer.as_ref()),
        }
    });
}

fn world_map(
    p: &mut ChildSpawnerCommands,
    kit: &mut Kit,
    c: &Campaign,
    pv: &ProvisionUi,
    offer: Option<&QuestOffer>,
) {
    p.spawn(Node {
        column_gap: Val::Px(16.0),
        flex_grow: 1.0,
        ..default()
    })
    .with_children(|main| {
        // The map of the estate's lands, drawn from the install's own Duskwood map.
        // The map art fills 1002x668 of its 1024x768 tile grid; the frame crops the rest.
        let (mw, mh) = (960.0, 640.0);
        main.spawn((
            Node {
                width: Val::Px(mw),
                height: Val::Px(mh),
                border: UiRect::all(Val::Px(3.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(EDGE),
            BackgroundColor(Color::BLACK),
        ))
        .with_children(|frame| {
            frame
                .spawn(Node {
                    width: Val::Px(mw * 1024.0 / 1002.0),
                    height: Val::Px(mh * 768.0 / 668.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|m| {
                    for i in 1..=12 {
                        let tile =
                            kit.image(&format!("Interface\\WorldMap\\Duskwood\\Duskwood{i}"));
                        m.spawn((
                            ImageNode {
                                image: tile,
                                color: Color::srgb(0.75, 0.68, 0.6),
                                ..default()
                            },
                            Node {
                                width: Val::Percent(25.0),
                                height: Val::Percent(33.333),
                                ..default()
                            },
                        ));
                    }
                    // The hamlet itself.
                    m.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(mw * 0.76),
                            top: Val::Px(mh * 0.47),
                            ..default()
                        },
                        kit.title("* The Hamlet", 20.0, GOLD),
                        TextShadow::default(),
                    ));
                    for r in Region::ALL {
                        let (x, y, icon) = region_marker(r);
                        let n = c.offers.iter().filter(|o| o.region == r).count();
                        m.spawn(Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(mw * x - 32.0),
                            top: Val::Px(mh * y - 32.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|mk| {
                            let img = kit.icon(icon);
                            ui::icon_button(
                                mk,
                                img,
                                64.0,
                                Act::Region(r),
                                true,
                                pv.region == r,
                                Some(format!(
                                    "{}\n{}\n{n} expeditions on offer",
                                    r.name(),
                                    r.blurb()
                                )),
                            );
                            mk.spawn((
                                kit.title(r.name(), 18.0, if pv.region == r { GOLD } else { BONE }),
                                TextShadow::default(),
                            ));
                            if c.bosses.contains(&r) {
                                mk.spawn(kit.text("(lair cleansed)", 12.0, HEAL));
                            }
                        });
                    }
                });
        });
        // The region's offers.
        main.spawn(ui::panel(Val::Auto, Val::Px(640.0)))
            .insert(Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            })
            .with_children(|col| {
                let r = pv.region;
                col.spawn(kit.title(r.name(), 28.0, GOLD));
                col.spawn(kit.text(r.blurb(), 14.0, DIM));
                divider(col);
                for (i, o) in c.offers.iter().enumerate().filter(|(_, o)| o.region == r) {
                    let sel = pv.offer == Some(i);
                    col.spawn((
                        Button,
                        Node {
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(8.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(if sel { ui::BTN_DOWN } else { ui::BTN }),
                        BorderColor::all(if sel { GOLD } else { EDGE }),
                        ui::BtnLook {
                            enabled: true,
                            selected: sel,
                        },
                        Act::Offer(i),
                    ))
                    .with_children(|b| {
                        b.spawn(kit.title(
                            o.title(),
                            19.0,
                            if o.kind == crate::rules::regions::QuestKind::Boss {
                                BLOOD
                            } else {
                                BONE
                            },
                        ));
                        b.spawn(kit.text(
                            format!(
                                "Reward: {} gold, {} heirlooms{}   ·   heroes up to resolve {}",
                                o.reward_gold,
                                o.reward_heirlooms,
                                if o.reward_trinket { ", a trinket" } else { "" },
                                max_resolve_for(o.level)
                            ),
                            13.0,
                            DIM,
                        ));
                    });
                }
                col.spawn(Node {
                    flex_grow: 1.0,
                    ..default()
                });
                if let Some(o) = offer {
                    col.spawn(kit.text(
                        format!(
                            "{} — {} rooms, {} firewood. {}.",
                            level_name(o.level),
                            o.length.rooms(),
                            o.length.firewood(),
                            o.goal()
                        ),
                        14.0,
                        BONE,
                    ));
                }
                col.spawn(Node {
                    column_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|rr| {
                    ui::button_sized(rr, kit, "Back to the Hamlet", Act::Back, true, false, 18.0);
                    ui::button_sized(
                        rr,
                        kit,
                        "Choose the Party >",
                        Act::ToParty,
                        offer.is_some(),
                        false,
                        22.0,
                    );
                });
            });
    });
}

fn party_step(
    p: &mut ChildSpawnerCommands,
    kit: &mut Kit,
    c: &Campaign,
    pv: &ProvisionUi,
    offer: Option<&QuestOffer>,
) {
    let Some(offer) = offer else { return };
    p.spawn(kit.title(
        format!("{} — {}", offer.region.name(), offer.title()),
        22.0,
        BONE,
    ));
    p.spawn(Node {
        column_gap: Val::Px(16.0),
        flex_grow: 1.0,
        ..default()
    })
    .with_children(|main| {
        main.spawn(ui::panel(Val::Px(760.0), Val::Percent(100.0)))
            .with_children(|col| {
                col.spawn(kit.title("Party (front rank first)", 22.0, BONE));
                for (i, id) in pv.party.iter().enumerate() {
                    if let Some(h) = c.roster.iter().find(|h| h.id == *id) {
                        col.spawn(Node {
                            column_gap: Val::Px(8.0),
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|r| {
                            r.spawn((
                                kit.title(format!("Rank {}", i + 1), 18.0, GOLD),
                                Node {
                                    width: Val::Px(70.0),
                                    ..default()
                                },
                            ));
                            hero_card(r, kit, h, Act::PartyToggle(h.id), true, None);
                        });
                    }
                }
                col.spawn(kit.text(
                    "Click a hero to add or remove them. Rank 1 stands at the front.",
                    13.0,
                    DIM,
                ));
                divider(col);
                col.spawn(kit.title("Available", 20.0, BONE));
                col.spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(6.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|w| {
                    for h in c.roster.iter().filter(|h| !pv.party.contains(&h.id)) {
                        match c.refusal(h, offer) {
                            None => hero_card(w, kit, h, Act::PartyToggle(h.id), false, None),
                            Some(why) => hero_card(w, kit, h, Act::Noop, false, Some(why)),
                        }
                    }
                });
            });
        main.spawn(ui::panel(Val::Auto, Val::Percent(100.0)))
            .insert(Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            })
            .with_children(|col| {
                col.spawn(kit.title("Provisioner", 26.0, BONE));
                col.spawn(kit.text(
                    format!(
                        "Suggested for a {} expedition. Firewood: {}.",
                        offer.length.name().to_lowercase(),
                        offer.length.firewood()
                    ),
                    13.0,
                    DIM,
                ));
                let mut total = 0;
                for item in Item::ALL {
                    let n = pv.pack.get(&item).copied().unwrap_or(0);
                    total += n * item.price();
                    col.spawn((
                        Node {
                            column_gap: Val::Px(8.0),
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        Interaction::default(),
                        Tip(item.desc().to_owned()),
                    ))
                    .with_children(|r| {
                        let icon = kit.icon(item.icon());
                        r.spawn((
                            ImageNode::new(icon),
                            Node {
                                width: Val::Px(34.0),
                                height: Val::Px(34.0),
                                ..default()
                            },
                        ));
                        r.spawn((
                            kit.text(format!("{} ({}g)", item.name(), item.price()), 15.0, BONE),
                            Node {
                                width: Val::Px(220.0),
                                ..default()
                            },
                        ));
                        ui::button(r, kit, "−", Act::Buy(item, -1), n > 0);
                        r.spawn((
                            kit.title(format!("{n}"), 20.0, GOLD),
                            Node {
                                width: Val::Px(34.0),
                                ..default()
                            },
                        ));
                        ui::button(r, kit, "+", Act::Buy(item, 1), true);
                    });
                }
                col.spawn(kit.text(
                    format!("Pack cost: {total} gold (of {})", c.gold),
                    16.0,
                    if total > c.gold { BLOOD } else { BONE },
                ));
                col.spawn(Node {
                    flex_grow: 1.0,
                    ..default()
                });
                col.spawn(Node {
                    column_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|r| {
                    ui::button_sized(r, kit, "< Map", Act::ToMap, true, false, 18.0);
                    ui::button_sized(
                        r,
                        kit,
                        "Embark",
                        Act::Depart,
                        !pv.party.is_empty() && total <= c.gold,
                        false,
                        26.0,
                    );
                });
            });
    });
}

fn build_debrief(
    mut commands: Commands,
    mut kit: Kit,
    last: Res<LastDebrief>,
    old: Query<Entity, With<UiRoot>>,
) {
    clear(&mut commands, &old);
    let bg = kit.image("Interface\\Glues\\LoadingScreens\\LoadScreenScholomance");
    let Some(d) = &last.0 else { return };
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            ImageNode {
                image: bg,
                color: Color::srgb(0.25, 0.22, 0.22),
                ..default()
            },
            Scoped(Screen::Debrief),
            UiRoot,
        ))
        .with_children(|p| {
            p.spawn(ui::panel(Val::Px(860.0), Val::Auto))
                .with_children(|col| {
                    col.spawn(kit.title(
                        if d.success {
                            "The Expedition Returns Triumphant"
                        } else {
                            "The Expedition Returns, Broken"
                        },
                        38.0,
                        if d.success { GOLD } else { BLOOD },
                    ));
                    col.spawn(kit.text(d.quest.clone(), 16.0, DIM));
                    col.spawn(kit.text(format!("Gold recovered: {}", d.gold), 20.0, GOLD));
                    let heir: Vec<String> = d
                        .heirlooms
                        .iter()
                        .filter(|(_, n)| *n > 0)
                        .map(|(h, n)| format!("{} × {n}", h.name()))
                        .collect();
                    if !heir.is_empty() {
                        col.spawn(kit.text(heir.join(", "), 16.0, BONE));
                    }
                    if !d.trinkets.is_empty() {
                        col.spawn(Node {
                            column_gap: Val::Px(6.0),
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|r| {
                            r.spawn(kit.text("Trinkets:", 16.0, BONE));
                            for id in &d.trinkets {
                                trinket_slot(r, &mut kit, trinket(id), None, 40.0);
                            }
                        });
                    }
                    for l in &d.leveled {
                        col.spawn(kit.text(l, 16.0, HEAL));
                    }
                    for q in &d.quirks {
                        col.spawn(kit.text(q, 15.0, DIM));
                    }
                    for dname in &d.dead {
                        col.spawn(kit.title(format!("Lost: {dname}"), 22.0, BLOOD));
                    }
                    col.spawn(Node {
                        height: Val::Px(12.0),
                        ..default()
                    });
                    ui::button_sized(
                        col,
                        &kit,
                        "Return to the Hamlet",
                        Act::Home,
                        true,
                        false,
                        22.0,
                    );
                });
        });
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn act(
    mut commands: Commands,
    q: Query<(&Interaction, &Act, Option<&ui::BtnLook>), Changed<Interaction>>,
    game: Option<ResMut<Game>>,
    hamlet: Option<ResMut<HamletUi>>,
    pv: Option<ResMut<ProvisionUi>>,
    mut next: ResMut<NextState<Screen>>,
    mut narrator: ResMut<ui::Narrator>,
    mut sfx: ResMut<crate::audio::Sfx>,
) {
    let Some(mut game) = game else { return };
    let mut hamlet = hamlet;
    let mut pv = pv;
    for (i, a, look) in &q {
        if *i != Interaction::Pressed || look.is_some_and(|l| !l.enabled) {
            continue;
        }
        let c = &mut game.0;
        let sel = hamlet.as_ref().and_then(|h| h.selected);
        match a.clone() {
            Act::Close => {
                if let Some(h) = hamlet.as_mut() {
                    h.open = None;
                }
            }
            Act::Select(id) => {
                if let Some(h) = hamlet.as_mut() {
                    h.selected = Some(id);
                    if matches!(
                        h.open,
                        None | Some(Panel::Hero) | Some(Panel::Spot(Spot::Memorial))
                    ) {
                        h.open = Some(Panel::Hero);
                    }
                }
            }
            Act::Recruit(i) => {
                if c.recruit(i) {
                    narrator.say("Another fool for the dark.");
                    if let Some(h) = hamlet.as_mut() {
                        h.selected = c.roster.last().map(|h| h.id);
                    }
                }
            }
            Act::Relieve(r) => {
                if let Some(n) = sel.and_then(|id| c.relieve(id, r)) {
                    narrator.say(format!("A night's respite: {n} stress washed away."));
                }
            }
            Act::Treat(qid) => {
                if sel.is_some_and(|id| c.treat_quirk(id, &qid)) {
                    narrator.say("The physicians' work is crude, but it holds.");
                }
            }
            Act::Upgrade(weapon) => {
                if sel.is_some_and(|id| c.upgrade(id, weapon)) {
                    narrator.say("The smith's hammer rings late into the night.");
                    sfx.play("Sound\\Item\\Weapons\\Axe2H\\m2hAxeHitFlesh1a.wav", -8.0);
                }
            }
            Act::Train(s) => {
                if sel.is_some_and(|id| c.train_skill(id, &s)) {
                    narrator.say("Hours of drill, and the technique sharpens.");
                    sfx.play(crate::audio::VIRTUE, -10.0);
                }
            }
            Act::ToggleSkill(s) => {
                if let Some(h) = sel.and_then(|id| c.hero_mut(id)) {
                    h.toggle_skill(&s);
                }
            }
            Act::Learn(t) => {
                if let Some(h) = sel.and_then(|id| c.hero_mut(id)) {
                    if h.learn_talent(&t) {
                        narrator.say(format!("{} has grown in their craft.", h.name));
                        sfx.play(crate::audio::VIRTUE, -8.0);
                    }
                }
            }
            Act::ResetTalents => {
                if sel.is_some_and(|id| c.reset_talents(id)) {
                    narrator.say("Old habits unlearned, at a price.");
                }
            }
            Act::BuildingUp(b) => {
                if c.upgrade_building(b) {
                    narrator.say(format!(
                        "The {} is rebuilt, better than it ever was.",
                        b.name()
                    ));
                    sfx.play(crate::audio::VIRTUE, -6.0);
                }
            }
            Act::BuyTrinket(i) => {
                if c.buy_trinket(i) {
                    sfx.play(crate::audio::COIN, -6.0);
                }
            }
            Act::SellTrinket(i) => {
                if c.sell_trinket(i) {
                    sfx.play(crate::audio::COIN, -6.0);
                }
            }
            Act::Equip(slot, idx) => {
                if let Some(id) = sel {
                    if let Err(e) = c.equip_trinket(id, slot, idx) {
                        narrator.say(format!("That will not do: {e}."));
                    }
                }
            }
            Act::Unequip(slot) => {
                if let Some(id) = sel {
                    c.unequip_trinket(id, slot);
                }
            }
            Act::Dismiss => {
                if let Some(id) = sel {
                    c.dismiss(id);
                    narrator.say("They leave without a word, and are not missed.");
                    if let Some(h) = hamlet.as_mut() {
                        h.open = None;
                    }
                }
            }
            Act::Embark => next.set(Screen::Provision),
            Act::Region(r) => {
                if let Some(p) = pv.as_mut() {
                    p.region = r;
                    p.offer = c.offers.iter().position(|o| o.region == r);
                }
            }
            Act::Offer(i) => {
                if let Some(p) = pv.as_mut() {
                    p.offer = Some(i);
                }
            }
            Act::ToParty => {
                if let Some(p) = pv.as_mut() {
                    if let Some(o) = p.offer.and_then(|i| c.offers.get(i)).cloned() {
                        p.step = Step::Party;
                        p.pack = default_pack(o.length);
                        p.party = c
                            .roster
                            .iter()
                            .filter(|h| c.refusal(h, &o).is_none())
                            .take(4)
                            .map(|h| h.id)
                            .collect();
                    }
                }
            }
            Act::ToMap => {
                if let Some(p) = pv.as_mut() {
                    p.step = Step::Map;
                }
            }
            Act::PartyToggle(id) => {
                if let Some(p) = pv.as_mut() {
                    if let Some(pos) = p.party.iter().position(|x| *x == id) {
                        p.party.remove(pos);
                    } else if p.party.len() < 4 {
                        p.party.push(id);
                    }
                }
            }
            Act::Buy(item, n) => {
                if let Some(p) = pv.as_mut() {
                    let have = p.pack.get(&item).copied().unwrap_or(0);
                    if have + n >= 0 {
                        p.pack.insert(item, have + n);
                    }
                }
            }
            Act::Depart => {
                if let Some(p) = pv.as_mut() {
                    let cost: i32 = p.pack.iter().map(|(i, n)| i.price() * n).sum();
                    let offer = p.offer.and_then(|i| c.offers.get(i)).cloned();
                    if let (Some(o), true) = (offer, cost <= c.gold) {
                        let pack = std::mem::take(&mut p.pack);
                        if let Some(run) = c.embark(&p.party, pack, &o) {
                            c.gold -= cost;
                            save(c);
                            commands.insert_resource(ActiveOffer(Some(o)));
                            commands.insert_resource(Run(run));
                            next.set(Screen::Dungeon);
                        }
                    }
                }
            }
            Act::Back => {
                if pv.is_some() {
                    next.set(Screen::Hamlet);
                }
            }
            Act::Noop => {}
            Act::Home => {
                save(c);
                next.set(Screen::Hamlet);
            }
        }
        if let Some(h) = hamlet.as_mut() {
            h.dirty = true;
        }
        if let Some(p) = pv.as_mut() {
            p.dirty = true;
        }
    }
}

/// `CRAWL_AUTOPLAY=1` in the hamlet: spend, train, equip and embark unattended, so a whole
/// campaign loop runs without a hand on the mouse.
#[allow(clippy::too_many_arguments)]
fn autoplay_town(
    time: Res<Time>,
    state: Res<State<Screen>>,
    mut wait: Local<f32>,
    game: Option<ResMut<Game>>,
    pv: Option<ResMut<ProvisionUi>>,
    mut commands: Commands,
    mut next: ResMut<NextState<Screen>>,
    mut ui_state: Option<ResMut<HamletUi>>,
) {
    if std::env::var("CRAWL_AUTOPLAY").is_err() {
        return;
    }
    let Some(mut game) = game else { return };
    *wait += time.delta_secs();
    if *wait < 2.5 {
        return;
    }
    *wait = 0.0;
    let c = &mut game.0;
    match state.get() {
        Screen::Hamlet => {
            while c.roster.len() < 4 && c.recruit(0) {}
            for b in Building::ALL {
                c.upgrade_building(b);
            }
            let ids: Vec<u32> = c.roster.iter().map(|h| h.id).collect();
            for id in &ids {
                let skills: Vec<String> = c
                    .roster
                    .iter()
                    .find(|h| h.id == *id)
                    .map(|h| h.skills.clone())
                    .unwrap_or_default();
                for s in skills {
                    c.train_skill(*id, &s);
                }
                c.upgrade(*id, true);
                c.upgrade(*id, false);
                if let Some(h) = c.hero_mut(*id) {
                    for tier in h.open_talent_tiers() {
                        let t = tree(h.class)[tier][0].id;
                        h.learn_talent(t);
                    }
                    if h.stress > 60 {
                        let _ = h;
                        c.relieve(*id, Relief::Tavern);
                    }
                }
                for slot in 0..2 {
                    if !c.trinkets.is_empty()
                        && c.roster
                            .iter()
                            .find(|h| h.id == *id)
                            .is_some_and(|h| h.trinkets[slot].is_none())
                    {
                        let _ = c.equip_trinket(*id, slot, 0);
                    }
                }
            }
            if let Some(u) = ui_state.as_mut() {
                u.dirty = true;
            }
            next.set(Screen::Provision);
        }
        Screen::Provision => {
            let Some(mut p) = pv else { return };
            let pick = c
                .offers
                .iter()
                .position(|o| o.kind == crate::rules::regions::QuestKind::Boss)
                .or_else(|| {
                    if c.offers.is_empty() {
                        None
                    } else {
                        Some(c.runs as usize % c.offers.len())
                    }
                });
            let Some(i) = pick else { return };
            let o = c.offers[i].clone();
            let party: Vec<u32> = c
                .roster
                .iter()
                .filter(|h| c.refusal(h, &o).is_none())
                .take(4)
                .map(|h| h.id)
                .collect();
            if party.is_empty() {
                c.end_week();
                next.set(Screen::Hamlet);
                return;
            }
            let mut pack = default_pack(o.length);
            let cost: i32 = pack.iter().map(|(i, n)| i.price() * n).sum();
            if cost > c.gold {
                pack = HashMap::from([(Item::Food, 4), (Item::Torch, 4)]);
            }
            let cost: i32 = pack.iter().map(|(i, n)| i.price() * n).sum();
            if let Some(run) = c.embark(&party, pack, &o) {
                c.gold -= cost.min(c.gold);
                commands.insert_resource(ActiveOffer(Some(o)));
                commands.insert_resource(Run(run));
                p.dirty = true;
                next.set(Screen::Dungeon);
            }
        }
        Screen::Debrief => next.set(Screen::Hamlet),
        _ => {}
    }
}
