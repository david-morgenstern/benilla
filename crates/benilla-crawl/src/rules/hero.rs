//! A hero on the roster: everything that persists between expeditions, and the stats derived from
//! it. Stress, its afflictions and virtues, and quirks live here too.

use serde::{Deserialize, Serialize};

use super::data::{HeroClass, Resists, SkillDef, Stat, Stats};
use super::rng::Rng;

pub const STRESS_MAX: i32 = 200;
pub const STRESS_RESOLVE: i32 = 100;
pub const MAX_LEVEL: u8 = 6;
/// Experience to reach each resolve level (index = target level).
pub const LEVEL_XP: [i32; 7] = [0, 2, 4, 8, 12, 18, 26];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Affliction {
    Fearful,
    Paranoid,
    Selfish,
    Masochistic,
    Abusive,
    Hopeless,
    Irrational,
}

impl Affliction {
    pub const ALL: [Affliction; 7] = [
        Affliction::Fearful,
        Affliction::Paranoid,
        Affliction::Selfish,
        Affliction::Masochistic,
        Affliction::Abusive,
        Affliction::Hopeless,
        Affliction::Irrational,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Affliction::Fearful => "Fearful",
            Affliction::Paranoid => "Paranoid",
            Affliction::Selfish => "Selfish",
            Affliction::Masochistic => "Masochistic",
            Affliction::Abusive => "Abusive",
            Affliction::Hopeless => "Hopeless",
            Affliction::Irrational => "Irrational",
        }
    }

    pub fn mods(self) -> &'static [(Stat, i32)] {
        match self {
            Affliction::Fearful => &[(Stat::Dodge, 10), (Stat::Acc, -10), (Stat::Dmg, -15)],
            Affliction::Paranoid => &[(Stat::Dodge, 10), (Stat::HealRecv, -40)],
            Affliction::Selfish => &[(Stat::Dodge, 5), (Stat::Speed, 2)],
            Affliction::Masochistic => &[(Stat::Prot, -10), (Stat::Dodge, -10)],
            Affliction::Abusive => &[(Stat::Dmg, 15), (Stat::Speed, 1)],
            Affliction::Hopeless => &[(Stat::Dmg, -25), (Stat::Speed, -2), (Stat::Dodge, -5)],
            Affliction::Irrational => &[(Stat::Acc, -5), (Stat::Crit, 5), (Stat::Speed, 2)],
        }
    }

    /// What the afflicted hero cries when the dark takes hold.
    pub fn barks(self) -> &'static [&'static str] {
        match self {
            Affliction::Fearful => &[
                "We're going to die down here. All of us.",
                "Did you hear that? Something is breathing in the walls.",
                "I want to go home. I want to go home.",
            ],
            Affliction::Paranoid => &[
                "Keep your hands where I can see them.",
                "You'd sell me to the dark for a copper. I know it.",
                "Don't touch me! Don't you dare heal me!",
            ],
            Affliction::Selfish => &[
                "The gold is mine. I bled for it.",
                "Every one of you for yourselves. Starting now.",
                "Why should I take the blow? Let the warrior do it.",
            ],
            Affliction::Masochistic => &[
                "More. Hit me again. I can take it.",
                "Pain is the only honest thing left down here.",
                "Let them come. Let it hurt.",
            ],
            Affliction::Abusive => &[
                "Useless. The lot of you are useless.",
                "Your mother should have drowned you in the Elwynn.",
                "Swing faster or get out of my way!",
            ],
            Affliction::Hopeless => &[
                "What's the point? The Light left us long ago.",
                "We're already dead. The bodies just haven't noticed.",
                "Leave me. I'm not getting up.",
            ],
            Affliction::Irrational => &[
                "Ha! The walls are laughing too!",
                "Left is right and down is up and the dead are dancing!",
                "I'll fight the darkness itself! Come here!",
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Virtue {
    Stalwart,
    Courageous,
    Focused,
    Powerful,
    Vigorous,
}

impl Virtue {
    pub const ALL: [Virtue; 5] = [
        Virtue::Stalwart,
        Virtue::Courageous,
        Virtue::Focused,
        Virtue::Powerful,
        Virtue::Vigorous,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Virtue::Stalwart => "Stalwart",
            Virtue::Courageous => "Courageous",
            Virtue::Focused => "Focused",
            Virtue::Powerful => "Powerful",
            Virtue::Vigorous => "Vigorous",
        }
    }

    pub fn mods(self) -> &'static [(Stat, i32)] {
        match self {
            Virtue::Stalwart => &[
                (Stat::Prot, 10),
                (Stat::StressTaken, -25),
                (Stat::DebuffRes, 20),
            ],
            Virtue::Courageous => &[(Stat::StressTaken, -33)],
            Virtue::Focused => &[(Stat::Acc, 10), (Stat::Crit, 5), (Stat::StressTaken, -25)],
            Virtue::Powerful => &[(Stat::Dmg, 25), (Stat::StressTaken, -25)],
            Virtue::Vigorous => &[
                (Stat::HealRecv, 25),
                (Stat::Dodge, 5),
                (Stat::StressTaken, -25),
            ],
        }
    }

    pub fn barks(self) -> &'static [&'static str] {
        match self {
            Virtue::Stalwart => &["I have stood against worse. Behind me, all of you."],
            Virtue::Courageous => &["Is that all the dark has? Then it has lost."],
            Virtue::Focused => &["I see it clearly now. Every weakness. Every opening."],
            Virtue::Powerful => &["The Light burns in my arm! Let them taste it!"],
            Virtue::Vigorous => &["My blood runs hot. I could walk these halls for days."],
        }
    }
}

