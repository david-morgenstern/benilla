//! The regions beneath the hamlet, how long an expedition runs, what it is for, and the denizens
//! of the two newer depths.

use serde::{Deserialize, Serialize};

use super::data::{
    anim, mres, mskill, stats, Effect, MonsterDef, SkillDef, SkillKind, Stat, Tag, Target,
    CRYPT_BOSS_PACK, CRYPT_HALL_PACKS, CRYPT_PACKS, R12, R1234, R234, R34,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Region {
    Crypts,
    Warrens,
    Grotto,
}

impl Region {
    pub const ALL: [Region; 3] = [Region::Crypts, Region::Warrens, Region::Grotto];

    pub fn name(self) -> &'static str {
        match self {
            Region::Crypts => "The Sunken Crypts",
            Region::Warrens => "The Kobold Warrens",
            Region::Grotto => "The Drowned Grotto",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Region::Crypts => "The family dead were never meant to rise. They rose anyway, and brought friends.",
            Region::Warrens => "Old silver mines under the estate, overrun by candle-bearing vermin who want them back.",
            Region::Grotto => "Where the estate's cellars meet the sea, something sings beneath the water.",
        }
    }

    pub fn packs(self) -> &'static [&'static [&'static str]] {
        match self {
            Region::Crypts => CRYPT_PACKS,
            Region::Warrens => WARREN_PACKS,
            Region::Grotto => GROTTO_PACKS,
        }
    }

    pub fn hall_packs(self) -> &'static [&'static [&'static str]] {
        match self {
            Region::Crypts => CRYPT_HALL_PACKS,
            Region::Warrens => WARREN_HALL_PACKS,
            Region::Grotto => GROTTO_HALL_PACKS,
        }
    }

    pub fn boss_pack(self) -> &'static [&'static str] {
        match self {
            Region::Crypts => CRYPT_BOSS_PACK,
            Region::Warrens => &["kobold_digger", "broodmother", "rat"],
            Region::Grotto => &["crab", "matriarch", "murloc_oracle"],
        }
    }

    pub fn boss_name(self) -> &'static str {
        match self {
            Region::Crypts => "the Pale Archivist",
            Region::Warrens => "the Broodmother of Stone",
            Region::Grotto => "the Abyssal Matriarch",
        }
    }

    /// The curios found here, by id.
    pub fn curios(self) -> &'static [&'static str] {
        match self {
            Region::Crypts => &[
                "sarcophagus",
                "altar",
                "bookshelf",
                "cauldron",
                "brazier",
                "portrait",
                "chest",
            ],
            Region::Warrens => &[
                "minecart",
                "ore_vein",
                "kobold_cache",
                "barrel",
                "chest",
                "brazier",
                "cauldron",
            ],
            Region::Grotto => &[
                "clam",
                "shipwreck_chest",
                "tide_shrine",
                "barrel",
                "chest",
                "portrait",
                "altar",
            ],
        }
    }

    /// The quest curio a gather quest asks for.
    pub fn quest_curio(self) -> &'static str {
        match self {
            Region::Crypts => "reliquary",
            Region::Warrens => "silver_cache",
            Region::Grotto => "tide_idol",
        }
    }

    pub fn delve_name(self) -> &'static str {
        match self {
            Region::Crypts => "reliquaries",
            Region::Warrens => "silver caches",
            Region::Grotto => "drowned idols",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Length {
    Short,
    Medium,
    Long,
}

impl Length {
    pub fn name(self) -> &'static str {
        match self {
            Length::Short => "Short",
            Length::Medium => "Medium",
            Length::Long => "Long",
        }
    }

    pub fn rooms(self) -> usize {
        match self {
            Length::Short => 9,
            Length::Medium => 14,
            Length::Long => 19,
        }
    }

    /// Bundles of firewood the party is issued: one camp each.
    pub fn firewood(self) -> u8 {
        match self {
            Length::Short => 0,
            Length::Medium => 1,
            Length::Long => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuestKind {
    /// Explore this percent of the rooms.
    Explore,
    /// Win every room battle.
    Cleanse,
    /// Recover this many quest curios.
    Gather,
    /// Slay the region's boss.
    Boss,
}

impl QuestKind {
    pub fn name(self) -> &'static str {
        match self {
            QuestKind::Explore => "Explore",
            QuestKind::Cleanse => "Cleanse",
            QuestKind::Gather => "Gather",
            QuestKind::Boss => "Boss",
        }
    }
}

/// The dungeon's tier: Apprentice, Veteran or Champion.
pub fn level_name(level: u8) -> &'static str {
    match level {
        1 => "Apprentice",
        2 => "Veteran",
        _ => "Champion",
    }
}

/// The highest resolve a hero may have and still agree to a dungeon of this level; veterans
/// refuse to waste themselves on apprentice work.
pub fn max_resolve_for(level: u8) -> u8 {
    match level {
        1 => 2,
        2 => 4,
        _ => 6,
    }
}

/// One expedition on offer this week.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuestOffer {
    pub region: Region,
    pub length: Length,
    pub kind: QuestKind,
    pub level: u8,
    pub reward_gold: i32,
    pub reward_heirlooms: i32,
    /// A trinket is promised on success.
    pub reward_trinket: bool,
}

