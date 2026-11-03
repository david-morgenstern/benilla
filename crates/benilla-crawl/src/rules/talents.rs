//! Talents: a hero earns a choice at resolve levels 1, 3 and 5, picking one of two perks per tier.
//! Choices are permanent; the training hall can unlearn them for gold.

use super::data::{HeroClass, Stat};

#[derive(Debug)]
pub struct TalentDef {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub desc: &'static str,
    pub mods: &'static [(Stat, i32)],
}

/// The resolve level each talent tier opens at.
pub const TIER_LEVELS: [u8; 3] = [1, 3, 5];

const fn tal(
    id: &'static str,
    name: &'static str,
    icon: &'static str,
    desc: &'static str,
    mods: &'static [(Stat, i32)],
) -> TalentDef {
    TalentDef {
        id,
        name,
        icon,
        desc,
        mods,
    }
}

/// A class's talents: three tiers of two choices.
pub fn tree(class: HeroClass) -> &'static [[TalentDef; 2]; 3] {
    match class {
        HeroClass::Warrior => &WARRIOR,
        HeroClass::Paladin => &PALADIN,
        HeroClass::Rogue => &ROGUE,
        HeroClass::Priest => &PRIEST,
        HeroClass::Mage => &MAGE,
        HeroClass::Hunter => &HUNTER,
        HeroClass::Warlock => &WARLOCK,
    }
}

pub fn talent(class: HeroClass, id: &str) -> Option<&'static TalentDef> {
    tree(class).iter().flatten().find(|t| t.id == id)
}

static WARRIOR: [[TalentDef; 2]; 3] = [
    [
        tal(
            "w_toughness",
            "Toughness",
            "Spell_Holy_Devotion",
            "+15% max HP.",
            &[(Stat::MaxHp, 15)],
        ),
        tal(
            "w_cruelty",
            "Cruelty",
            "Ability_Rogue_Eviscerate",
            "+5% crit.",
            &[(Stat::Crit, 5)],
        ),
    ],
    [
        tal(
            "w_shield_spec",
            "Shield Specialization",
            "INV_Shield_06",
            "+10% protection, +10% stun resist.",
            &[(Stat::Prot, 10), (Stat::StunRes, 10)],
        ),
        tal(
            "w_two_handed",
            "Two-Handed Specialization",
            "INV_Axe_09",
            "+15% damage, -3 dodge.",
            &[(Stat::Dmg, 15), (Stat::Dodge, -3)],
        ),
    ],
    [
        tal(
            "w_last_stand",
            "Last Stand",
            "Spell_Holy_AshesToAshes",
            "+15% death blow resist.",
            &[(Stat::DeathBlowRes, 15)],
        ),
        tal(
            "w_death_wish",
            "Death Wish",
            "Spell_Shadow_DeathPact",
            "+20% damage, +10% stress taken.",
            &[(Stat::Dmg, 20), (Stat::StressTaken, 10)],
        ),
    ],
];

static PALADIN: [[TalentDef; 2]; 3] = [
    [
        tal(
            "p_divine_str",
            "Divine Strength",
            "Ability_GolemThunderClap",
            "+10% damage.",
            &[(Stat::Dmg, 10)],
        ),
        tal(
            "p_healing_light",
            "Healing Light",
            "Spell_Holy_HolyBolt",
            "+20% healing done.",
            &[(Stat::HealOut, 20)],
        ),
    ],
    [
        tal(
            "p_toughness",
            "Toughness",
            "Spell_Holy_Devotion",
            "+8% protection, +10% max HP.",
            &[(Stat::Prot, 8), (Stat::MaxHp, 10)],
        ),
        tal(
            "p_conviction",
            "Conviction",
            "Spell_Holy_RetributionAura",
            "+5% crit, +5 accuracy.",
            &[(Stat::Crit, 5), (Stat::Acc, 5)],
        ),
    ],
    [
        tal(
            "p_unbreakable",
            "Unbreakable Faith",
            "Spell_Holy_SealOfSalvation",
            "-20% stress taken.",
            &[(Stat::StressTaken, -20)],
        ),
        tal(
            "p_vengeance",
            "Vengeance",
            "Ability_Racial_Avatar",
            "+20% damage, -5% protection.",
            &[(Stat::Dmg, 20), (Stat::Prot, -5)],
        ),
    ],
];