/// A persistent trait, gained on expeditions.
#[derive(Clone, Copy, Debug)]
pub struct QuirkDef {
    pub id: &'static str,
    pub name: &'static str,
    pub positive: bool,
    pub mods: &'static [(Stat, i32)],
}

pub static QUIRKS: &[QuirkDef] = &[
    QuirkDef {
        id: "quick_reflexes",
        name: "Quick Reflexes",
        positive: true,
        mods: &[(Stat::Speed, 2)],
    },
    QuirkDef {
        id: "eagle_eye",
        name: "Eagle Eye",
        positive: true,
        mods: &[(Stat::Acc, 5), (Stat::Crit, 2)],
    },
    QuirkDef {
        id: "tough",
        name: "Tough",
        positive: true,
        mods: &[(Stat::Prot, 5)],
    },
    QuirkDef {
        id: "steady",
        name: "Steady",
        positive: true,
        mods: &[(Stat::StressTaken, -10)],
    },
    QuirkDef {
        id: "lightborn",
        name: "Lightborn",
        positive: true,
        mods: &[(Stat::HealRecv, 20)],
    },
    QuirkDef {
        id: "iron_gut",
        name: "Iron Gut",
        positive: true,
        mods: &[(Stat::BlightRes, 20)],
    },
    QuirkDef {
        id: "slippery",
        name: "Slippery",
        positive: true,
        mods: &[(Stat::Dodge, 5)],
    },
    QuirkDef {
        id: "hard_hitter",
        name: "Hard Hitter",
        positive: true,
        mods: &[(Stat::Dmg, 10)],
    },
    QuirkDef {
        id: "nervous",
        name: "Nervous",
        positive: false,
        mods: &[(Stat::StressTaken, 15)],
    },
    QuirkDef {
        id: "slow_reflexes",
        name: "Slow Reflexes",
        positive: false,
        mods: &[(Stat::Speed, -2)],
    },
    QuirkDef {
        id: "nearsighted",
        name: "Nearsighted",
        positive: false,
        mods: &[(Stat::Acc, -5)],
    },
    QuirkDef {
        id: "hemophilia",
        name: "Hemophilia",
        positive: false,
        mods: &[(Stat::BleedRes, -20)],
    },
    QuirkDef {
        id: "fragile",
        name: "Fragile",
        positive: false,
        mods: &[(Stat::Prot, -5), (Stat::HealRecv, -10)],
    },
    QuirkDef {
        id: "fear_of_undead",
        name: "Dread of the Dead",
        positive: false,
        mods: &[(Stat::StressTaken, 10), (Stat::Dmg, -5)],
    },
    QuirkDef {
        id: "clumsy",
        name: "Clumsy",
        positive: false,
        mods: &[(Stat::Dodge, -5)],
    },
    QuirkDef {
        id: "weak_grip",
        name: "Weak Grip",
        positive: false,
        mods: &[(Stat::Dmg, -10)],
    },
];

pub fn quirk(id: &str) -> Option<&'static QuirkDef> {
    QUIRKS.iter().find(|q| q.id == id)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hero {
    pub id: u32,
    pub name: String,
    pub class: HeroClass,
    pub hp: i32,
    pub stress: i32,
    pub level: u8,
    pub xp: i32,
    /// Blacksmith upgrades, 1..=5.
    pub weapon: u8,
    pub armor: u8,
    /// The four equipped skill ids, out of the class's six.
    pub skills: Vec<String>,
    pub quirks: Vec<String>,
    pub affliction: Option<Affliction>,
    #[serde(default)]
    pub virtue: Option<Virtue>,
    /// Seeds the hero's appearance: race, sex, face and hair.
    pub look_seed: u32,
    pub kills: u32,
    pub expeditions: u32,
    /// Busy recovering in town for the coming week.
    #[serde(default)]
    pub resting: bool,
}

