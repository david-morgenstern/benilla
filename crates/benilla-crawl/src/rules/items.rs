//! Trinkets: the double-edged charms a hero wears two of. Most give with one hand and take with
//! the other; the rarest are bound to a class.

use serde::{Deserialize, Serialize};

use super::data::{HeroClass, Stat};
use super::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    VeryRare,
    Ancestral,
}

impl Rarity {
    pub fn name(self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Uncommon => "Uncommon",
            Rarity::Rare => "Rare",
            Rarity::VeryRare => "Very Rare",
            Rarity::Ancestral => "Ancestral",
        }
    }

    /// The rarity's colour, as WoW item quality colours (linear sRGB components).
    pub fn color(self) -> (f32, f32, f32) {
        match self {
            Rarity::Common => (0.85, 0.85, 0.85),
            Rarity::Uncommon => (0.12, 1.0, 0.0),
            Rarity::Rare => (0.0, 0.44, 0.87),
            Rarity::VeryRare => (0.64, 0.21, 0.93),
            Rarity::Ancestral => (1.0, 0.5, 0.0),
        }
    }

    /// Drop weight at a dungeon level (1..=3): deeper runs find rarer things.
    fn weight(self, level: u8) -> u32 {
        let l = u32::from(level.clamp(1, 3));
        match self {
            Rarity::Common => 40,
            Rarity::Uncommon => 25 + 5 * l,
            Rarity::Rare => 6 + 6 * l,
            Rarity::VeryRare => 2 * l,
            Rarity::Ancestral => 0,
        }
    }
}

#[derive(Debug)]
pub struct TrinketDef {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub rarity: Rarity,
    /// Only this class may wear it.
    pub class: Option<HeroClass>,
    pub mods: &'static [(Stat, i32)],
    pub lore: &'static str,
}

impl TrinketDef {
    /// Gold the nomad wagon asks; it buys back at a quarter.
    pub fn price(&self) -> i32 {
        match self.rarity {
            Rarity::Common => 1500,
            Rarity::Uncommon => 3500,
            Rarity::Rare => 6500,
            Rarity::VeryRare => 10000,
            Rarity::Ancestral => 20000,
        }
    }

    pub fn summary(&self) -> String {
        let mods: Vec<String> = self
            .mods
            .iter()
            .map(|(s, v)| format!("{v:+}{} {}", if pct(*s) { "%" } else { "" }, s.label()))
            .collect();
        let class = self
            .class
            .map_or(String::new(), |c| format!("\n{} only", c.def().name));
        format!(
            "{} ({})\n{}{class}\n\u{201c}{}\u{201d}",
            self.name,
            self.rarity.name(),
            mods.join("\n"),
            self.lore
        )
    }
}

/// Whether a stat's modifier reads as a percentage.
pub fn pct(s: Stat) -> bool {
    matches!(
        s,
        Stat::Crit
            | Stat::Dmg
            | Stat::Prot
            | Stat::StressTaken
            | Stat::HealRecv
            | Stat::HealOut
            | Stat::MaxHp
            | Stat::StunRes
            | Stat::BleedRes
            | Stat::BlightRes
            | Stat::DebuffRes
            | Stat::MoveRes
            | Stat::DeathBlowRes
    )
}

pub fn trinket(id: &str) -> Option<&'static TrinketDef> {
    TRINKETS.iter().find(|t| t.id == id)
}

const fn t(
    id: &'static str,
    name: &'static str,
    icon: &'static str,
    rarity: Rarity,
    class: Option<HeroClass>,
    mods: &'static [(Stat, i32)],
    lore: &'static str,
) -> TrinketDef {
    TrinketDef {
        id,
        name,
        icon,
        rarity,
        class,
        mods,
        lore,
    }
}