static ROGUE: [[TalentDef; 2]; 3] = [
    [
        tal(
            "r_malice",
            "Malice",
            "Ability_Racial_BloodRage",
            "+5% crit.",
            &[(Stat::Crit, 5)],
        ),
        tal(
            "r_lightning",
            "Lightning Reflexes",
            "Spell_Nature_Invisibilty",
            "+8 dodge.",
            &[(Stat::Dodge, 8)],
        ),
    ],
    [
        tal(
            "r_precision",
            "Precision",
            "Ability_Marksmanship",
            "+8 accuracy.",
            &[(Stat::Acc, 8)],
        ),
        tal(
            "r_vile_poisons",
            "Vile Poisons",
            "Ability_Rogue_FeignDeath",
            "+15% damage.",
            &[(Stat::Dmg, 15)],
        ),
    ],
    [
        tal(
            "r_adrenaline",
            "Adrenaline Rush",
            "Spell_Shadow_ShadowWordDominate",
            "+2 speed, +5% crit.",
            &[(Stat::Speed, 2), (Stat::Crit, 5)],
        ),
        tal(
            "r_cheat_death",
            "Cheat Death",
            "Ability_Rogue_CheatDeath",
            "+20% death blow resist, +5 dodge.",
            &[(Stat::DeathBlowRes, 20), (Stat::Dodge, 5)],
        ),
    ],
];

static PRIEST: [[TalentDef; 2]; 3] = [
    [
        tal(
            "pr_spiritual_heal",
            "Spiritual Healing",
            "Spell_Nature_MoonGlow",
            "+20% healing done.",
            &[(Stat::HealOut, 20)],
        ),
        tal(
            "pr_unbreakable_will",
            "Unbreakable Will",
            "Spell_Magic_MageArmor",
            "+20% stun and debuff resist.",
            &[(Stat::StunRes, 20), (Stat::DebuffRes, 20)],
        ),
    ],
    [
        tal(
            "pr_inner_focus",
            "Inner Focus",
            "Spell_Frost_WindWalkOn",
            "-15% stress taken.",
            &[(Stat::StressTaken, -15)],
        ),
        tal(
            "pr_holy_spec",
            "Holy Specialization",
            "Spell_Holy_SealOfSalvation",
            "+6% crit, +10% damage.",
            &[(Stat::Crit, 6), (Stat::Dmg, 10)],
        ),
    ],
    [
        tal(
            "pr_spirit_redemption",
            "Spirit of Redemption",
            "INV_Enchant_EssenceEternalLarge",
            "+20% death blow resist.",
            &[(Stat::DeathBlowRes, 20)],
        ),
        tal(
            "pr_divine_fury",
            "Divine Fury",
            "Spell_Holy_SealOfFury",
            "+2 speed, +10% healing done.",
            &[(Stat::Speed, 2), (Stat::HealOut, 10)],
        ),
    ],
];