impl QuestOffer {
    pub fn title(&self) -> String {
        format!(
            "{} {}: {}",
            level_name(self.level),
            self.length.name(),
            self.goal()
        )
    }

    pub fn goal(&self) -> String {
        match self.kind {
            QuestKind::Explore => "Explore 90% of rooms".to_owned(),
            QuestKind::Cleanse => "Cleanse all battle rooms".to_owned(),
            QuestKind::Gather => format!(
                "Recover {} {}",
                gather_target(self.length),
                self.region.delve_name()
            ),
            QuestKind::Boss => format!("Slay {}", self.region.boss_name()),
        }
    }
}

pub fn gather_target(length: Length) -> u32 {
    match length {
        Length::Short => 3,
        Length::Medium => 4,
        Length::Long => 5,
    }
}

// --------------------------------------------------------------------------- the new denizens

pub static WARREN_PACKS: &[&[&str]] = &[
    &["kobold_digger", "kobold_digger", "kobold_geomancer"],
    &["rat", "rat", "kobold_digger", "kobold_geomancer"],
    &["kobold_taskmaster", "kobold_digger", "cave_creeper"],
    &["cave_creeper", "rat", "gut_worm"],
    &[
        "kobold_digger",
        "kobold_taskmaster",
        "kobold_geomancer",
        "kobold_geomancer",
    ],
    &["rat", "cave_creeper", "cave_creeper", "gut_worm"],
];
pub static WARREN_HALL_PACKS: &[&[&str]] = &[
    &["rat", "rat"],
    &["kobold_digger", "kobold_geomancer"],
    &["cave_creeper", "gut_worm"],
];
pub static GROTTO_PACKS: &[&[&str]] = &[
    &["murloc_raider", "murloc_raider", "murloc_oracle"],
    &["crab", "murloc_raider", "siren"],
    &["myrmidon", "murloc_raider", "murloc_oracle", "siren"],
    &["lobstrok", "siren", "murloc_oracle"],
    &["crab", "myrmidon", "siren", "siren"],
    &["murloc_raider", "lobstrok", "murloc_oracle"],
];
pub static GROTTO_HALL_PACKS: &[&[&str]] = &[
    &["murloc_raider", "murloc_oracle"],
    &["crab", "siren"],
    &["murloc_raider", "murloc_raider"],
];

const fn m(id: &'static str, name: &'static str) -> SkillDef {
    mskill(id, name)
}

