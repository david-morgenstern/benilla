//! The hamlet between expeditions, the provisioner before one, and the reckoning after.

use std::collections::HashMap;

use bevy::prelude::*;

use super::looks::{class_icon_rect, CLASS_ATLAS};
use super::ui::{self, Kit, Tip, BLOOD, BONE, DIM, EDGE, GOLD, HEAL, PANEL, PANEL_LIGHT};
use super::{save, Game, LastDebrief, Run, Scoped, Screen};
use crate::rules::combat::bonus_label;
use crate::rules::data::{has_rank, SkillDef, Target};
use crate::rules::dungeon::{Heirloom, Item, Quest};
use crate::rules::hero::{quirk, Hero, LEVEL_XP, MAX_LEVEL};
use crate::rules::town::{default_pack, Relief, ROSTER_MAX};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Building {
    Stagecoach,
    Tavern,
    Chapel,
    Sanitarium,
    Blacksmith,
    Training,
    Memorial,
}

impl Building {
    const ALL: [Building; 7] = [
        Building::Stagecoach,
        Building::Tavern,
        Building::Chapel,
        Building::Sanitarium,
        Building::Blacksmith,
        Building::Training,
        Building::Memorial,
    ];

    fn name(self) -> &'static str {
        match self {
            Building::Stagecoach => "Stagecoach",
            Building::Tavern => "Tavern",
            Building::Chapel => "Chapel",
            Building::Sanitarium => "Sanitarium",
            Building::Blacksmith => "Blacksmith",
            Building::Training => "Training Hall",
            Building::Memorial => "Memorial",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Building::Stagecoach => "Ability_Mount_RidingHorse",
            Building::Tavern => "INV_Drink_04",
            Building::Chapel => "Spell_Holy_PrayerOfHealing02",
            Building::Sanitarium => "Spell_Holy_DispelMagic",
            Building::Blacksmith => "Trade_BlackSmithing",
            Building::Training => "INV_Misc_Book_09",
            Building::Memorial => "INV_Misc_Bone_HumanSkull_01",
        }
    }
}

#[derive(Resource)]
struct HamletUi {
    building: Building,
    selected: Option<u32>,
    dirty: bool,
}

#[derive(Resource)]
struct ProvisionUi {
    party: Vec<u32>,
    pack: HashMap<Item, i32>,
    quest: Quest,
    dirty: bool,
}

#[derive(Component, Clone)]
enum Act {
    Building(Building),
    Select(u32),
    Recruit(usize),
    Relieve(Relief),
    Treat(String),
    Upgrade(bool),
    ToggleSkill(String),
    Dismiss,
    Prepare,
    // Provisioning.
    PartyToggle(u32),
    Quest(Quest),
    Buy(Item, i32),
    Embark,
    Back,
    // Debrief.
    Home,
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
                    act,
                    rebuild_hamlet.run_if(in_state(Screen::Hamlet)),
                    rebuild_provision.run_if(in_state(Screen::Provision)),
                )
                    .chain(),
            );
    }
}

fn enter_hamlet(mut commands: Commands, game: Option<Res<Game>>) {
    let selected = game.and_then(|g| g.0.roster.first().map(|h| h.id));
    commands.insert_resource(HamletUi {
        building: Building::Stagecoach,
        selected,
        dirty: true,
    });
}

fn enter_provision(mut commands: Commands, game: Res<Game>) {
    let party: Vec<u32> = game
        .0
        .roster
        .iter()
        .filter(|h| !h.resting)
        .take(4)
        .map(|h| h.id)
        .collect();
    let quest = if game.0.runs >= 2 && !game.0.boss_slain {
        Quest::Boss
    } else {
        Quest::Explore(90)
    };
    commands.insert_resource(ProvisionUi {
        party,
        pack: default_pack(),
        quest,
        dirty: true,
    });
}

fn root(
    commands: &mut Commands,
    kit: &mut Kit,
    screen: Screen,
    bg: &str,
    old: &Query<Entity, With<UiRoot>>,
) -> Entity {
    for e in old {
        commands.entity(e).despawn();
    }
    let bg = kit.image(bg);
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(10.0),
                ..default()
            },
            ImageNode {
                image: bg,
                color: Color::srgb(0.28, 0.25, 0.25),
                ..default()
            },
            Scoped(screen),
            UiRoot,
        ))
        .id()
}

