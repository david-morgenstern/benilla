//! Camping: on longer expeditions the party may make camp in a cleared room, eat, and spend twelve
//! hours of camp skills on one another before the night passes, or is interrupted.

use super::data::{HeroClass, Stat};

pub const CAMP_HOURS: u8 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampTarget {
    Myself,
    Ally,
    Party,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CampEffect {
    /// Heal a percent of max HP.
    Heal(i32),
    StressHeal(i32),
    /// A buff that lasts until the end of the next battle.
    Buff(Stat, i32),
    /// Removes bleeding, blight and the effects of hunger.
    Cure,
    /// The night ambush is less likely.
    Ward(i32),
}

#[derive(Debug)]
pub struct CampSkill {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub desc: &'static str,
    pub hours: u8,
    pub target: CampTarget,
    pub effects: &'static [CampEffect],
    /// `None`: every hero knows it.
    pub class: Option<HeroClass>,
}

const fn cs(
    id: &'static str,
    name: &'static str,
    icon: &'static str,
    desc: &'static str,
    hours: u8,
    target: CampTarget,
    effects: &'static [CampEffect],
    class: Option<HeroClass>,
) -> CampSkill {
    CampSkill {
        id,
        name,
        icon,
        desc,
        hours,
        target,
        effects,
        class,
    }
}

use CampEffect::*;
use CampTarget::*;

pub static CAMP_SKILLS: &[CampSkill] = &[
    cs(
        "encourage",
        "Encourage",
        "Spell_Holy_PowerInfusion",
        "A few kind words. -15 stress on an ally.",
        2,
        Ally,
        &[StressHeal(15)],
        None,
    ),
    cs(
        "first_aid",
        "First Aid",
        "Spell_Holy_SealOfSacrifice",
        "Bandage an ally: heal 15%.",
        2,
        Ally,
        &[Heal(15), Cure],
        None,
    ),
    cs(
        "pep_talk",
        "Pep Talk",
        "Ability_Warrior_BattleShout",
        "+10% stress resist for an ally.",
        2,
        Ally,
        &[Buff(Stat::StressTaken, -10)],
        None,
    ),
    cs(
        "w_war_stories",
        "War Stories",
        "Ability_Warrior_WarCry",
        "Tales of old victories: -10 stress for the party, +10% damage for self.",
        3,
        Party,
        &[StressHeal(10)],
        Some(HeroClass::Warrior),
    ),
    cs(
        "w_sharpen",
        "Sharpen Blades",
        "INV_Stone_SharpeningStone_05",
        "+15% damage, +5% crit for self.",
        2,
        Myself,
        &[Buff(Stat::Dmg, 15), Buff(Stat::Crit, 5)],
        Some(HeroClass::Warrior),
    ),
    cs(
        "w_watch",
        "Stand Watch",
        "Ability_Defend",
        "The warrior keeps watch: the night is safer.",
        3,
        Myself,
        &[Ward(25)],
        Some(HeroClass::Warrior),
    ),
    cs(
        "p_vigil",
        "Holy Vigil",
        "Spell_Holy_SealOfWisdom",
        "Prayer through the night: -15 stress for the party.",
        4,
        Party,
        &[StressHeal(15)],
        Some(HeroClass::Paladin),
    ),
    cs(
        "p_lay_hands",
        "Lay on Hands",
        "Spell_Holy_LayOnHands",
        "Heal an ally 40%.",
        3,
        Ally,
        &[Heal(40), Cure],
        Some(HeroClass::Paladin),
    ),
    cs(
        "p_consecrate",
        "Consecrate the Ground",
        "Spell_Holy_InnerFire",
        "Hallowed camp: ambushes are rarer, +10% protection for the party.",
        4,
        Party,
        &[Ward(20), Buff(Stat::Prot, 10)],
        Some(HeroClass::Paladin),
    ),
    cs(
        "r_gamble",
        "Dice by Torchlight",
        "INV_Misc_Coin_02",
        "A game of chance: -20 stress for self and an ally.",
        2,
        Ally,
        &[StressHeal(20)],
        Some(HeroClass::Rogue),
    ),
    cs(
        "r_traps",
        "Set Traps",
        "Spell_Fire_SelfDestruct",
        "Snares around camp: ambushes far rarer.",
        3,
        Myself,
        &[Ward(35)],
        Some(HeroClass::Rogue),
    ),
    cs(
        "r_poisons",
        "Brew Poisons",
        "Trade_BrewPoison",
        "+20% damage, +8 accuracy for self.",
        3,
        Myself,
        &[Buff(Stat::Dmg, 20), Buff(Stat::Acc, 8)],
        Some(HeroClass::Rogue),
    ),
    cs(
        "pr_sermon",
        "Sermon",
        "Spell_Holy_PrayerOfHealing02",
        "-20 stress for the party.",
        4,
        Party,
        &[StressHeal(20)],
        Some(HeroClass::Priest),
    ),
    cs(
        "pr_mend",
        "Mend the Wounded",
        "Spell_Holy_Renew",
        "Heal the party 20%.",
        4,
        Party,
        &[Heal(20)],
        Some(HeroClass::Priest),
    ),
    cs(
        "pr_confession",
        "Hear Confession",
        "Spell_Holy_MindSooth",
        "-35 stress for an ally.",
        3,
        Ally,
        &[StressHeal(35)],
        Some(HeroClass::Priest),
    ),
    cs(
        "m_conjure",
        "Conjure Refreshments",
        "INV_Drink_18",
        "Heal the party 15% and -5 stress.",
        3,
        Party,
        &[Heal(15), StressHeal(5)],
        Some(HeroClass::Mage),
    ),
    cs(
        "m_study",
        "Study the Arcane",
        "INV_Misc_Book_06",
        "+10 accuracy, +5% crit for self.",
        2,
        Myself,
        &[Buff(Stat::Acc, 10), Buff(Stat::Crit, 5)],
        Some(HeroClass::Mage),
    ),
    cs(
        "m_ward",
        "Arcane Ward",
        "Spell_Holy_MagicalSentry",
        "A shimmering ward: ambushes rarer, +10% debuff resist for the party.",
        3,
        Party,
        &[Ward(20), Buff(Stat::DebuffRes, 10)],
        Some(HeroClass::Mage),
    ),
    cs(
        "h_hunt",
        "Forage",
        "INV_Misc_Food_15",
        "Fresh game: heal the party 15%.",
        3,
        Party,
        &[Heal(15)],
        Some(HeroClass::Hunter),
    ),
    cs(
        "h_scout",
        "Scout the Perimeter",
        "Ability_Hunter_EagleEye",
        "Ambushes far rarer.",
        2,
        Myself,
        &[Ward(30)],
        Some(HeroClass::Hunter),
    ),
    cs(
        "h_tend",
        "Tend Wounds",
        "INV_Misc_Bandage_12",
        "Heal an ally 30%, cure them.",
        2,
        Ally,
        &[Heal(30), Cure],
        Some(HeroClass::Hunter),
    ),
    cs(
        "wl_ritual",
        "Dark Ritual",
        "Spell_Shadow_RitualOfSacrifice",
        "+25% damage for the party; +10 stress on self.",
        4,
        Party,
        &[Buff(Stat::Dmg, 25)],
        Some(HeroClass::Warlock),
    ),
    cs(
        "wl_soothe",
        "Whispered Bargain",
        "Spell_Shadow_Charm",
        "-25 stress on self.",
        2,
        Myself,
        &[StressHeal(25)],
        Some(HeroClass::Warlock),
    ),
    cs(
        "wl_healthstone",
        "Create Healthstone",
        "INV_Stone_04",
        "Heal an ally 35%.",
        3,
        Ally,
        &[Heal(35)],
        Some(HeroClass::Warlock),
    ),
];

pub fn camp_skill(id: &str) -> Option<&'static CampSkill> {
    CAMP_SKILLS.iter().find(|s| s.id == id)
}

/// The camp skills a class knows: its own three and the shared ones.
pub fn known(class: HeroClass) -> Vec<&'static CampSkill> {
    CAMP_SKILLS
        .iter()
        .filter(|s| s.class.is_none() || s.class == Some(class))
        .collect()
}
