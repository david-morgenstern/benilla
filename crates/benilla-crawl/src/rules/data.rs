//! The game's content tables: hero classes, their skills, and the denizens of the deep. Pure data;
//! the combat rules in [`super::combat`] interpret it.

use serde::{Deserialize, Serialize};

/// Rank masks: bit 0 is rank 1 (the front), bit 3 rank 4 (the back).
pub const R1: u8 = 0b0001;
pub const R2: u8 = 0b0010;
pub const R3: u8 = 0b0100;
pub const R4: u8 = 0b1000;
pub const R12: u8 = R1 | R2;
pub const R23: u8 = R2 | R3;
pub const R34: u8 = R3 | R4;
pub const R123: u8 = R1 | R2 | R3;
pub const R234: u8 = R2 | R3 | R4;
pub const R1234: u8 = R1 | R2 | R3 | R4;

/// Whether a 1-based rank is in a mask.
pub fn has_rank(mask: u8, rank: usize) -> bool {
    (1..=4).contains(&rank) && mask & (1 << (rank - 1)) != 0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stat {
    Acc,
    Crit,
    Dmg,
    Dodge,
    Prot,
    Speed,
    StunRes,
    BleedRes,
    BlightRes,
    DebuffRes,
    MoveRes,
    /// Stress taken, in percent: +20 means 20% more stress from every source.
    StressTaken,
    /// Healing received, in percent.
    HealRecv,
}

impl Stat {
    pub fn label(self) -> &'static str {
        match self {
            Stat::Acc => "ACC",
            Stat::Crit => "CRIT",
            Stat::Dmg => "DMG",
            Stat::Dodge => "DODGE",
            Stat::Prot => "PROT",
            Stat::Speed => "SPD",
            Stat::StunRes => "Stun Res",
            Stat::BleedRes => "Bleed Res",
            Stat::BlightRes => "Blight Res",
            Stat::DebuffRes => "Debuff Res",
            Stat::MoveRes => "Move Res",
            Stat::StressTaken => "Stress",
            Stat::HealRecv => "Heal Recv",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tag {
    Undead,
    Beast,
    Humanoid,
    Demon,
    Eldritch,
    Kobold,
}

/// What a skill does to whoever it lands on (or, in `self_effects`, to its user).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    Bleed {
        dmg: i32,
        turns: u8,
        chance: i32,
    },
    Blight {
        dmg: i32,
        turns: u8,
        chance: i32,
    },
    Stun {
        chance: i32,
    },
    Push {
        ranks: u8,
        chance: i32,
    },
    Pull {
        ranks: u8,
        chance: i32,
    },
    Mark {
        turns: u8,
    },
    Debuff {
        stat: Stat,
        amt: i32,
        turns: u8,
        chance: i32,
    },
    Buff {
        stat: Stat,
        amt: i32,
        turns: u8,
    },
    /// The user guards the target: hits on the target land on the user instead.
    Guard {
        turns: u8,
    },
    Stress(i32),
    StressHeal(i32),
    /// Removes bleeding and blight.
    Cure,
    Riposte {
        turns: u8,
    },
    /// Monsters prefer to strike this unit.
    Taunt {
        turns: u8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// One enemy in these ranks.
    Enemy(u8),
    /// Every enemy in these ranks.
    EnemyAll(u8),
    /// One ally (self included) in these ranks.
    Ally(u8),
    /// One ally other than the user.
    AllyOther(u8),
    /// The whole party.
    AllyAll,
    SelfOnly,
}

impl Target {
    pub fn is_enemy(self) -> bool {
        matches!(self, Target::Enemy(_) | Target::EnemyAll(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillKind {
    Melee,
    Ranged,
    Spell,
    Heal,
    Support,
}

#[derive(Clone, Copy, Debug)]
pub struct SkillDef {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    /// `Interface\Icons\…` basename, loaded from the player's install.
    pub icon: &'static str,
    pub kind: SkillKind,
    /// Ranks the user may stand in to use it.
    pub launch: u8,
    pub target: Target,
    /// Base accuracy, before the user's modifier and the target's dodge.
    pub acc: i32,
    /// Damage modifier in percent, applied to the user's damage range.
    pub dmg_mod: i32,
    pub crit_mod: i32,
    /// A heal range; replaces damage.
    pub heal: Option<(i32, i32)>,
    pub effects: &'static [Effect],
    pub self_effects: &'static [Effect],
    /// Extra damage percent against a tag.
    pub bonus_vs: Option<(Tag, i32)>,
    /// Extra damage percent against a marked target.
    pub bonus_marked: i32,
    /// Positions the user moves after use: positive is toward the front.
    pub self_move: i8,
    /// Uses per battle; `0` is unlimited.
    pub limit: u8,
    /// Monster AI weight; heroes ignore it.
    pub weight: u32,
    /// The creature animation to play: an `AnimationData.dbc` id.
    pub anim: u16,
}

/// The `AnimationData.dbc` ids the presentation plays.
pub mod anim {
    pub const STAND: u16 = 0;
    pub const DEATH: u16 = 1;
    pub const WALK: u16 = 4;
    pub const RUN: u16 = 5;
    pub const DEAD: u16 = 6;
    pub const WOUND: u16 = 9;
    pub const COMBAT_WOUND: u16 = 10;
    pub const ATTACK_UNARMED: u16 = 16;
    pub const ATTACK_1H: u16 = 17;
    pub const ATTACK_2H: u16 = 18;
    pub const PARRY_UNARMED: u16 = 20;
    pub const SPELL_CAST_DIRECTED: u16 = 31;
    pub const SPELL_CAST_OMNI: u16 = 32;
    pub const BATTLE_ROAR: u16 = 55;
    pub const READY_UNARMED: u16 = 25;
    pub const READY_1H: u16 = 26;
    pub const STUN: u16 = 64;
    pub const ATTACK_BOW: u16 = 44;
    pub const SPECIAL_1H: u16 = 57;
    pub const SPELL_PRECAST: u16 = 51;
}

const fn skill(id: &'static str, name: &'static str, icon: &'static str) -> SkillDef {
    SkillDef {
        id,
        name,
        desc: "",
        icon,
        kind: SkillKind::Melee,
        launch: R1234,
        target: Target::Enemy(R12),
        acc: 85,
        dmg_mod: 0,
        crit_mod: 0,
        heal: None,
        effects: &[],
        self_effects: &[],
        bonus_vs: None,
        bonus_marked: 0,
        self_move: 0,
        limit: 0,
        weight: 1,
        anim: anim::ATTACK_1H,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Resists {
    pub stun: i32,
    pub blight: i32,
    pub bleed: i32,
    pub debuff: i32,
    pub moves: i32,
    pub death_blow: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub max_hp: i32,
    pub dodge: i32,
    pub prot: i32,
    pub speed: i32,
    pub acc: i32,
    pub crit: i32,
    pub dmg: (i32, i32),
    pub res: Resists,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeroClass {
    Warrior,
    Paladin,
    Rogue,
    Priest,
    Mage,
    Hunter,
    Warlock,
}

impl HeroClass {
    pub const ALL: [HeroClass; 7] = [
        HeroClass::Warrior,
        HeroClass::Paladin,
        HeroClass::Rogue,
        HeroClass::Priest,
        HeroClass::Mage,
        HeroClass::Hunter,
        HeroClass::Warlock,
    ];

    pub fn def(self) -> &'static ClassDef {
        CLASSES
            .iter()
            .find(|c| c.class == self)
            .expect("every class has a def")
    }
}

pub struct ClassDef {
    pub class: HeroClass,
    pub name: &'static str,
    pub blurb: &'static str,
    pub base: Stats,
    /// The class's skills; a hero equips four of them.
    pub skills: &'static [SkillDef],
    /// The ranks the class wants to stand in, for the default formation.
    pub prefers: u8,
}

const fn res(stun: i32, blight: i32, bleed: i32, debuff: i32, moves: i32) -> Resists {
    Resists {
        stun,
        blight,
        bleed,
        debuff,
        moves,
        death_blow: 67,
    }
}

const fn stats(
    max_hp: i32,
    dodge: i32,
    prot: i32,
    speed: i32,
    crit: i32,
    dmg: (i32, i32),
    res: Resists,
) -> Stats {
    Stats {
        max_hp,
        dodge,
        prot,
        speed,
        acc: 0,
        crit,
        dmg,
        res,
    }
}

pub static CLASSES: &[ClassDef] = &[
    ClassDef {
        class: HeroClass::Warrior,
        name: "Warrior",
        blurb: "A veteran of a hundred losing battles, still standing at the front of the line.",
        base: stats(37, 5, 10, 3, 4, (6, 12), res(50, 30, 40, 30, 50)),
        prefers: R12,
        skills: &[
            SkillDef {
                desc: "A brutal two-handed blow that leaves wounds slow to close.",
                launch: R12,
                target: Target::Enemy(R12),
                dmg_mod: 0,
                crit_mod: 5,
                effects: &[Effect::Debuff {
                    stat: Stat::HealRecv,
                    amt: -33,
                    turns: 3,
                    chance: 100,
                }],
                anim: anim::ATTACK_2H,
                ..skill(
                    "mortal_strike",
                    "Mortal Strike",
                    "Ability_Warrior_SavageBlow",
                )
            },
            SkillDef {
                desc: "Slam a shield into the foe's face; the stunned do not strike back.",
                launch: R12,
                target: Target::Enemy(R12),
                dmg_mod: -50,
                effects: &[Effect::Stun { chance: 100 }],
                anim: anim::SPECIAL_1H,
                ..skill("shield_bash", "Shield Bash", "Ability_Warrior_ShieldBash")
            },
            SkillDef {
                desc: "A sweeping strike across the enemy's front line.",
                launch: R12,
                target: Target::EnemyAll(R12),
                dmg_mod: -40,
                anim: anim::ATTACK_2H,
                ..skill("cleave", "Cleave", "Ability_Warrior_Cleave")
            },
            SkillDef {
                desc: "Stand before an ally and take their blows.",
                kind: SkillKind::Support,
                launch: R1234,
                target: Target::AllyOther(R1234),
                effects: &[Effect::Guard { turns: 3 }],
                self_effects: &[Effect::Buff {
                    stat: Stat::Prot,
                    amt: 15,
                    turns: 3,
                }],
                anim: anim::BATTLE_ROAR,
                ..skill("intervene", "Intervene", "Ability_Defend")
            },
            SkillDef {
                desc: "A roar that steels the party's nerve.",
                kind: SkillKind::Support,
                launch: R1234,
                target: Target::AllyAll,
                effects: &[
                    Effect::StressHeal(4),
                    Effect::Buff {
                        stat: Stat::Dmg,
                        amt: 15,
                        turns: 3,
                    },
                ],
                limit: 2,
                anim: anim::BATTLE_ROAR,
                ..skill(
                    "battle_shout",
                    "Battle Shout",
                    "Ability_Warrior_BattleShout",
                )
            },
            SkillDef {
                desc: "Charge into the enemy, scattering their ranks.",
                launch: R34,
                target: Target::Enemy(R123),
                dmg_mod: -25,
                effects: &[Effect::Push {
                    ranks: 1,
                    chance: 100,
                }],
                self_move: 2,
                anim: anim::ATTACK_2H,
                ..skill("charge", "Charge", "Ability_Warrior_Charge")
            },
        ],
    },
    ClassDef {
        class: HeroClass::Paladin,
        name: "Paladin",
        blurb: "Sworn to a Light that has gone quiet. Swings the hammer anyway.",
        base: stats(33, 5, 15, 2, 3, (5, 10), res(45, 35, 35, 35, 45)),
        prefers: R12,
        skills: &[
            SkillDef {
                desc: "A blow charged with holy wrath, ruinous to the undead.",
                launch: R12,
                target: Target::Enemy(R12),
                bonus_vs: Some((Tag::Undead, 50)),
                anim: anim::ATTACK_2H,
                ..skill("holy_strike", "Holy Strike", "Spell_Holy_HolySmite")
            },
            SkillDef {
                desc: "Hurl judgement at a distant foe.",
                kind: SkillKind::Spell,
                launch: R12,
                target: Target::Enemy(R34),
                dmg_mod: -35,
                effects: &[Effect::Debuff {
                    stat: Stat::Acc,
                    amt: -10,
                    turns: 3,
                    chance: 100,
                }],
                anim: anim::SPELL_CAST_DIRECTED,
                ..skill("judgement", "Judgement", "Spell_Holy_RighteousFury")
            },
            SkillDef {
                desc: "Strike the foe with a hammer of light.",
                launch: R12,
                target: Target::Enemy(R12),
                dmg_mod: -60,
                effects: &[Effect::Stun { chance: 110 }],
                anim: anim::SPELL_CAST_DIRECTED,
                ..skill(
                    "hammer_justice",
                    "Hammer of Justice",
                    "Spell_Holy_SealOfMight",
                )
            },
            SkillDef {
                desc: "A brief, warm prayer over a wounded companion.",
                kind: SkillKind::Heal,
                launch: R1234,
                target: Target::Ally(R1234),
                heal: Some((3, 5)),
                anim: anim::SPELL_CAST_OMNI,
                ..skill("holy_light", "Holy Light", "Spell_Holy_HolyBolt")
            },
            SkillDef {
                desc: "Bless the whole party against the dark.",
                kind: SkillKind::Support,
                launch: R1234,
                target: Target::AllyAll,
                effects: &[
                    Effect::Buff {
                        stat: Stat::Prot,
                        amt: 10,
                        turns: 3,
                    },
                    Effect::StressHeal(3),
                ],
                limit: 2,
                anim: anim::SPELL_CAST_OMNI,
                ..skill("devotion", "Devotion Aura", "Spell_Holy_DevotionAura")
            },
            SkillDef {
                desc: "Hallow the ground at the paladin's feet; foes in front burn.",
                kind: SkillKind::Spell,
                launch: R1,
                target: Target::EnemyAll(R12),
                dmg_mod: -50,
                bonus_vs: Some((Tag::Undead, 40)),
                anim: anim::SPELL_CAST_OMNI,
                ..skill("consecration", "Consecration", "Spell_Holy_InnerFire")
            },
        ],
    },
    ClassDef {
        class: HeroClass::Rogue,
        name: "Rogue",
        blurb: "A cutpurse who came down for the gold and stayed for the debt.",
        base: stats(23, 15, 0, 6, 7, (5, 10), res(30, 40, 30, 30, 20)),
        prefers: R23,
        skills: &[
            SkillDef {
                desc: "A quick, vicious cut.",
                launch: R123,
                target: Target::Enemy(R12),
                crit_mod: 5,
                anim: anim::ATTACK_1H,
                ..skill(
                    "sinister_strike",
                    "Sinister Strike",
                    "Spell_Shadow_RitualOfSacrifice",
                )
            },
            SkillDef {
                desc: "Open an artery and let the dark do the rest.",
                launch: R123,
                target: Target::Enemy(R123),
                dmg_mod: -40,
                effects: &[Effect::Bleed {
                    dmg: 3,
                    turns: 3,
                    chance: 100,
                }],
                anim: anim::ATTACK_1H,
                ..skill("rupture", "Rupture", "Ability_Rogue_Rupture")
            },
            SkillDef {
                desc: "Step from the shadows into the enemy's back line.",
                launch: R23,
                target: Target::Enemy(R234),
                dmg_mod: 15,
                crit_mod: 10,
                self_move: 1,
                anim: anim::SPECIAL_1H,
                ..skill("ambush", "Ambush", "Ability_Rogue_Ambush")
            },
            SkillDef {
                desc: "A thumb to the eye.",
                launch: R12,
                target: Target::Enemy(R12),
                dmg_mod: -70,
                effects: &[Effect::Stun { chance: 100 }],
                self_move: -1,
                anim: anim::ATTACK_1H,
                ..skill("gouge", "Gouge", "Ability_Gouge")
            },
            SkillDef {
                desc: "Throw a knife at whoever cowers at the back.",
                kind: SkillKind::Ranged,
                launch: R234,
                target: Target::Enemy(R1234),
                dmg_mod: -15,
                bonus_marked: 50,
                anim: anim::ATTACK_1H,
                ..skill("throw", "Throw", "INV_ThrowingKnife_02")
            },
            SkillDef {
                desc: "Coat the blades. The next cuts fester.",
                kind: SkillKind::Support,
                launch: R1234,
                target: Target::SelfOnly,
                self_effects: &[
                    Effect::Buff {
                        stat: Stat::Dmg,
                        amt: 25,
                        turns: 3,
                    },
                    Effect::Buff {
                        stat: Stat::Crit,
                        amt: 8,
                        turns: 3,
                    },
                ],
                anim: anim::SPELL_PRECAST,
                ..skill("poisons", "Envenom", "Ability_Poisons")
            },
        ],
    },
    ClassDef {
        class: HeroClass::Priest,
        name: "Priest",
        blurb: "Prays to the Light out of habit. Some days the Light answers.",
        base: stats(24, 5, 0, 4, 1, (3, 7), res(30, 30, 30, 40, 30)),
        prefers: R34,
        skills: &[
            SkillDef {
                desc: "Close a companion's wounds.",
                kind: SkillKind::Heal,
                launch: R34,
                target: Target::Ally(R1234),
                heal: Some((4, 7)),
                anim: anim::SPELL_CAST_OMNI,
                ..skill("heal", "Heal", "Spell_Holy_Heal")
            },
            SkillDef {
                desc: "A wave of light washes over the party.",
                kind: SkillKind::Heal,
                launch: R34,
                target: Target::AllyAll,
                heal: Some((1, 3)),
                anim: anim::SPELL_CAST_OMNI,
                ..skill(
                    "prayer_healing",
                    "Prayer of Healing",
                    "Spell_Holy_PrayerOfHealing02",
                )
            },
            SkillDef {
                desc: "Smite the wicked with holy fire.",
                kind: SkillKind::Spell,
                launch: R234,
                target: Target::Enemy(R1234),
                dmg_mod: -20,
                bonus_vs: Some((Tag::Undead, 40)),
                anim: anim::SPELL_CAST_DIRECTED,
                ..skill("smite", "Smite", "Spell_Holy_HolySmite")
            },
            SkillDef {
                desc: "Calm a mind gone ragged.",
                kind: SkillKind::Support,
                launch: R1234,
                target: Target::AllyOther(R1234),
                effects: &[Effect::StressHeal(12)],
                anim: anim::SPELL_CAST_OMNI,
                ..skill("mind_soothe", "Mind Soothe", "Spell_Holy_MindSooth")
            },
            SkillDef {
                desc: "A shell of light around an ally.",
                kind: SkillKind::Support,
                launch: R1234,
                target: Target::Ally(R1234),
                effects: &[
                    Effect::Buff {
                        stat: Stat::Prot,
                        amt: 25,
                        turns: 2,
                    },
                    Effect::Cure,
                ],
                anim: anim::SPELL_CAST_OMNI,
                ..skill(
                    "pw_shield",
                    "Power Word: Shield",
                    "Spell_Holy_PowerWordShield",
                )
            },
            SkillDef {
                desc: "A scream that sends the front ranks reeling.",
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::EnemyAll(R12),
                dmg_mod: -100,
                effects: &[
                    Effect::Stun { chance: 60 },
                    Effect::Push {
                        ranks: 1,
                        chance: 75,
                    },
                ],
                limit: 1,
                anim: anim::SPELL_CAST_OMNI,
                ..skill(
                    "psychic_scream",
                    "Psychic Scream",
                    "Spell_Shadow_PsychicScream",
                )
            },
        ],
    },
    ClassDef {
        class: HeroClass::Mage,
        name: "Mage",
        blurb: "Studied the arcane in Dalaran; learned the rest down here.",
        base: stats(20, 5, 0, 5, 6, (5, 10), res(20, 30, 30, 50, 30)),
        prefers: R34,
        skills: &[
            SkillDef {
                desc: "A bolt of fire that keeps burning.",
                kind: SkillKind::Spell,
                launch: R234,
                target: Target::Enemy(R1234),
                dmg_mod: 0,
                effects: &[Effect::Blight {
                    dmg: 2,
                    turns: 3,
                    chance: 100,
                }],
                anim: anim::SPELL_CAST_DIRECTED,
                ..skill("fireball", "Fireball", "Spell_Fire_FlameBolt")
            },
            SkillDef {
                desc: "A shard of ice; the struck move slowly.",
                kind: SkillKind::Spell,
                launch: R234,
                target: Target::Enemy(R1234),
                dmg_mod: -15,
                effects: &[Effect::Debuff {
                    stat: Stat::Speed,
                    amt: -4,
                    turns: 3,
                    chance: 100,
                }],
                anim: anim::SPELL_CAST_DIRECTED,
                ..skill("frostbolt", "Frostbolt", "Spell_Frost_FrostBolt02")
            },
            SkillDef {
                desc: "Rain arcane missiles on the enemy's back line.",
                kind: SkillKind::Spell,
                launch: R34,
                target: Target::EnemyAll(R34),
                dmg_mod: -45,
                anim: anim::SPELL_CAST_OMNI,
                ..skill(
                    "arcane_missiles",
                    "Arcane Missiles",
                    "Spell_Nature_StarFall",
                )
            },
            SkillDef {
                desc: "Freeze the front ranks where they stand.",
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::EnemyAll(R12),
                dmg_mod: -80,
                effects: &[Effect::Stun { chance: 75 }],
                limit: 2,
                anim: anim::SPELL_CAST_OMNI,
                ..skill("frost_nova", "Frost Nova", "Spell_Frost_FrostNova")
            },
            SkillDef {
                desc: "Turn a foe into a sheep. It is as humiliating as it sounds.",
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::Enemy(R1234),
                dmg_mod: -100,
                effects: &[
                    Effect::Stun { chance: 120 },
                    Effect::Debuff {
                        stat: Stat::Prot,
                        amt: -20,
                        turns: 2,
                        chance: 100,
                    },
                ],
                limit: 1,
                anim: anim::SPELL_CAST_DIRECTED,
                ..skill("polymorph", "Polymorph", "Spell_Nature_Polymorph")
            },
            SkillDef {
                desc: "Blink back to safety and steady the hands.",
                kind: SkillKind::Support,
                launch: R12,
                target: Target::SelfOnly,
                self_effects: &[
                    Effect::Buff {
                        stat: Stat::Dodge,
                        amt: 15,
                        turns: 2,
                    },
                    Effect::Buff {
                        stat: Stat::Acc,
                        amt: 10,
                        turns: 2,
                    },
                ],
                self_move: -2,
                anim: anim::SPELL_CAST_OMNI,
                ..skill("blink", "Blink", "Spell_Arcane_Blink")
            },
        ],
    },
    ClassDef {
        class: HeroClass::Hunter,
        name: "Hunter",
        blurb: "Tracked beasts across Kalimdor. The things down here leave no tracks.",
        base: stats(26, 10, 0, 4, 6, (6, 11), res(40, 50, 40, 30, 40)),
        prefers: R34,
        skills: &[
            SkillDef {
                desc: "A slow, deliberate shot at the enemy's rear.",
                kind: SkillKind::Ranged,
                launch: R34,
                target: Target::Enemy(R234),
                crit_mod: 8,
                bonus_marked: 60,
                anim: anim::ATTACK_BOW,
                ..skill("aimed_shot", "Aimed Shot", "INV_Spear_07")
            },
            SkillDef {
                desc: "Mark a foe; everyone will know where to strike.",
                kind: SkillKind::Ranged,
                launch: R234,
                target: Target::Enemy(R1234),
                acc: 100,
                dmg_mod: -100,
                effects: &[
                    Effect::Mark { turns: 3 },
                    Effect::Debuff {
                        stat: Stat::Prot,
                        amt: -15,
                        turns: 3,
                        chance: 100,
                    },
                ],
                anim: anim::ATTACK_BOW,
                ..skill("hunters_mark", "Hunter's Mark", "Ability_Hunter_SniperShot")
            },
            SkillDef {
                desc: "A venomous arrow.",
                kind: SkillKind::Ranged,
                launch: R234,
                target: Target::Enemy(R1234),
                dmg_mod: -50,
                effects: &[Effect::Blight {
                    dmg: 4,
                    turns: 3,
                    chance: 100,
                }],
                anim: anim::ATTACK_BOW,
                ..skill("serpent_sting", "Serpent Sting", "Ability_Hunter_Quickshot")
            },
            SkillDef {
                desc: "Arrows across the whole enemy line.",
                kind: SkillKind::Ranged,
                launch: R34,
                target: Target::EnemyAll(R1234),
                dmg_mod: -60,
                anim: anim::ATTACK_BOW,
                ..skill("multi_shot", "Multi-Shot", "Ability_UpgradeMoonGlaive")
            },
            SkillDef {
                desc: "A dazing shot that drags a foe forward.",
                kind: SkillKind::Ranged,
                launch: R234,
                target: Target::Enemy(R234),
                dmg_mod: -50,
                effects: &[
                    Effect::Pull {
                        ranks: 2,
                        chance: 100,
                    },
                    Effect::Stun { chance: 50 },
                ],
                anim: anim::ATTACK_BOW,
                ..skill("concussive_shot", "Concussive Shot", "Spell_Frost_Stun")
            },
            SkillDef {
                desc: "Field dressing for a bleeding friend.",
                kind: SkillKind::Heal,
                launch: R1234,
                target: Target::Ally(R1234),
                heal: Some((2, 3)),
                effects: &[Effect::Cure, Effect::StressHeal(3)],
                anim: anim::SPELL_PRECAST,
                ..skill("bandage", "First Aid", "INV_Misc_Bandage_15")
            },
        ],
    },
    ClassDef {
        class: HeroClass::Warlock,
        name: "Warlock",
        blurb: "Bargained with something below. The bargain is still being paid.",
        base: stats(22, 5, 0, 4, 5, (4, 9), res(30, 50, 30, 50, 30)),
        prefers: R234,
        skills: &[
            SkillDef {
                desc: "Rot the foe from within.",
                kind: SkillKind::Spell,
                launch: R234,
                target: Target::Enemy(R1234),
                acc: 95,
                dmg_mod: -80,
                effects: &[Effect::Blight {
                    dmg: 5,
                    turns: 3,
                    chance: 110,
                }],
                anim: anim::SPELL_CAST_DIRECTED,
                ..skill(
                    "corruption",
                    "Corruption",
                    "Spell_Shadow_AbominationExplosion",
                )
            },
            SkillDef {
                desc: "A bolt of pure shadow.",
                kind: SkillKind::Spell,
                launch: R234,
                target: Target::Enemy(R1234),
                dmg_mod: 10,
                crit_mod: 3,
                anim: anim::SPELL_CAST_DIRECTED,
                ..skill("shadow_bolt", "Shadow Bolt", "Spell_Shadow_ShadowBolt")
            },
            SkillDef {
                desc: "Siphon a foe's life to mend one's own.",
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::Enemy(R1234),
                dmg_mod: -30,
                self_effects: &[Effect::Buff {
                    stat: Stat::HealRecv,
                    amt: 0,
                    turns: 1,
                }],
                anim: anim::SPELL_CAST_DIRECTED,
                ..skill("drain_life", "Drain Life", "Spell_Shadow_LifeDrain02")
            },
            SkillDef {
                desc: "Terror takes the foe; it can do nothing but tremble.",
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::Enemy(R1234),
                dmg_mod: -100,
                effects: &[
                    Effect::Stun { chance: 100 },
                    Effect::Push {
                        ranks: 2,
                        chance: 100,
                    },
                ],
                limit: 1,
                anim: anim::SPELL_CAST_DIRECTED,
                ..skill("fear", "Fear", "Spell_Shadow_Possession")
            },
            SkillDef {
                desc: "A curse that withers strength and resolve.",
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::Enemy(R1234),
                acc: 100,
                dmg_mod: -100,
                effects: &[
                    Effect::Debuff {
                        stat: Stat::Dmg,
                        amt: -25,
                        turns: 3,
                        chance: 110,
                    },
                    Effect::Debuff {
                        stat: Stat::Acc,
                        amt: -10,
                        turns: 3,
                        chance: 110,
                    },
                ],
                anim: anim::SPELL_CAST_DIRECTED,
                ..skill(
                    "curse_weakness",
                    "Curse of Weakness",
                    "Spell_Shadow_CurseOfMannoroth",
                )
            },
            SkillDef {
                desc: "Trade blood for clarity. The voices quiet, for a while.",
                kind: SkillKind::Support,
                launch: R1234,
                target: Target::SelfOnly,
                self_effects: &[
                    Effect::StressHeal(15),
                    Effect::Buff {
                        stat: Stat::Dmg,
                        amt: 20,
                        turns: 2,
                    },
                ],
                anim: anim::SPELL_CAST_OMNI,
                ..skill("life_tap", "Life Tap", "Spell_Shadow_BurningSpirit")
            },
        ],
    },
];

/// The skill id whose damage also heals its user for half the damage dealt.
pub const DRAIN_SKILL: &str = "drain_life";
/// The skill id that costs its user hit points.
pub const LIFE_TAP_SKILL: &str = "life_tap";

#[derive(Debug)]
pub struct MonsterDef {
    pub id: &'static str,
    pub name: &'static str,
    pub tags: &'static [Tag],
    pub base: Stats,
    pub skills: &'static [SkillDef],
    /// Positions the monster occupies (a big brute takes two).
    pub size: u8,
    /// The `CreatureDisplayInfo.dbc` row whose model and skins the monster wears.
    pub display: u32,
    /// Standing height on stage in yards; the model is scaled to it from its bounds.
    pub height: f32,
    pub boss: bool,
}

impl MonsterDef {
    pub fn has_tag(&self, tag: Tag) -> bool {
        self.tags.contains(&tag)
    }
}

pub fn monster(id: &str) -> Option<&'static MonsterDef> {
    MONSTERS.iter().find(|m| m.id == id)
}

const fn mres(stun: i32, blight: i32, bleed: i32, debuff: i32, moves: i32) -> Resists {
    Resists {
        stun,
        blight,
        bleed,
        debuff,
        moves,
        death_blow: 0,
    }
}

const fn mskill(id: &'static str, name: &'static str) -> SkillDef {
    skill(id, name, "")
}

pub static MONSTERS: &[MonsterDef] = &[
    MonsterDef {
        id: "skeleton_soldier",
        name: "Bone Sentinel",
        tags: &[Tag::Undead],
        base: stats(14, 10, 15, 1, 6, (3, 7), mres(25, 10, 200, 15, 40)),
        size: 1,
        display: 158,
        height: 2.0,
        boss: false,
        skills: &[
            SkillDef {
                launch: R12,
                target: Target::Enemy(R12),
                weight: 3,
                anim: anim::ATTACK_1H,
                ..mskill("rusted_blade", "Rusted Blade")
            },
            SkillDef {
                launch: R12,
                target: Target::Enemy(R12),
                dmg_mod: -50,
                effects: &[Effect::Stun { chance: 70 }],
                weight: 1,
                anim: anim::SPECIAL_1H,
                ..mskill("shield_slam", "Shield Slam")
            },
        ],
    },
    MonsterDef {
        id: "skeleton_archer",
        name: "Hollow Bowman",
        tags: &[Tag::Undead],
        base: stats(11, 15, 10, 4, 6, (3, 6), mres(10, 10, 200, 15, 10)),
        size: 1,
        display: 535,
        height: 1.9,
        boss: false,
        skills: &[
            SkillDef {
                kind: SkillKind::Ranged,
                launch: R34,
                target: Target::Enemy(R1234),
                weight: 3,
                anim: anim::ATTACK_BOW,
                ..mskill("bone_arrow", "Bone Arrow")
            },
            SkillDef {
                launch: R12,
                target: Target::Enemy(R12),
                dmg_mod: -30,
                weight: 1,
                anim: anim::ATTACK_UNARMED,
                ..mskill("desperate_jab", "Desperate Jab")
            },
            SkillDef {
                kind: SkillKind::Ranged,
                launch: R34,
                target: Target::Enemy(R1234),
                dmg_mod: -100,
                effects: &[Effect::Mark { turns: 3 }],
                weight: 1,
                anim: anim::ATTACK_BOW,
                ..mskill("mark_for_death", "Mark for Death")
            },
        ],
    },
    MonsterDef {
        id: "ghoul",
        name: "Grave Ghoul",
        tags: &[Tag::Undead, Tag::Beast],
        base: stats(16, 10, 0, 5, 8, (3, 6), mres(20, 60, 25, 20, 20)),
        size: 1,
        display: 137,
        height: 1.8,
        boss: false,
        skills: &[
            SkillDef {
                launch: R12,
                target: Target::Enemy(R12),
                effects: &[Effect::Bleed {
                    dmg: 2,
                    turns: 3,
                    chance: 100,
                }],
                weight: 2,
                anim: anim::ATTACK_UNARMED,
                ..mskill("rending_claws", "Rending Claws")
            },
            SkillDef {
                launch: R12,
                target: Target::Enemy(R12),
                dmg_mod: -40,
                effects: &[Effect::Blight {
                    dmg: 3,
                    turns: 3,
                    chance: 100,
                }],
                weight: 2,
                anim: anim::ATTACK_UNARMED,
                ..mskill("festering_bite", "Festering Bite")
            },
        ],
    },
    MonsterDef {
        id: "banshee",
        name: "Keening Banshee",
        tags: &[Tag::Undead, Tag::Eldritch],
        base: stats(12, 20, 0, 6, 6, (2, 5), mres(40, 200, 200, 40, 40)),
        size: 1,
        display: 915,
        height: 2.1,
        boss: false,
        skills: &[
            SkillDef {
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::Enemy(R1234),
                dmg_mod: -60,
                effects: &[Effect::Stress(14)],
                weight: 3,
                anim: anim::SPELL_CAST_DIRECTED,
                ..mskill("dirge", "Dirge of the Drowned")
            },
            SkillDef {
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::EnemyAll(R1234),
                dmg_mod: -100,
                effects: &[Effect::Stress(7)],
                weight: 1,
                anim: anim::SPELL_CAST_OMNI,
                ..mskill("wail", "Wail")
            },
        ],
    },
    MonsterDef {
        id: "necrolyte",
        name: "Cult Necrolyte",
        tags: &[Tag::Humanoid],
        base: stats(13, 10, 0, 5, 6, (3, 6), mres(30, 30, 30, 30, 30)),
        size: 1,
        display: 16279,
        height: 2.0,
        boss: false,
        skills: &[
            SkillDef {
                kind: SkillKind::Spell,
                launch: R234,
                target: Target::Enemy(R1234),
                effects: &[Effect::Blight {
                    dmg: 3,
                    turns: 3,
                    chance: 100,
                }],
                weight: 2,
                anim: anim::SPELL_CAST_DIRECTED,
                ..mskill("shadow_word", "Shadow Word: Rot")
            },
            SkillDef {
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::Enemy(R1234),
                dmg_mod: -100,
                effects: &[
                    Effect::Stress(10),
                    Effect::Debuff {
                        stat: Stat::Acc,
                        amt: -10,
                        turns: 3,
                        chance: 100,
                    },
                ],
                weight: 2,
                anim: anim::SPELL_CAST_DIRECTED,
                ..mskill("whispers", "Whispers from the Grave")
            },
        ],
    },
    MonsterDef {
        id: "gargoyle",
        name: "Crypt Gargoyle",
        tags: &[Tag::Eldritch],
        base: stats(15, 20, 25, 7, 5, (3, 7), mres(60, 200, 200, 30, 20)),
        size: 1,
        display: 7533,
        height: 2.1,
        boss: false,
        skills: &[
            SkillDef {
                launch: R1234,
                target: Target::Enemy(R234),
                effects: &[Effect::Pull {
                    ranks: 1,
                    chance: 75,
                }],
                weight: 2,
                anim: anim::ATTACK_UNARMED,
                ..mskill("swooping_talons", "Swooping Talons")
            },
            SkillDef {
                launch: R12,
                target: Target::Enemy(R12),
                crit_mod: 6,
                weight: 2,
                anim: anim::ATTACK_UNARMED,
                ..mskill("stone_fist", "Stone Fist")
            },
        ],
    },
    MonsterDef {
        id: "abomination",
        name: "Stitched Horror",
        tags: &[Tag::Undead],
        base: stats(38, 0, 25, 2, 6, (6, 11), mres(75, 75, 50, 30, 100)),
        size: 2,
        display: 1061,
        height: 3.0,
        boss: false,
        skills: &[
            SkillDef {
                launch: R12,
                target: Target::EnemyAll(R12),
                dmg_mod: -30,
                weight: 2,
                anim: anim::ATTACK_2H,
                ..mskill("hook_sweep", "Hook Sweep")
            },
            SkillDef {
                launch: R12,
                target: Target::Enemy(R1234),
                dmg_mod: -40,
                effects: &[
                    Effect::Pull {
                        ranks: 3,
                        chance: 100,
                    },
                    Effect::Stress(5),
                ],
                weight: 1,
                anim: anim::ATTACK_1H,
                ..mskill("meat_hook", "Meat Hook")
            },
            SkillDef {
                launch: R12,
                target: Target::Enemy(R12),
                effects: &[Effect::Blight {
                    dmg: 3,
                    turns: 3,
                    chance: 100,
                }],
                weight: 1,
                anim: anim::ATTACK_UNARMED,
                ..mskill("bile", "Spew Bile")
            },
        ],
    },
    MonsterDef {
        id: "lich",
        name: "The Pale Archivist",
        tags: &[Tag::Undead, Tag::Eldritch],
        base: stats(75, 10, 20, 5, 8, (5, 10), mres(80, 100, 200, 40, 200)),
        size: 1,
        display: 16197,
        height: 2.7,
        boss: true,
        skills: &[
            SkillDef {
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::EnemyAll(R1234),
                dmg_mod: -50,
                effects: &[Effect::Debuff {
                    stat: Stat::Speed,
                    amt: -3,
                    turns: 2,
                    chance: 100,
                }],
                weight: 2,
                anim: anim::SPELL_CAST_OMNI,
                ..mskill("frost_tomb", "Frost Tomb")
            },
            SkillDef {
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::Enemy(R1234),
                dmg_mod: 15,
                crit_mod: 4,
                weight: 2,
                anim: anim::SPELL_CAST_DIRECTED,
                ..mskill("death_bolt", "Death Bolt")
            },
            SkillDef {
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::EnemyAll(R1234),
                dmg_mod: -100,
                effects: &[Effect::Stress(12)],
                weight: 2,
                anim: anim::BATTLE_ROAR,
                ..mskill("forbidden_page", "Read the Forbidden Page")
            },
            SkillDef {
                kind: SkillKind::Spell,
                launch: R1234,
                target: Target::Enemy(R1234),
                dmg_mod: -100,
                effects: &[Effect::Stun { chance: 90 }, Effect::Stress(6)],
                weight: 1,
                anim: anim::SPELL_CAST_DIRECTED,
                ..mskill("dread_gaze", "Dread Gaze")
            },
        ],
    },
];

/// The ordinary packs a crypt room can hold, front to back; [`super::dungeon`] scales them.
pub static CRYPT_PACKS: &[&[&str]] = &[
    &[
        "skeleton_soldier",
        "skeleton_soldier",
        "skeleton_archer",
        "skeleton_archer",
    ],
    &["ghoul", "skeleton_soldier", "banshee", "skeleton_archer"],
    &["skeleton_soldier", "ghoul", "necrolyte", "banshee"],
    &["gargoyle", "ghoul", "necrolyte"],
    &["abomination", "skeleton_archer", "necrolyte"],
    &["ghoul", "ghoul", "banshee"],
    &[
        "skeleton_soldier",
        "gargoyle",
        "skeleton_archer",
        "necrolyte",
    ],
    &["abomination", "banshee", "banshee"],
];

pub static CRYPT_BOSS_PACK: &[&str] = &["skeleton_soldier", "lich", "necrolyte"];
pub static CRYPT_HALL_PACKS: &[&[&str]] = &[
    &["skeleton_soldier", "skeleton_archer"],
    &["ghoul", "banshee"],
    &["gargoyle", "skeleton_archer"],
    &["ghoul", "necrolyte"],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_has_six_skills_with_unique_ids() {
        for c in CLASSES {
            assert_eq!(c.skills.len(), 6, "{}", c.name);
            let mut ids: Vec<_> = c.skills.iter().map(|s| s.id).collect();
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), 6, "{}", c.name);
        }
    }

    #[test]
    fn packs_name_real_monsters() {
        for pack in CRYPT_PACKS
            .iter()
            .chain(CRYPT_HALL_PACKS)
            .chain([&CRYPT_BOSS_PACK])
        {
            let size: u8 = pack.iter().map(|id| monster(id).expect(id).size).sum();
            assert!(size <= 4, "{pack:?}");
        }
    }

    #[test]
    fn rank_masks() {
        assert!(has_rank(R12, 1));
        assert!(has_rank(R12, 2));
        assert!(!has_rank(R12, 3));
        assert!(!has_rank(R1234, 5));
    }
}