pub static MONSTERS: &[MonsterDef] = &[
    MonsterDef {
        id: "kobold_digger",
        name: "Tunnel Digger",
        tags: &[Tag::Kobold, Tag::Humanoid],
        base: stats(13, 10, 5, 4, 5, (3, 6), mres(20, 30, 20, 20, 25)),
        size: 1,
        display: 26,
        height: 1.4,
        boss: false,
        skills: &[
            SkillDef {
                launch: R12,
                target: Target::Enemy(R12),
                weight: 3,
                anim: anim::ATTACK_1H,
                ..m("pick_swing", "Pick Swing")
            },
            SkillDef {
                launch: R12,
                target: Target::Enemy(R12),
                dmg_mod: -40,
                effects: &[Effect::Bleed {
                    dmg: 2,
                    turns: 3,
                    chance: 100,
                }],
                weight: 2,
                anim: anim::ATTACK_1H,
                ..m("rusty_shiv", "Rusty Shiv")
            },
        ],
    },
    MonsterDef {
        id: "kobold_geomancer",
        name: "Candle-Wick Geomancer",
        tags: &[Tag::Kobold, Tag::Humanoid],
        base: stats(11, 10, 0, 5, 6, (3, 6), mres(20, 30, 20, 30, 20)),
        size: 1,
        display: 163,
        height: 1.35,
        boss: false,
        skills: &[
            SkillDef {
                kind: SkillKind::Spell,
                launch: R234,
                target: Target::Enemy(R1234),
                dmg_mod: -20,
                effects: &[Effect::Stun { chance: 60 }],
                weight: 2,
                anim: anim::SPELL_CAST_DIRECTED,
                ..m("rock_shard", "Rock Shard")
            },
            SkillDef {
                kind: SkillKind::Spell,
                launch: R234,
                target: Target::Enemy(R1234),
                dmg_mod: -100,
                effects: &[Effect::Stress(10)],
                weight: 2,
                anim: anim::SPELL_CAST_DIRECTED,
                ..m("you_no_take", "\u{201c}You No Take Candle!\u{201d}")
            },
        ],
    },
    MonsterDef {
        id: "kobold_taskmaster",
        name: "Kobold Taskmaster",
        tags: &[Tag::Kobold, Tag::Humanoid],
        base: stats(20, 5, 15, 3, 6, (4, 8), mres(40, 30, 30, 30, 40)),
        size: 1,
        display: 371,
        height: 1.8,
        boss: false,
        skills: &[
            SkillDef {
                launch: R12,
                target: Target::Enemy(R12),
                crit_mod: 4,
                weight: 3,
                anim: anim::ATTACK_1H,
                ..m("whip_crack", "Whip Crack")
            },
            SkillDef {
                kind: SkillKind::Support,
                launch: R1234,
                target: Target::AllyAll,
                effects: &[
                    Effect::Buff {
                        stat: Stat::Dmg,
                        amt: 20,
                        turns: 3,
                    },
                    Effect::Buff {
                        stat: Stat::Speed,
                        amt: 2,
                        turns: 3,
                    },
                ],
                weight: 1,
                limit: 2,
                anim: anim::BATTLE_ROAR,
                ..m("work_harder", "Work Harder!")
            },
        ],
    },
    MonsterDef {
        id: "rat",
        name: "Swarming Rat",
        tags: &[Tag::Beast],
        base: stats(7, 25, 0, 8, 6, (2, 4), mres(10, 50, 10, 10, 10)),
        size: 1,
        display: 1418,
        height: 0.8,
        boss: false,
        skills: &[SkillDef {
            launch: R1234,
            target: Target::Enemy(R12),
            effects: &[Effect::Blight {
                dmg: 1,
                turns: 3,
                chance: 80,
            }],
            weight: 1,
            anim: anim::ATTACK_UNARMED,
            ..m("gnaw", "Gnaw")
        }],
    },
    MonsterDef {
        id: "cave_creeper",
        name: "Cave Creeper",
        tags: &[Tag::Beast],
        base: stats(14, 15, 10, 6, 6, (3, 6), mres(25, 100, 20, 20, 30)),
        size: 1,
        display: 283,
        height: 0.75,
        boss: false,
        skills: &[
            SkillDef {
                launch: R1234,
                target: Target::Enemy(R1234),
                dmg_mod: -30,
                effects: &[Effect::Blight {
                    dmg: 3,
                    turns: 3,
                    chance: 100,
                }],
                weight: 2,
                anim: anim::ATTACK_UNARMED,
                ..m("venom_bite", "Venom Bite")
            },
            SkillDef {
                kind: SkillKind::Ranged,
                launch: R1234,
                target: Target::Enemy(R34),
                dmg_mod: -60,
                effects: &[Effect::Pull {
                    ranks: 2,
                    chance: 100,
                }],
                weight: 1,
                anim: anim::ATTACK_UNARMED,
                ..m("web_line", "Web Line")
            },
        ],
    },
    MonsterDef {
        id: "gut_worm",
        name: "Gut-Worm",
        tags: &[Tag::Beast, Tag::Eldritch],
        base: stats(18, 0, 10, 2, 5, (3, 7), mres(60, 100, 60, 30, 80)),
        size: 1,
        display: 8182,
        height: 1.6,
        boss: false,
        skills: &[SkillDef {
            launch: R1234,
            target: Target::Enemy(R1234),
            dmg_mod: -50,
            effects: &[
                Effect::Stress(10),
                Effect::Blight {
                    dmg: 2,
                    turns: 2,
                    chance: 80,
                },
            ],
            weight: 2,
            anim: anim::ATTACK_UNARMED,
            ..m("burst_from_below", "Burst From Below")
        }],
    },
    MonsterDef {
        id: "broodmother",
        name: "The Broodmother of Stone",
        tags: &[Tag::Beast, Tag::Eldritch],
        base: stats(85, 10, 25, 6, 8, (6, 11), mres(80, 200, 60, 40, 200)),
        size: 1,
        display: 7536,
        height: 1.7,
        boss: true,
        skills: &[
            SkillDef {
                launch: R1234,
                target: Target::EnemyAll(R1234),
                dmg_mod: -70,
                effects: &[Effect::Blight {
                    dmg: 3,
                    turns: 3,
                    chance: 100,
                }],
                weight: 2,
                anim: anim::ATTACK_UNARMED,
                ..m("venom_spray", "Venom Spray")
            },
            SkillDef {
                launch: R1234,
                target: Target::EnemyAll(R12),
                dmg_mod: -100,
                effects: &[Effect::Stun { chance: 80 }, Effect::Stress(6)],
                weight: 1,
                anim: anim::ATTACK_UNARMED,
                ..m("cocoon", "Cocoon")
            },
            SkillDef {
                launch: R1234,
                target: Target::Enemy(R1234),
                dmg_mod: 25,
                crit_mod: 6,
                effects: &[Effect::Bleed {
                    dmg: 4,
                    turns: 3,
                    chance: 100,
                }],
                weight: 2,
                anim: anim::ATTACK_UNARMED,
                ..m("devour", "Devour")
            },
        ],
    },
    MonsterDef {
        id: "murloc_raider",
        name: "Mire Raider",
        tags: &[Tag::Humanoid],
        base: stats(13, 15, 5, 5, 6, (3, 6), mres(20, 30, 20, 20, 20)),
        size: 1,
        display: 31,
        height: 1.5,
        boss: false,
        skills: &[
            SkillDef {
                launch: R12,
                target: Target::Enemy(R12),
                weight: 2,
                anim: anim::ATTACK_1H,
                ..m("barbed_spear", "Barbed Spear")
            },
            SkillDef {
                launch: R12,
                target: Target::Enemy(R12),
                dmg_mod: -40,
                effects: &[
                    Effect::Bleed {
                        dmg: 2,
                        turns: 3,
                        chance: 100,
                    },
                    Effect::Stress(4),
                ],
                weight: 2,
                anim: anim::ATTACK_1H,
                ..m("mrglgrgl", "Mrrglgrgl!")
            },
        ],
    },
    MonsterDef {
        id: "murloc_oracle",
        name: "Tidecaller",
        tags: &[Tag::Humanoid],
        base: stats(11, 10, 0, 6, 5, (2, 5), mres(20, 30, 20, 30, 20)),
        size: 1,
        display: 188,
        height: 1.45,
        boss: false,
        skills: &[
            SkillDef {
                kind: SkillKind::Heal,
                launch: R1234,
                target: Target::Ally(R1234),
                heal: Some((4, 6)),
                weight: 2,
                anim: anim::SPELL_CAST_OMNI,
                ..m("healing_tide", "Healing Tide")
            },
            SkillDef {
                kind: SkillKind::Spell,
                launch: R234,
                target: Target::Enemy(R1234),
                dmg_mod: -10,
                weight: 2,
                anim: anim::SPELL_CAST_DIRECTED,
                ..m("water_bolt", "Water Bolt")
            },
        ],
    },
    MonsterDef {
        id: "crab",
        name: "Shellback",
        tags: &[Tag::Beast],
        base: stats(18, 5, 35, 2, 5, (3, 6), mres(50, 40, 60, 30, 60)),
        size: 1,
        display: 342,
        height: 0.9,
        boss: false,
        skills: &[
            SkillDef {
                launch: R12,
                target: Target::Enemy(R12),
                weight: 2,
                anim: anim::ATTACK_UNARMED,
                ..m("pinch", "Pinch")
            },
            SkillDef {
                kind: SkillKind::Support,
                launch: R1234,
                target: Target::Ally(R1234),
                effects: &[Effect::Guard { turns: 2 }],
                self_effects: &[Effect::Buff {
                    stat: Stat::Prot,
                    amt: 10,
                    turns: 2,
                }],
                weight: 1,
                anim: anim::BATTLE_ROAR,
                ..m("shell_wall", "Shell Wall")
            },
        ],
    },
    MonsterDef {
        id: "lobstrok",
        name: "Reef Lurker",
        tags: &[Tag::Beast],
        base: stats(26, 0, 25, 3, 6, (5, 9), mres(60, 40, 50, 30, 80)),
        size: 1,
        display: 1817,
        height: 1.7,
        boss: false,
        skills: &[
            SkillDef {
                launch: R12,
                target: Target::Enemy(R12),
                dmg_mod: -30,
                effects: &[Effect::Stun { chance: 75 }],
                weight: 2,
                anim: anim::ATTACK_UNARMED,
                ..m("crushing_claw", "Crushing Claw")
            },
            SkillDef {
                launch: R12,
                target: Target::EnemyAll(R12),
                dmg_mod: -40,
                weight: 1,
                anim: anim::ATTACK_UNARMED,
                ..m("tail_sweep", "Tail Sweep")
            },
        ],
    },
    MonsterDef {
        id: "siren",
        name: "Drowned Siren",
        tags: &[Tag::Eldritch],
        base: stats(12, 20, 0, 6, 6, (2, 5), mres(30, 60, 100, 40, 30)),
        size: 1,
        display: 4972,
        height: 2.0,
        boss: false,
        skills: &[
            SkillDef {
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::Enemy(R1234),
                dmg_mod: -100,
                effects: &[Effect::Stress(15)],
                weight: 2,
                anim: anim::SPELL_CAST_DIRECTED,
                ..m("drowning_song", "Drowning Song")
            },
            SkillDef {
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::Enemy(R34),
                dmg_mod: -50,
                effects: &[
                    Effect::Pull {
                        ranks: 3,
                        chance: 100,
                    },
                    Effect::Debuff {
                        stat: Stat::Dodge,
                        amt: -10,
                        turns: 3,
                        chance: 100,
                    },
                ],
                weight: 1,
                anim: anim::SPELL_CAST_DIRECTED,
                ..m("lure", "Lure Into the Deep")
            },
        ],
    },
    MonsterDef {
        id: "myrmidon",
        name: "Tidal Myrmidon",
        tags: &[Tag::Humanoid],
        base: stats(22, 10, 15, 4, 7, (5, 9), mres(40, 40, 40, 30, 40)),
        size: 1,
        display: 4763,
        height: 2.2,
        boss: false,
        skills: &[
            SkillDef {
                launch: R12,
                target: Target::Enemy(R12),
                crit_mod: 5,
                weight: 2,
                anim: anim::ATTACK_2H,
                ..m("trident_thrust", "Trident Thrust")
            },
            SkillDef {
                launch: R12,
                target: Target::EnemyAll(R12),
                dmg_mod: -35,
                effects: &[Effect::Bleed {
                    dmg: 2,
                    turns: 2,
                    chance: 80,
                }],
                weight: 1,
                anim: anim::ATTACK_2H,
                ..m("riptide_sweep", "Riptide Sweep")
            },
        ],
    },
    MonsterDef {
        id: "matriarch",
        name: "The Abyssal Matriarch",
        tags: &[Tag::Eldritch],
        base: stats(90, 5, 20, 5, 7, (6, 11), mres(80, 100, 80, 40, 200)),
        size: 1,
        display: 791,
        height: 3.2,
        boss: true,
        skills: &[
            SkillDef {
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::EnemyAll(R1234),
                dmg_mod: -60,
                effects: &[Effect::Debuff {
                    stat: Stat::Acc,
                    amt: -10,
                    turns: 2,
                    chance: 100,
                }],
                weight: 2,
                anim: anim::SPELL_CAST_OMNI,
                ..m("crushing_tide", "Crushing Tide")
            },
            SkillDef {
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::EnemyAll(R1234),
                dmg_mod: -100,
                effects: &[Effect::Stress(12)],
                weight: 2,
                anim: anim::BATTLE_ROAR,
                ..m("call_of_the_deep", "Call of the Deep")
            },
            SkillDef {
                launch: R1234,
                target: Target::Enemy(R12),
                dmg_mod: 30,
                effects: &[Effect::Push {
                    ranks: 2,
                    chance: 100,
                }],
                weight: 1,
                anim: anim::ATTACK_2H,
                ..m("leviathan_fist", "Leviathan Fist")
            },
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::super::data::monster;
    use super::*;

    #[test]
    fn every_region_pack_names_real_monsters_that_fit() {
        for r in Region::ALL {
            for pack in r
                .packs()
                .iter()
                .chain(r.hall_packs())
                .chain([&r.boss_pack()])
            {
                let size: u8 = pack
                    .iter()
                    .map(|id| monster(id).unwrap_or_else(|| panic!("{id}")).size)
                    .sum();
                assert!(size <= 4, "{pack:?}");
            }
        }
    }
}