impl Hero {
    pub fn new(id: u32, class: HeroClass, name: String, rng: &mut Rng) -> Hero {
        let def = class.def();
        let mut quirks = Vec::new();
        let positives: Vec<_> = QUIRKS.iter().filter(|q| q.positive).collect();
        let negatives: Vec<_> = QUIRKS.iter().filter(|q| !q.positive).collect();
        if let Some(q) = rng.pick(&positives) {
            quirks.push(q.id.to_owned());
        }
        if rng.percent(60) {
            if let Some(q) = rng.pick(&negatives) {
                quirks.push(q.id.to_owned());
            }
        }
        let mut hero = Hero {
            id,
            name,
            class,
            hp: 0,
            stress: rng.range(0, 25),
            level: 0,
            xp: 0,
            weapon: 1,
            armor: 1,
            skills: def.skills.iter().take(4).map(|s| s.id.to_owned()).collect(),
            quirks,
            affliction: None,
            virtue: None,
            look_seed: rng.next_u64() as u32,
            kills: 0,
            expeditions: 0,
            resting: false,
        };
        hero.hp = hero.max_hp();
        hero
    }

    pub fn class_name(&self) -> &'static str {
        self.class.def().name
    }

    pub fn equipped_skills(&self) -> Vec<&'static SkillDef> {
        let def = self.class.def();
        self.skills
            .iter()
            .filter_map(|id| def.skills.iter().find(|s| s.id == id))
            .collect()
    }

    pub fn toggle_skill(&mut self, id: &str) {
        if let Some(pos) = self.skills.iter().position(|s| s == id) {
            if self.skills.len() > 1 {
                self.skills.remove(pos);
            }
        } else if self.skills.len() < 4 && self.class.def().skills.iter().any(|s| s.id == id) {
            self.skills.push(id.to_owned());
        }
    }

    /// Every flat modifier the hero carries: quirks, then an affliction or virtue.
    pub fn mods(&self) -> Vec<(Stat, i32)> {
        let mut out: Vec<(Stat, i32)> = self
            .quirks
            .iter()
            .filter_map(|q| quirk(q))
            .flat_map(|q| q.mods.iter().copied())
            .collect();
        if let Some(a) = self.affliction {
            out.extend_from_slice(a.mods());
        }
        if let Some(v) = self.virtue {
            out.extend_from_slice(v.mods());
        }
        out
    }

    pub fn mod_total(&self, stat: Stat) -> i32 {
        self.mods()
            .into_iter()
            .filter(|(s, _)| *s == stat)
            .map(|(_, v)| v)
            .sum()
    }

    pub fn max_hp(&self) -> i32 {
        let base = self.class.def().base.max_hp;
        // Each armour tier adds a sixth of the base; each resolve level a little more.
        base + (base * i32::from(self.armor - 1)) / 6 + i32::from(self.level)
    }

    /// The base stats with level, gear and quirks folded in; battle buffs come on top.
    pub fn stats(&self) -> Stats {
        let b = self.class.def().base;
        let w = i32::from(self.weapon - 1);
        let a = i32::from(self.armor - 1);
        let lvl = i32::from(self.level);
        let r = b.res;
        Stats {
            max_hp: self.max_hp(),
            dodge: b.dodge + a * 4 + lvl * 2,
            prot: b.prot,
            speed: b.speed + w / 2,
            acc: b.acc + lvl * 3,
            crit: b.crit + w,
            dmg: (b.dmg.0 + w * b.dmg.0 / 5, b.dmg.1 + w * b.dmg.1 / 5),
            res: Resists {
                stun: r.stun + lvl * 5,
                blight: r.blight + lvl * 5,
                bleed: r.bleed + lvl * 5,
                debuff: r.debuff + lvl * 5,
                moves: r.moves + lvl * 5,
                death_blow: r.death_blow + lvl * 2,
            },
        }
    }

    pub fn grant_xp(&mut self, xp: i32) -> bool {
        self.xp += xp;
        let mut leveled = false;
        while self.level < MAX_LEVEL && self.xp >= LEVEL_XP[usize::from(self.level + 1)] {
            self.level += 1;
            leveled = true;
        }
        leveled
    }

    /// Gold the blacksmith wants for the next weapon or armour tier.
    pub fn upgrade_cost(tier: u8) -> i32 {
        [0, 750, 1500, 2600, 4000, 0][usize::from(tier.min(5))]
    }
}