pub static TRINKETS: &[TrinketDef] = &[
    t(
        "tin_locket",
        "Tarnished Locket",
        "INV_Jewelry_Necklace_01",
        Rarity::Common,
        None,
        &[(Stat::StressTaken, -10), (Stat::Dodge, -3)],
        "A portrait inside, too faded to name.",
    ),
    t(
        "rabbit_foot",
        "Elwynn Rabbit's Foot",
        "INV_Misc_Food_54",
        Rarity::Common,
        None,
        &[(Stat::Dodge, 6), (Stat::Prot, -5)],
        "Lucky for someone. Not the rabbit.",
    ),
    t(
        "whetstone",
        "Coarse Whetstone",
        "INV_Stone_SharpeningStone_01",
        Rarity::Common,
        None,
        &[(Stat::Dmg, 10), (Stat::Acc, -3)],
        "Sharper edges, shakier hands.",
    ),
    t(
        "candle_stub",
        "Kobold Candle Stub",
        "INV_Misc_Candle_01",
        Rarity::Common,
        None,
        &[(Stat::Acc, 4), (Stat::StressTaken, 5)],
        "\u{201c}You no take!\u{201d}",
    ),
    t(
        "bone_charm",
        "Knucklebone Charm",
        "INV_Misc_Bone_02",
        Rarity::Common,
        None,
        &[(Stat::BlightRes, 25), (Stat::BleedRes, -10)],
        "Rattles when the dead are near.",
    ),
    t(
        "iron_band",
        "Plain Iron Band",
        "INV_Jewelry_Ring_02",
        Rarity::Common,
        None,
        &[(Stat::Prot, 5), (Stat::Speed, -1)],
        "Heavy, honest, unremarkable.",
    ),
    t(
        "tavern_token",
        "Tavern Token",
        "INV_Misc_Coin_04",
        Rarity::Common,
        None,
        &[(Stat::StressTaken, -8), (Stat::Acc, -2)],
        "Good for one drink. Worth more down here.",
    ),
    t(
        "murloc_fin",
        "Dried Murloc Fin",
        "INV_Misc_MonsterScales_02",
        Rarity::Uncommon,
        None,
        &[(Stat::Speed, 2), (Stat::HealRecv, -10)],
        "Still smells of the tide.",
    ),
    t(
        "blood_vial",
        "Vial of Old Blood",
        "INV_Potion_02",
        Rarity::Uncommon,
        None,
        &[(Stat::Crit, 4), (Stat::MaxHp, -10)],
        "It does not settle when shaken.",
    ),
    t(
        "warding_rune",
        "Warding Rune",
        "INV_Misc_Rune_01",
        Rarity::Uncommon,
        None,
        &[(Stat::DebuffRes, 25), (Stat::StunRes, 15)],
        "Carved by a hand that knew what it feared.",
    ),
    t(
        "pilgrim_beads",
        "Pilgrim's Prayer Beads",
        "INV_Jewelry_Necklace_03",
        Rarity::Uncommon,
        None,
        &[(Stat::HealRecv, 20), (Stat::Dmg, -5)],
        "Worn smooth by a thousand prayers.",
    ),
    t(
        "spiked_collar",
        "Spiked Collar",
        "INV_Jewelry_Necklace_05",
        Rarity::Uncommon,
        None,
        &[(Stat::Dmg, 15), (Stat::StressTaken, 10)],
        "Worn by something that did not want it.",
    ),
    t(
        "bandit_mask",
        "Bandit's Cloth Mask",
        "INV_Misc_Bandana_03",
        Rarity::Uncommon,
        None,
        &[(Stat::Dodge, 8), (Stat::Crit, 2), (Stat::HealRecv, -15)],
        "The red kerchief of an old brotherhood.",
    ),
    t(
        "lantern_glass",
        "Cracked Lantern Glass",
        "INV_Misc_Gem_Diamond_01",
        Rarity::Uncommon,
        None,
        &[(Stat::Acc, 6), (Stat::Crit, 2), (Stat::Dodge, -4)],
        "The world looks clearer through the crack.",
    ),
    t(
        "ward_shield",
        "Footman's Badge",
        "INV_Shield_05",
        Rarity::Uncommon,
        Some(HeroClass::Warrior),
        &[(Stat::Prot, 12), (Stat::MaxHp, 10)],
        "Third Battalion. All of them dead but this one.",
    ),
    t(
        "libram",
        "Libram of Vigil",
        "INV_Misc_Book_11",
        Rarity::Uncommon,
        Some(HeroClass::Paladin),
        &[(Stat::HealOut, 20), (Stat::StressTaken, -10)],
        "Its last page is still blank.",
    ),
    t(
        "poison_vial",
        "Viper Venom Vial",
        "INV_Potion_19",
        Rarity::Uncommon,
        Some(HeroClass::Rogue),
        &[(Stat::Crit, 6), (Stat::Dmg, 10), (Stat::Dodge, -5)],
        "Handle with gloves. Or don't.",
    ),
    t(
        "holy_symbol",
        "Silver Holy Symbol",
        "INV_Jewelry_Talisman_01",
        Rarity::Uncommon,
        Some(HeroClass::Priest),
        &[(Stat::HealOut, 25), (Stat::Dodge, -4)],
        "It warms in the presence of the wounded.",
    ),
    t(
        "focus_crystal",
        "Focusing Crystal",
        "INV_Misc_Gem_Sapphire_02",
        Rarity::Uncommon,
        Some(HeroClass::Mage),
        &[(Stat::Acc, 8), (Stat::Crit, 4), (Stat::Prot, -5)],
        "Dalaran craftsmanship, Dalaran prices.",
    ),
    t(
        "hawk_feather",
        "Hawk Feather Fletching",
        "INV_Feather_04",
        Rarity::Uncommon,
        Some(HeroClass::Hunter),
        &[(Stat::Acc, 8), (Stat::Speed, 1)],
        "Arrows fletched with it fly true.",
    ),
    t(
        "soul_shard",
        "Soul Shard",
        "INV_Misc_Gem_Amethyst_02",
        Rarity::Uncommon,
        Some(HeroClass::Warlock),
        &[(Stat::Dmg, 20), (Stat::MaxHp, -15)],
        "Someone is still in there.",
    ),
    t(
        "lich_phylactery_chip",
        "Phylactery Fragment",
        "INV_Misc_Gem_Variety_02",
        Rarity::Rare,
        None,
        &[(Stat::DeathBlowRes, 15), (Stat::StressTaken, 15)],
        "A shard of a vessel that held a soul. It wants another.",
    ),
    t(
        "stormwind_crest",
        "Stormwind Signet",
        "INV_Jewelry_Ring_15",
        Rarity::Rare,
        None,
        &[(Stat::Acc, 5), (Stat::Dmg, 10), (Stat::Prot, 5)],
        "The lion still roars, if faintly.",
    ),
    t(
        "worgen_fang",
        "Worgen Fang",
        "INV_Misc_Bone_08",
        Rarity::Rare,
        None,
        &[(Stat::Crit, 6), (Stat::Speed, 2), (Stat::StressTaken, 10)],
        "From the woods of Duskwood. The bite still itches.",
    ),
    t(
        "ancient_idol",
        "Ancient Idol",
        "INV_Misc_Idol_02",
        Rarity::Rare,
        None,
        &[(Stat::MaxHp, 15), (Stat::DebuffRes, 20), (Stat::Speed, -1)],
        "Older than the crypt. Older than the hamlet.",
    ),
    t(
        "tide_pearl",
        "Black Tide Pearl",
        "INV_Misc_Gem_Pearl_04",
        Rarity::Rare,
        None,
        &[
            (Stat::HealRecv, 25),
            (Stat::StressTaken, -10),
            (Stat::Dmg, -5),
        ],
        "It hums with the voice of the deep.",
    ),
    t(
        "battle_standard",
        "Tattered Battle Standard",
        "INV_BannerPVP_02",
        Rarity::Rare,
        Some(HeroClass::Warrior),
        &[(Stat::Dmg, 20), (Stat::DeathBlowRes, 10), (Stat::Dodge, -5)],
        "Carried at the front of every lost cause.",
    ),
    t(
        "ashbringer_shard",
        "Light-Touched Shard",
        "INV_Misc_Gem_Crystal_02",
        Rarity::Rare,
        Some(HeroClass::Paladin),
        &[(Stat::Dmg, 15), (Stat::HealOut, 15), (Stat::Prot, 5)],
        "It burns the hand that does not believe.",
    ),
    t(
        "shadow_cloak_clasp",
        "Shadowcloak Clasp",
        "INV_Misc_Cape_06",
        Rarity::Rare,
        Some(HeroClass::Rogue),
        &[(Stat::Dodge, 12), (Stat::Crit, 5), (Stat::MaxHp, -10)],
        "Nobody saw it fastened. Nobody sees it worn.",
    ),
    t(
        "tome_of_tides",
        "Tome of the Quiet Light",
        "INV_Misc_Book_07",
        Rarity::Rare,
        Some(HeroClass::Priest),
        &[
            (Stat::HealOut, 30),
            (Stat::StressTaken, -15),
            (Stat::Speed, -1),
        ],
        "Its words calm even the reader.",
    ),
    t(
        "archmage_ring",
        "Kirin Tor Band",
        "INV_Jewelry_Ring_14",
        Rarity::Rare,
        Some(HeroClass::Mage),
        &[(Stat::Dmg, 20), (Stat::Crit, 5), (Stat::MaxHp, -10)],
        "Violet, of course.",
    ),
    t(
        "beast_whistle",
        "Beastmaster's Whistle",
        "INV_Misc_Horn_01",
        Rarity::Rare,
        Some(HeroClass::Hunter),
        &[(Stat::Crit, 8), (Stat::Acc, 5), (Stat::StressTaken, -5)],
        "Something out there still answers.",
    ),
    t(
        "demon_eye",
        "Eye of the Pit",
        "INV_Misc_Eye_01",
        Rarity::Rare,
        Some(HeroClass::Warlock),
        &[(Stat::Dmg, 25), (Stat::Crit, 5), (Stat::StressTaken, 20)],
        "It blinks when you are not looking.",
    ),
    t(
        "archivist_quill",
        "The Archivist's Quill",
        "INV_Feather_05",
        Rarity::VeryRare,
        None,
        &[(Stat::Crit, 8), (Stat::Acc, 8), (Stat::StressTaken, 15)],
        "It writes on its own. Mostly names.",
    ),
    t(
        "broodmother_eye",
        "Broodmother's Eye",
        "INV_Misc_Gem_Ruby_02",
        Rarity::VeryRare,
        None,
        &[(Stat::Speed, 3), (Stat::Dodge, 10), (Stat::HealRecv, -20)],
        "Eight of them watched you. Now one does.",
    ),
    t(
        "matriarch_scale",
        "Matriarch's Scale",
        "INV_Misc_MonsterScales_05",
        Rarity::VeryRare,
        None,
        &[(Stat::Prot, 15), (Stat::MaxHp, 20), (Stat::Speed, -2)],
        "Cold, and never quite dry.",
    ),
    t(
        "ancestors_signet",
        "The Ancestor's Signet",
        "INV_Jewelry_Ring_37",
        Rarity::Ancestral,
        None,
        &[
            (Stat::Dmg, 15),
            (Stat::Acc, 5),
            (Stat::Crit, 5),
            (Stat::StressTaken, -10),
        ],
        "The family crest, pressed into every deed of the estate.",
    ),
];

/// A random trinket fit for a dungeon level, never ancestral.
pub fn roll_trinket(rng: &mut Rng, level: u8) -> &'static TrinketDef {
    let pool: Vec<&TrinketDef> = TRINKETS
        .iter()
        .filter(|t| t.rarity != Rarity::Ancestral)
        .collect();
    let weights: Vec<u32> = pool.iter().map(|t| t.rarity.weight(level)).collect();
    pool[rng.weighted(&weights).unwrap_or(0)]
}

/// A trinket of at least `min` rarity.
pub fn roll_trinket_at_least(rng: &mut Rng, min: Rarity) -> &'static TrinketDef {
    let pool: Vec<&TrinketDef> = TRINKETS
        .iter()
        .filter(|t| t.rarity >= min && t.rarity != Rarity::Ancestral)
        .collect();
    pool[rng.index(pool.len())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = TRINKETS.iter().map(|t| t.id).collect();
        ids.sort();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn drops_are_never_ancestral() {
        let mut rng = Rng::new(1);
        for _ in 0..500 {
            assert_ne!(roll_trinket(&mut rng, 3).rarity, Rarity::Ancestral);
        }
    }
}