fn heirloom_strip(
    p: &mut ChildSpawnerCommands,
    kit: &mut Kit,
    gold: i32,
    heirlooms: &dyn Fn(Heirloom) -> i32,
) {
    p.spawn(Node {
        column_gap: Val::Px(16.0),
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
        for h in [
            Heirloom::Bust,
            Heirloom::Portrait,
            Heirloom::Deed,
            Heirloom::Crest,
        ] {
            let icon = kit.icon(h.icon());
            row.spawn((
                ImageNode::new(icon),
                Node {
                    width: Val::Px(26.0),
                    height: Val::Px(26.0),
                    ..default()
                },
                Interaction::default(),
                Tip(h.name().to_owned()),
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

fn hero_card(p: &mut ChildSpawnerCommands, kit: &mut Kit, hero: &Hero, act: Act, selected: bool) {
    p.spawn((
        Button,
        Node {
            width: Val::Px(330.0),
            padding: UiRect::all(Val::Px(5.0)),
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
        act,
    ))
    .with_children(|c| {
        class_icon(c, kit, hero, 44.0);
        c.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|col| {
            col.spawn(kit.title(hero.name.clone(), 19.0, BONE));
            let status = if hero.resting {
                " — resting".to_owned()
            } else if let Some(a) = hero.affliction {
                format!(" — {}", a.name())
            } else {
                String::new()
            };
            col.spawn(kit.text(
                format!("Lv {} {}{}", hero.level, hero.class_name(), status),
                13.0,
                if hero.affliction.is_some() {
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
                ui::bar(r, 90.0, 8.0, hero.hp as f32 / hero.max_hp() as f32, BLOOD);
                ui::stress_pips(r, hero.stress, 9.0);
            });
        });
    });
}

fn skill_tip(s: &SkillDef) -> String {
    let dots = |mask: u8| -> String {
        (1..=4)
            .rev()
            .map(|r| if has_rank(mask, r) { '●' } else { '○' })
            .collect()
    };
    let mut t = format!("{}\n{}\n\nUse from: {}", s.name, s.desc, dots(s.launch));
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
    if s.target.is_enemy() && s.dmg_mod > -100 {
        t += &format!(
            "\nAccuracy {}  Damage {:+}%  Crit {:+}%",
            s.acc, s.dmg_mod, s.crit_mod
        );
    }
    if let Some((lo, hi)) = s.heal {
        t += &format!("\nHeals {lo}–{hi}");
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
    skill_tip(s)
}

fn hero_summary(p: &mut ChildSpawnerCommands, kit: &mut Kit, h: &Hero, portrait: Handle<Image>) {
    let st = h.stats();
    p.spawn(Node {
        column_gap: Val::Px(12.0),
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|r| {
        r.spawn((
            Node {
                width: Val::Px(120.0),
                height: Val::Px(120.0),
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
        class_icon(r, kit, h, 48.0);
        r.spawn(Node {
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|c| {
            c.spawn(kit.title(format!("{} the {}", h.name, h.class_name()), 28.0, GOLD));
            let next = if h.level < MAX_LEVEL {
                format!("{}/{} xp", h.xp, LEVEL_XP[usize::from(h.level + 1)])
            } else {
                "max".to_owned()
            };
            c.spawn(kit.text(
                format!(
                    "Resolve {}  ({next})   Weapon {}  Armour {}   Expeditions {}  Kills {}",
                    h.level, h.weapon, h.armor, h.expeditions, h.kills
                ),
                14.0,
                DIM,
            ));
        });
    });
    p.spawn(kit.text(
        format!(
            "HP {}/{}   Stress {}   ACC {:+}  CRIT {}%  DMG {}–{}  DODGE {}  PROT {}%  SPD {}",
            h.hp,
            h.max_hp(),
            h.stress,
            st.acc,
            st.crit,
            st.dmg.0,
            st.dmg.1,
            st.dodge,
            st.prot,
            st.speed
        ),
        15.0,
        BONE,
    ));
    let quirks: Vec<String> = h
        .quirks
        .iter()
        .filter_map(|q| quirk(q))
        .map(|q| format!("{}{}", if q.positive { "+" } else { "−" }, q.name))
        .collect();
    p.spawn(kit.text(
        format!(
            "Quirks: {}",
            if quirks.is_empty() {
                "none".into()
            } else {
                quirks.join(", ")
            }
        ),
        14.0,
        DIM,
    ));
    if let Some(a) = h.affliction {
        p.spawn(kit.title(format!("Afflicted: {}", a.name()), 20.0, BLOOD));
    }
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
    let c = &game.0;
    if ui_state
        .selected
        .is_none_or(|id| !c.roster.iter().any(|h| h.id == id))
    {
        ui_state.selected = c.roster.first().map(|h| h.id);
    }
    portrait.want = ui_state.selected;
    let root = root(
        &mut commands,
        &mut kit,
        Screen::Hamlet,
        "Interface\\Glues\\LoadingScreens\\LoadScreenShadowFangKeep",
        &old,
    );
    let sel = ui_state
        .selected
        .and_then(|id| c.roster.iter().find(|h| h.id == id));
    commands.entity(root).with_children(|p| {
        // Top bar.
        p.spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            width: Val::Percent(100.0),
            ..default()
        })
        .with_children(|top| {
            top.spawn((
                kit.title(format!("The Hamlet — Week {}", c.week), 40.0, GOLD),
                TextShadow::default(),
            ));
            heirloom_strip(top, &mut kit, c.gold, &|h| c.heirloom(h));
        });
        p.spawn(Node {
            flex_grow: 1.0,
            column_gap: Val::Px(14.0),
            ..default()
        })
        .with_children(|main| {
            // Buildings.
            main.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                width: Val::Px(220.0),
                ..default()
            })
            .with_children(|col| {
                for b in Building::ALL {
                    let icon = kit.icon(b.icon());
                    let selected = ui_state.building == b;
                    col.spawn((
                        Button,
                        Node {
                            padding: UiRect::all(Val::Px(6.0)),
                            column_gap: Val::Px(10.0),
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(if selected { ui::BTN_DOWN } else { ui::BTN }),
                        BorderColor::all(if selected { GOLD } else { EDGE }),
                        ui::BtnLook {
                            enabled: true,
                            selected,
                        },
                        Act::Building(b),
                    ))
                    .with_children(|bt| {
                        bt.spawn((
                            ImageNode::new(icon),
                            Node {
                                width: Val::Px(36.0),
                                height: Val::Px(36.0),
                                ..default()
                            },
                        ));
                        bt.spawn(kit.title(b.name(), 20.0, BONE));
                    });
                }
                col.spawn(Node {
                    height: Val::Px(30.0),
                    ..default()
                });
                ui::button_sized(
                    col,
                    &kit,
                    "Prepare Expedition",
                    Act::Prepare,
                    c.roster.iter().any(|h| !h.resting),
                    false,
                    22.0,
                );
            });
            // Centre: the selected hero and the building's business.
            main.spawn(ui::panel(Val::Auto, Val::Percent(100.0)))
                .insert(Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(14.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    row_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|mid| {
                    if let Some(h) = sel {
                        hero_summary(mid, &mut kit, h, portrait.image.clone());
                        mid.spawn(Node {
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|r| {
                            ui::button(r, &kit, "Dismiss", Act::Dismiss, c.roster.len() > 1);
                        });
                    } else {
                        mid.spawn(kit.text(
                            "Your roster is empty. Hire at the stagecoach.",
                            18.0,
                            BONE,
                        ));
                    }
                    mid.spawn((
                        Node {
                            height: Val::Px(2.0),
                            width: Val::Percent(100.0),
                            margin: UiRect::vertical(Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(EDGE),
                    ));
                    mid.spawn(kit.title(ui_state.building.name(), 30.0, GOLD));
                    building_panel(mid, &mut kit, ui_state.building, c, sel);
                });
            // Roster.
            main.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                width: Val::Px(340.0),
                ..default()
            })
            .with_children(|col| {
                col.spawn(kit.title(
                    format!("Roster ({}/{ROSTER_MAX})", c.roster.len()),
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
                    );
                }
            });
        });
    });
}

fn building_panel(
    p: &mut ChildSpawnerCommands,
    kit: &mut Kit,
    b: Building,
    c: &crate::rules::town::Campaign,
    sel: Option<&Hero>,
) {
    match b {
        Building::Stagecoach => {
            p.spawn(kit.text(
                "Fresh faces off the coach road, each sure they will be the one to come back rich.",
                15.0,
                DIM,
            ));
            for (i, h) in c.stagecoach.iter().enumerate() {
                p.spawn(Node {
                    column_gap: Val::Px(10.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|r| {
                    class_icon(r, kit, h, 40.0);
                    let quirks: Vec<&str> = h
                        .quirks
                        .iter()
                        .filter_map(|q| quirk(q))
                        .map(|q| q.name)
                        .collect();
                    r.spawn(kit.text(
                        format!("{} the {} — {}", h.name, h.class_name(), quirks.join(", ")),
                        16.0,
                        BONE,
                    ));
                    ui::button(r, kit, "Hire", Act::Recruit(i), c.roster.len() < ROSTER_MAX);
                });
            }
            p.spawn(kit.text(
                format!("Class: {}", sel.map_or("", |h| h.class.def().blurb)),
                14.0,
                DIM,
            ));
        }
        Building::Tavern | Building::Chapel => {
            let relief = if b == Building::Tavern {
                Relief::Tavern
            } else {
                Relief::Chapel
            };
            p.spawn(kit.text(relief.name(), 18.0, BONE));
            let (lo, hi) = relief.amount();
            p.spawn(kit.text(
                format!(
                    "{}\nRemoves {lo}–{hi} stress and breaks an affliction. The hero sits out the next expedition. Cost: {} gold.",
                    if b == Building::Tavern {
                        "Cheap ale, loaded dice, and a fire that never quite warms."
                    } else {
                        "Candles, silence, and a priest who has stopped asking questions."
                    },
                    relief.cost()
                ),
                15.0,
                DIM,
            ));
            if let Some(h) = sel {
                let ok = !h.resting
                    && (h.stress > 0 || h.affliction.is_some())
                    && c.gold >= relief.cost();
                ui::button(
                    p,
                    kit,
                    &format!("Send {}", h.name),
                    Act::Relieve(relief),
                    ok,
                );
            }
        }
        Building::Sanitarium => {
            p.spawn(kit.text(
                "The physicians can cut a bad habit out of a hero. 500 gold, and a week abed.",
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
                        &format!("Treat {} (500g)", q.name),
                        Act::Treat(q.id.to_owned()),
                        !h.resting && c.gold >= 500,
                    );
                }
            }
        }
        Building::Blacksmith => {
            p.spawn(kit.text("Better steel, for gold and the crests of old houses. A hero's gear cannot outpace their resolve.", 15.0, DIM));
            if let Some(h) = sel {
                for weapon in [true, false] {
                    let tier = if weapon { h.weapon } else { h.armor };
                    let what = if weapon { "Weapon" } else { "Armour" };
                    if tier >= 5 {
                        p.spawn(kit.text(format!("{what}: tier 5 (mastered)"), 16.0, BONE));
                        continue;
                    }
                    let gold = Hero::upgrade_cost(tier);
                    let crests = i32::from(tier) - 1;
                    let ok = c.gold >= gold
                        && c.heirloom(Heirloom::Crest) >= crests
                        && tier <= h.level + 1;
                    let label = format!(
                        "Upgrade {what} to tier {} — {gold}g, {crests} crests{}",
                        tier + 1,
                        if tier > h.level + 1 {
                            " (needs resolve)"
                        } else {
                            ""
                        }
                    );
                    ui::button(p, kit, &label, Act::Upgrade(weapon), ok);
                }
            }
        }
        Building::Training => {
            p.spawn(kit.text(
                "Choose the four skills this hero carries into the dark.",
                15.0,
                DIM,
            ));
            if let Some(h) = sel {
                p.spawn(Node {
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|r| {
                    for s in h.class.def().skills {
                        let on = h.skills.iter().any(|x| x == s.id);
                        let icon = kit.icon(s.icon);
                        ui::icon_button(
                            r,
                            icon,
                            56.0,
                            Act::ToggleSkill(s.id.to_owned()),
                            true,
                            on,
                            Some(skill_tip(s)),
                        );
                    }
                });
                let names: Vec<&str> = h.equipped_skills().iter().map(|s| s.name).collect();
                p.spawn(kit.text(format!("Equipped: {}", names.join(", ")), 15.0, BONE));
            }
        }
        Building::Memorial => {
            if c.graveyard.is_empty() {
                p.spawn(kit.text("No names on the stone. Not yet.", 16.0, DIM));
            }
            for g in c.graveyard.iter().rev().take(14) {
                p.spawn(kit.text(
                    format!(
                        "{} the {} (Lv {}) — {}, week {}",
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
    }
}

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
    let c = &game.0;
    let root = root(
        &mut commands,
        &mut kit,
        Screen::Provision,
        "Interface\\Glues\\LoadingScreens\\LoadScreenDungeon",
        &old,
    );
    commands.entity(root).with_children(|p| {
        p.spawn(Node { justify_content: JustifyContent::SpaceBetween, width: Val::Percent(100.0), ..default() }).with_children(|top| {
            top.spawn(kit.title("Prepare the Expedition", 40.0, GOLD));
            heirloom_strip(top, &mut kit, c.gold, &|h| c.heirloom(h));
        });
        p.spawn(Node { column_gap: Val::Px(16.0), flex_grow: 1.0, ..default() }).with_children(|main| {
            // Party and quest.
            main.spawn(ui::panel(Val::Px(720.0), Val::Percent(100.0))).with_children(|col| {
                col.spawn(kit.title("The Sunken Crypts", 28.0, BONE));
                for q in [Quest::Explore(90), Quest::Boss] {
                    let label = match q {
                        Quest::Explore(_) => "Short: Explore 90% of the rooms",
                        Quest::Boss => "Long: Hunt the Pale Archivist in its lair",
                    };
                    ui::button_sized(col, &kit, label, Act::Quest(q), true, pv.quest == q, 16.0);
                }
                col.spawn(Node { height: Val::Px(10.0), ..default() });
                col.spawn(kit.title("Party (front rank first)", 22.0, BONE));
                for (i, id) in pv.party.iter().enumerate() {
                    if let Some(h) = c.roster.iter().find(|h| h.id == *id) {
                        col.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
                            r.spawn(kit.title(format!("Rank {}", i + 1), 18.0, GOLD));
                            hero_card(r, &mut kit, h, Act::PartyToggle(h.id), true);
                        });
                    }
                }
                col.spawn(kit.text("Click a hero to add or remove them. Order matters: rank 1 stands at the front.", 13.0, DIM));
                col.spawn(Node { height: Val::Px(8.0), ..default() });
                col.spawn(kit.title("Available", 20.0, BONE));
                col.spawn(Node { flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(6.0), row_gap: Val::Px(4.0), ..default() }).with_children(|w| {
                    for h in c.roster.iter().filter(|h| !pv.party.contains(&h.id)) {
                        if h.resting {
                            continue;
                        }
                        hero_card(w, &mut kit, h, Act::PartyToggle(h.id), false);
                    }
                });
            });
            // The provisioner.
            main.spawn(ui::panel(Val::Auto, Val::Percent(100.0))).insert(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), padding: UiRect::all(Val::Px(12.0)), border: UiRect::all(Val::Px(2.0)), ..default() })
                .with_children(|col| {
                    col.spawn(kit.title("Provisioner", 28.0, BONE));
                    let mut total = 0;
                    for item in Item::ALL {
                        let n = pv.pack.get(&item).copied().unwrap_or(0);
                        total += n * item.price();
                        col.spawn((Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }, Interaction::default(), Tip(item.desc().to_owned())))
                            .with_children(|r| {
                                let icon = kit.icon(item.icon());
                                r.spawn((ImageNode::new(icon), Node { width: Val::Px(36.0), height: Val::Px(36.0), ..default() }));
                                r.spawn((kit.text(format!("{} ({}g)", item.name(), item.price()), 16.0, BONE), Node { width: Val::Px(230.0), ..default() }));
                                ui::button(r, &kit, "−", Act::Buy(item, -1), n > 0);
                                r.spawn((kit.title(format!("{n}"), 22.0, GOLD), Node { width: Val::Px(36.0), justify_content: JustifyContent::Center, ..default() }));
                                ui::button(r, &kit, "+", Act::Buy(item, 1), true);
                            });
                    }
                    col.spawn(kit.text(format!("Pack cost: {total} gold (of {})", c.gold), 16.0, if total > c.gold { BLOOD } else { BONE }));
                    col.spawn(Node { flex_grow: 1.0, ..default() });
                    col.spawn(Node { column_gap: Val::Px(10.0), ..default() }).with_children(|r| {
                        ui::button_sized(r, &kit, "Back to the Hamlet", Act::Back, true, false, 20.0);
                        ui::button_sized(r, &kit, "Embark", Act::Embark, !pv.party.is_empty() && total <= c.gold, false, 26.0);
                    });
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
    let root = root(
        &mut commands,
        &mut kit,
        Screen::Debrief,
        "Interface\\Glues\\LoadingScreens\\LoadScreenScholomance",
        &old,
    );
    let Some(d) = &last.0 else {
        return;
    };
    commands.entity(root).insert(Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: Val::Px(10.0),
        ..default()
    });
    commands.entity(root).with_children(|p| {
        p.spawn(ui::panel(Val::Px(820.0), Val::Auto))
            .with_children(|col| {
                col.spawn(kit.title(
                    if d.success {
                        "The Expedition Returns Triumphant"
                    } else {
                        "The Expedition Returns, Broken"
                    },
                    40.0,
                    if d.success { GOLD } else { BLOOD },
                ));
                col.spawn(kit.text(format!("Gold recovered: {}", d.gold), 20.0, GOLD));
                for (h, n) in &d.heirlooms {
                    col.spawn(kit.text(format!("{} × {n}", h.name()), 17.0, BONE));
                }
                for l in &d.leveled {
                    col.spawn(kit.text(l, 17.0, HEAL));
                }
                for q in &d.quirks {
                    col.spawn(kit.text(q, 16.0, DIM));
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
    let _ = PANEL;
    let _ = PANEL_LIGHT;
}

#[allow(clippy::too_many_arguments)]
fn act(
    mut commands: Commands,
    q: Query<(&Interaction, &Act, Option<&ui::BtnLook>), Changed<Interaction>>,
    game: Option<ResMut<Game>>,
    hamlet: Option<ResMut<HamletUi>>,
    pv: Option<ResMut<ProvisionUi>>,
    mut next: ResMut<NextState<Screen>>,
    mut narrator: ResMut<ui::Narrator>,
) {
    let Some(mut game) = game else {
        return;
    };
    let mut hamlet = hamlet;
    let mut pv = pv;
    for (i, a, look) in &q {
        if *i != Interaction::Pressed || look.is_some_and(|l| !l.enabled) {
            continue;
        }
        let c = &mut game.0;
        let sel = hamlet.as_ref().and_then(|h| h.selected);
        match a.clone() {
            Act::Building(b) => {
                if let Some(h) = hamlet.as_mut() {
                    h.building = b;
                }
            }
            Act::Select(id) => {
                if let Some(h) = hamlet.as_mut() {
                    h.selected = Some(id);
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
                if let Some(id) = sel {
                    if let Some(n) = c.relieve(id, r) {
                        narrator.say(format!("A night's respite: {n} stress washed away."));
                    }
                }
            }
            Act::Treat(qid) => {
                if let Some(id) = sel {
                    if c.treat_quirk(id, &qid) {
                        narrator.say("The physicians' work is crude, but it holds.");
                    }
                }
            }
            Act::Upgrade(weapon) => {
                if let Some(id) = sel {
                    if c.upgrade(id, weapon) {
                        narrator.say("The smith's hammer rings late into the night.");
                    }
                }
            }
            Act::ToggleSkill(s) => {
                if let Some(id) = sel {
                    if let Some(h) = c.hero_mut(id) {
                        h.toggle_skill(&s);
                    }
                }
            }
            Act::Dismiss => {
                if let Some(id) = sel {
                    c.dismiss(id);
                    narrator.say("They leave without a word, and are not missed.");
                }
            }
            Act::Prepare => next.set(Screen::Provision),
            Act::PartyToggle(id) => {
                if let Some(p) = pv.as_mut() {
                    if let Some(pos) = p.party.iter().position(|x| *x == id) {
                        p.party.remove(pos);
                    } else if p.party.len() < 4 {
                        p.party.push(id);
                    }
                }
            }
            Act::Quest(q) => {
                if let Some(p) = pv.as_mut() {
                    p.quest = q;
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
            Act::Embark => {
                if let Some(p) = pv.as_mut() {
                    let cost: i32 = p.pack.iter().map(|(i, n)| i.price() * n).sum();
                    if cost <= c.gold {
                        let pack = std::mem::take(&mut p.pack);
                        if let Some(run) = c.embark(&p.party, pack, p.quest) {
                            c.gold -= cost;
                            save(c);
                            commands.insert_resource(Run(run));
                            next.set(Screen::Dungeon);
                        }
                    }
                }
            }
            Act::Back => next.set(Screen::Hamlet),
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