/// What a stress change did to a hero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StressOutcome {
    None,
    Afflicted(Affliction),
    Virtuous(Virtue),
    /// Stress hit the cap: the heart gives out.
    HeartAttack,
}

/// Apply raw stress to a hero's meter, scaled by their stress-taken modifier, running the resolve
/// test at 100 and the heart attack at 200. Returns the change applied and what came of it.
pub fn apply_stress(
    stress: &mut i32,
    affliction: &mut Option<Affliction>,
    virtue: &mut Option<Virtue>,
    amount: i32,
    taken_pct: i32,
    rng: &mut Rng,
) -> (i32, StressOutcome) {
    let scaled = if amount > 0 {
        (amount * (100 + taken_pct).max(10) + 50) / 100
    } else {
        amount
    };
    let before = *stress;
    *stress = (*stress + scaled).clamp(0, STRESS_MAX);
    let mut outcome = StressOutcome::None;
    if before < STRESS_RESOLVE
        && *stress >= STRESS_RESOLVE
        && affliction.is_none()
        && virtue.is_none()
    {
        // The resolve test: one in four rise to the occasion.
        if rng.percent(25) {
            let v = *rng.pick(&Virtue::ALL).expect("virtues");
            *virtue = Some(v);
            *stress = 45;
            outcome = StressOutcome::Virtuous(v);
        } else {
            let a = *rng.pick(&Affliction::ALL).expect("afflictions");
            *affliction = Some(a);
            outcome = StressOutcome::Afflicted(a);
        }
    }
    if *stress >= STRESS_MAX {
        *stress = 170;
        outcome = StressOutcome::HeartAttack;
    }
    (*stress - before, outcome)
}

/// Names for recruits, drawn without regard to class; some are Azerothian, all are original.
pub static HERO_NAMES: &[&str] = &[
    "Aldric", "Brennan", "Corwin", "Dagna", "Edric", "Fenna", "Garrick", "Hilde", "Isolde",
    "Jorund", "Kestrel", "Liesel", "Morven", "Nessa", "Osric", "Perrin", "Quenna", "Rurik",
    "Sabine", "Tobin", "Ulla", "Varik", "Wren", "Yorick", "Zelda", "Bram", "Cassia", "Doran",
    "Elspeth", "Fargus", "Greta", "Halvard", "Ilsa", "Joss", "Kara", "Lucan", "Mira", "Niall",
    "Orla", "Piet", "Rowan", "Sigrun", "Thane", "Vesna", "Wilhelm", "Ysolde", "Ansel", "Brynja",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_hero_is_full_health_with_four_skills() {
        let mut rng = Rng::new(1);
        let h = Hero::new(1, HeroClass::Warrior, "Test".into(), &mut rng);
        assert_eq!(h.hp, h.max_hp());
        assert_eq!(h.skills.len(), 4);
        assert_eq!(h.equipped_skills().len(), 4);
    }

    #[test]
    fn toggling_keeps_at_most_four() {
        let mut rng = Rng::new(1);
        let mut h = Hero::new(1, HeroClass::Rogue, "Test".into(), &mut rng);
        h.toggle_skill("throw");
        assert_eq!(h.skills.len(), 4, "a fifth is refused");
        h.toggle_skill("rupture");
        assert_eq!(h.skills.len(), 3);
        h.toggle_skill("throw");
        assert!(h.skills.iter().any(|s| s == "throw"));
    }

    #[test]
    fn crossing_one_hundred_tests_resolve() {
        let mut rng = Rng::new(3);
        for _ in 0..50 {
            let (mut s, mut a, mut v) = (95, None, None);
            let (_, out) = apply_stress(&mut s, &mut a, &mut v, 10, 0, &mut rng);
            assert!(matches!(
                out,
                StressOutcome::Afflicted(_) | StressOutcome::Virtuous(_)
            ));
        }
    }

    #[test]
    fn two_hundred_is_a_heart_attack() {
        let mut rng = Rng::new(3);
        let (mut s, mut a, mut v) = (190, Some(Affliction::Fearful), None);
        let (_, out) = apply_stress(&mut s, &mut a, &mut v, 20, 0, &mut rng);
        assert_eq!(out, StressOutcome::HeartAttack);
    }

    #[test]
    fn leveling_follows_the_table() {
        let mut rng = Rng::new(1);
        let mut h = Hero::new(1, HeroClass::Mage, "T".into(), &mut rng);
        assert!(h.grant_xp(4));
        assert_eq!(h.level, 2);
    }
}