static MAGE: [[TalentDef; 2]; 3] = [
    [
        tal(
            "m_arcane_focus",
            "Arcane Focus",
            "Spell_Holy_Devotion",
            "+8 accuracy.",
            &[(Stat::Acc, 8)],
        ),
        tal(
            "m_ignite",
            "Ignite",
            "Spell_Fire_Incinerate",
            "+6% crit.",
            &[(Stat::Crit, 6)],
        ),
    ],
    [
        tal(
            "m_ice_barrier",
            "Ice Barrier",
            "Spell_Ice_Lament",
            "+10% protection, +10% max HP.",
            &[(Stat::Prot, 10), (Stat::MaxHp, 10)],
        ),
        tal(
            "m_arcane_power",
            "Arcane Power",
            "Spell_Nature_Lightning",
            "+20% damage, +10% stress taken.",
            &[(Stat::Dmg, 20), (Stat::StressTaken, 10)],
        ),
    ],
    [
        tal(
            "m_presence",
            "Presence of Mind",
            "Spell_Nature_EnchantArmor",
            "+3 speed.",
            &[(Stat::Speed, 3)],
        ),
        tal(
            "m_combustion",
            "Combustion",
            "Spell_Fire_SealOfFire",
            "+10% crit, -5 dodge.",
            &[(Stat::Crit, 10), (Stat::Dodge, -5)],
        ),
    ],
];

static HUNTER: [[TalentDef; 2]; 3] = [
    [
        tal(
            "h_lethal_shots",
            "Lethal Shots",
            "Ability_SearingArrow",
            "+5% crit.",
            &[(Stat::Crit, 5)],
        ),
        tal(
            "h_survivalist",
            "Survivalist",
            "Spell_Shadow_Twilight",
            "+15% max HP.",
            &[(Stat::MaxHp, 15)],
        ),
    ],
    [
        tal(
            "h_efficiency",
            "Efficiency",
            "Spell_Frost_WizardMark",
            "+10 accuracy.",
            &[(Stat::Acc, 10)],
        ),
        tal(
            "h_deflection",
            "Deflection",
            "Ability_Parry",
            "+8 dodge, +10% stun resist.",
            &[(Stat::Dodge, 8), (Stat::StunRes, 10)],
        ),
    ],
    [
        tal(
            "h_trueshot",
            "Trueshot Aura",
            "Ability_TrueShot",
            "+15% damage, +5% crit.",
            &[(Stat::Dmg, 15), (Stat::Crit, 5)],
        ),
        tal(
            "h_lightning_reflex",
            "Lightning Reflexes",
            "Spell_Nature_Invisibilty",
            "+2 speed, +5 dodge.",
            &[(Stat::Speed, 2), (Stat::Dodge, 5)],
        ),
    ],
];

static WARLOCK: [[TalentDef; 2]; 3] = [
    [
        tal(
            "wl_demonic_emb",
            "Demonic Embrace",
            "Spell_Shadow_Metamorphosis",
            "+20% max HP, -5% stress resistance.",
            &[(Stat::MaxHp, 20), (Stat::StressTaken, 5)],
        ),
        tal(
            "wl_bane",
            "Bane",
            "Spell_Shadow_DeathPact",
            "+10% damage.",
            &[(Stat::Dmg, 10)],
        ),
    ],
    [
        tal(
            "wl_fel_concentration",
            "Fel Concentration",
            "Spell_Shadow_FingerOfDeath",
            "+20% debuff and stun resist.",
            &[(Stat::DebuffRes, 20), (Stat::StunRes, 20)],
        ),
        tal(
            "wl_devastation",
            "Devastation",
            "Spell_Fire_FlameShock",
            "+6% crit.",
            &[(Stat::Crit, 6)],
        ),
    ],
    [
        tal(
            "wl_shadow_mastery",
            "Shadow Mastery",
            "Spell_Shadow_ShadeTrueSight",
            "+20% damage.",
            &[(Stat::Dmg, 20)],
        ),
        tal(
            "wl_soul_link",
            "Soul Link",
            "Spell_Shadow_GatherShadows",
            "-15% stress taken, +5% protection.",
            &[(Stat::StressTaken, -15), (Stat::Prot, 5)],
        ),
    ],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_has_a_full_tree_with_unique_ids() {
        let mut all = Vec::new();
        for c in HeroClass::ALL {
            for tier in tree(c) {
                for t in tier {
                    all.push(t.id);
                }
            }
        }
        let n = all.len();
        all.sort();
        all.dedup();
        assert_eq!(n, all.len());
        assert_eq!(n, 7 * 6);
    }
}
