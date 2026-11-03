//! The hamlet between expeditions: the roster and the stagecoach, the tavern and chapel that take
//! stress off for coin, the sanitarium, the smith and the guild, the nomad wagon, the memorial,
//! the heirlooms that rebuild it all, and the week that turns between runs.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::data::HeroClass;
use super::dungeon::{Dungeon, Expedition, Heirloom, Item, Quest};
use super::hero::{Hero, HERO_NAMES, QUIRKS};
use super::items::{roll_trinket, trinket, Rarity};
use super::regions::{max_resolve_for, Length, QuestKind, QuestOffer, Region};
use super::rng::Rng;

pub const MAX_BUILDING: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Building {
    Stagecoach,
    Tavern,
    Chapel,
    Sanitarium,
    Blacksmith,
    Guild,
    Wagon,
}

impl Building {
    pub const ALL: [Building; 7] = [
        Building::Stagecoach,
        Building::Tavern,
        Building::Chapel,
        Building::Sanitarium,
        Building::Blacksmith,
        Building::Guild,
        Building::Wagon,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Building::Stagecoach => "Stagecoach",
            Building::Tavern => "The Drowned Lantern",
            Building::Chapel => "Chapel of the Quiet Light",
            Building::Sanitarium => "Sanitarium",
            Building::Blacksmith => "Blacksmith",
            Building::Guild => "Guild Hall",
            Building::Wagon => "Nomad Wagon",
        }
    }

    /// The two heirloom kinds an upgrade wants.
    pub fn heirlooms(self) -> (Heirloom, Heirloom) {
        match self {
            Building::Stagecoach => (Heirloom::Deed, Heirloom::Crest),
            Building::Tavern => (Heirloom::Bust, Heirloom::Deed),
            Building::Chapel => (Heirloom::Portrait, Heirloom::Bust),
            Building::Sanitarium => (Heirloom::Bust, Heirloom::Crest),
            Building::Blacksmith => (Heirloom::Crest, Heirloom::Deed),
            Building::Guild => (Heirloom::Portrait, Heirloom::Crest),
            Building::Wagon => (Heirloom::Portrait, Heirloom::Deed),
        }
    }

    /// Heirlooms of each kind for the next upgrade from `level`.
    pub fn upgrade_cost(level: u8) -> i32 {
        [6, 14, 26, 0][usize::from(level.min(3))]
    }

    pub fn upgrade_effect(self, level: u8) -> String {
        let l = i32::from(level);
        match self {
            Building::Stagecoach => format!(
                "{} recruits a week, roster of {}, veterans on the road",
                3 + l,
                9 + 3 * l
            ),
            Building::Tavern => format!(
                "relief {}–{} for {} gold",
                30 + 8 * l,
                60 + 8 * l,
                300 - 50 * l
            ),
            Building::Chapel => format!(
                "relief {}–{} for {} gold",
                40 + 8 * l,
                55 + 8 * l,
                350 - 50 * l
            ),
            Building::Sanitarium => format!("treatments for {} gold", 500 - 100 * l),
            Building::Blacksmith => format!("gear up to tier {}", 2 + l),
            Building::Guild => format!("skills up to rank {}", 2 + l),
            Building::Wagon => format!("{} trinkets on offer, rarer stock", 3 + l),
        }
    }
}

/// The week's happening in the hamlet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TownEvent {
    #[default]
    Quiet,
    TavernDiscount,
    ChapelClosed,
    Patron,
    ExtraRecruits,
    WagonSale,
    Festival,
}

impl TownEvent {
    pub fn text(self) -> &'static str {
        match self {
            TownEvent::Quiet => {
                "A quiet week. The fog lies thick, and no one speaks of the estate."
            }
            TownEvent::TavernDiscount => {
                "A cask of good ale arrived on the coach: the tavern charges half this week."
            }
            TownEvent::ChapelClosed => {
                "A sickness among the brothers: the chapel is closed this week."
            }
            TownEvent::Patron => {
                "A wealthy patron, eager to see the estate cleansed, sends 750 gold."
            }
            TownEvent::ExtraRecruits => {
                "Word of the estate's gold spreads: more recruits on the coach this week."
            }
            TownEvent::WagonSale => {
                "The nomad is in a generous mood: trinkets are cheaper this week."
            }
            TownEvent::Festival => "A festival of lanterns: every hero sheds a little stress.",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Buildings(pub HashMap<Building, u8>);

impl Buildings {
    pub fn level(&self, b: Building) -> u8 {
        self.0.get(&b).copied().unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Relief {
    Tavern,
    Chapel,
}

impl Relief {
    pub fn building(self) -> Building {
        match self {
            Relief::Tavern => Building::Tavern,
            Relief::Chapel => Building::Chapel,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Grave {
    pub hero: Hero,
    pub week: u32,
    pub cause: String,
}

/// What came back from an expedition, for the results screen.
#[derive(Clone, Debug, Default)]
pub struct Debrief {
    pub success: bool,
    pub quest: String,
    pub gold: i32,
    pub heirlooms: Vec<(Heirloom, i32)>,
    pub trinkets: Vec<String>,
    pub leveled: Vec<String>,
    pub quirks: Vec<String>,
    pub dead: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Campaign {
    pub week: u32,
    pub gold: i32,
    pub heirlooms: HashMap<Heirloom, i32>,
    pub roster: Vec<Hero>,
    pub graveyard: Vec<Grave>,
    pub stagecoach: Vec<Hero>,
    pub next_id: u32,
    pub rng: Rng,
    pub runs: u32,
    #[serde(default)]
    pub boss_slain: bool,
    #[serde(default)]
    pub buildings: Buildings,
    /// Trinkets in the hamlet's stash, by id.
    #[serde(default)]
    pub trinkets: Vec<String>,
    /// The nomad wagon's stock this week.
    #[serde(default)]
    pub wagon: Vec<String>,
    /// Expeditions on offer this week.
    #[serde(default)]
    pub offers: Vec<QuestOffer>,
    /// Expeditions run in each region, which unlocks its lair.
    #[serde(default)]
    pub region_runs: HashMap<Region, u32>,
    /// Bosses slain, by region.
    #[serde(default)]
    pub bosses: Vec<Region>,
    /// Heroes sent to the tavern or chapel this week, per building.
    #[serde(default)]
    pub relief_used: HashMap<Building, u8>,
    #[serde(default)]
    pub event: TownEvent,
}

impl Campaign {
    pub fn new(seed: Option<u64>) -> Campaign {
        let rng = seed.map_or_else(Rng::from_time, Rng::new);
        let mut c = Campaign {
            week: 1,
            gold: 2000,
            heirlooms: HashMap::new(),
            roster: Vec::new(),
            graveyard: Vec::new(),
            stagecoach: Vec::new(),
            next_id: 1,
            rng,
            runs: 0,
            boss_slain: false,
            buildings: Buildings::default(),
            trinkets: Vec::new(),
            wagon: Vec::new(),
            offers: Vec::new(),
            region_runs: HashMap::new(),
            bosses: Vec::new(),
            relief_used: HashMap::new(),
            event: TownEvent::Quiet,
        };
        for class in [
            HeroClass::Warrior,
            HeroClass::Rogue,
            HeroClass::Hunter,
            HeroClass::Priest,
        ] {
            let h = c.make_hero(class, 0);
            c.roster.push(h);
        }
        c.refresh_week();
        c
    }

    pub fn level(&self, b: Building) -> u8 {
        self.buildings.level(b)
    }

    pub fn roster_cap(&self) -> usize {
        9 + 3 * usize::from(self.level(Building::Stagecoach))
    }

    fn make_hero(&mut self, class: HeroClass, level: u8) -> Hero {
        let taken: Vec<&str> = self
            .roster
            .iter()
            .chain(&self.stagecoach)
            .map(|h| h.name.as_str())
            .collect();
        let free: Vec<&&str> = HERO_NAMES.iter().filter(|n| !taken.contains(n)).collect();
        let name = self.rng.pick(&free).map_or_else(
            || format!("Wanderer {}", self.next_id),
            |n| (**n).to_owned(),
        );
        let id = self.next_id;
        self.next_id += 1;
        let mut h = Hero::new(id, class, name, &mut self.rng);
        if level > 0 {
            h.grant_xp(super::hero::LEVEL_XP[usize::from(level)]);
            h.weapon = level.min(4);
            h.armor = level.min(4);
            h.hp = h.max_hp();
        }
        h
    }

    /// New week: an event, fresh recruits, wagon stock and quests.
    pub fn refresh_week(&mut self) {
        self.event = if self.week <= 1 {
            TownEvent::Quiet
        } else {
            *self
                .rng
                .pick(&[
                    TownEvent::Quiet,
                    TownEvent::Quiet,
                    TownEvent::TavernDiscount,
                    TownEvent::ChapelClosed,
                    TownEvent::Patron,
                    TownEvent::ExtraRecruits,
                    TownEvent::WagonSale,
                    TownEvent::Festival,
                ])
                .expect("events")
        };
        match self.event {
            TownEvent::Patron => self.gold += 750,
            TownEvent::Festival => {
                for h in &mut self.roster {
                    h.stress = (h.stress - 15).max(0);
                }
            }
            _ => {}
        }
        let sc = self.level(Building::Stagecoach);
        let extra = if self.event == TownEvent::ExtraRecruits {
            2
        } else {
            0
        };
        self.stagecoach.clear();
        for _ in 0..3 + sc + extra {
            let class = *self.rng.pick(&HeroClass::ALL).expect("classes");
            // An upgraded coach brings blooded veterans now and then.
            let level = if sc >= 2 && self.rng.percent(35) {
                2
            } else if sc >= 1 && self.rng.percent(40) {
                1
            } else {
                0
            };
            let h = self.make_hero(class, level);
            self.stagecoach.push(h);
        }
        let wl = self.level(Building::Wagon);
        self.wagon.clear();
        for _ in 0..3 + wl {
            let t = roll_trinket(&mut self.rng, 1 + wl.min(2));
            self.wagon.push(t.id.to_owned());
        }
        self.relief_used.clear();
        self.make_offers();
    }

    fn make_offers(&mut self) {
        self.offers.clear();
        let top = self.roster.iter().map(|h| h.level).max().unwrap_or(0);
        let levels: Vec<u8> = match top {
            0..=1 => vec![1],
            2..=3 => vec![1, 2],
            _ => vec![2, 3],
        };
        for region in Region::ALL {
            for _ in 0..2 {
                let level = *self.rng.pick(&levels).expect("levels");
                let length = *self
                    .rng
                    .pick(&[Length::Short, Length::Short, Length::Medium, Length::Long])
                    .expect("lengths");
                let kind = *self
                    .rng
                    .pick(&[QuestKind::Explore, QuestKind::Cleanse, QuestKind::Gather])
                    .expect("kinds");
                let o = self.offer(region, length, kind, level);
                self.offers.push(o);
            }
            // A region's lair opens after two expeditions there.
            if self.region_runs.get(&region).copied().unwrap_or(0) >= 2 {
                let level = *levels.last().expect("levels");
                let o = self.offer(region, Length::Medium, QuestKind::Boss, level);
                self.offers.push(o);
            }
        }
    }

    fn offer(&mut self, region: Region, length: Length, kind: QuestKind, level: u8) -> QuestOffer {
        let l = i32::from(level);
        let len = match length {
            Length::Short => 1,
            Length::Medium => 2,
            Length::Long => 3,
        };
        let boss = i32::from(kind == QuestKind::Boss);
        QuestOffer {
            region,
            length,
            kind,
            level,
            reward_gold: (1000 + 900 * len) * l + 2500 * boss * l,
            reward_heirlooms: (3 + 3 * len) * l + 6 * boss,
            reward_trinket: boss == 1 || len == 3 || self.rng.percent(30),
        }
    }

    pub fn recruit(&mut self, idx: usize) -> bool {
        if idx >= self.stagecoach.len() || self.roster.len() >= self.roster_cap() {
            return false;
        }
        let h = self.stagecoach.remove(idx);
        self.roster.push(h);
        true
    }

    pub fn dismiss(&mut self, id: u32) {
        if let Some(pos) = self.roster.iter().position(|h| h.id == id) {
            let h = self.roster.remove(pos);
            // What they wore goes back to the stash.
            self.trinkets.extend(h.trinkets.into_iter().flatten());
        }
    }

    pub fn hero_mut(&mut self, id: u32) -> Option<&mut Hero> {
        self.roster.iter_mut().find(|h| h.id == id)
    }

    pub fn relief_cost(&self, r: Relief) -> i32 {
        let l = i32::from(self.level(r.building()));
        match r {
            Relief::Tavern if self.event == TownEvent::TavernDiscount => (300 - 50 * l) / 2,
            Relief::Tavern => 300 - 50 * l,
            Relief::Chapel => 350 - 50 * l,
        }
    }

    pub fn relief_amount(&self, r: Relief) -> (i32, i32) {
        let l = 8 * i32::from(self.level(r.building()));
        match r {
            Relief::Tavern => (30 + l, 60 + l),
            Relief::Chapel => (40 + l, 55 + l),
        }
    }

    /// Room for heroes this week.
    pub fn relief_slots(&self, r: Relief) -> u8 {
        let b = r.building();
        if r == Relief::Chapel && self.event == TownEvent::ChapelClosed {
            return 0;
        }
        (2 + self.level(b)).saturating_sub(self.relief_used.get(&b).copied().unwrap_or(0))
    }

    /// Send a hero to drink or pray. They sit out the next expedition.
    pub fn relieve(&mut self, id: u32, relief: Relief) -> Option<i32> {
        let cost = self.relief_cost(relief);
        if self.gold < cost || self.relief_slots(relief) == 0 {
            return None;
        }
        let (lo, hi) = self.relief_amount(relief);
        let amount = self.rng.range(lo, hi);
        let h = self.roster.iter_mut().find(|h| h.id == id)?;
        if h.resting || h.stress == 0 && h.affliction.is_none() {
            return None;
        }
        self.gold -= cost;
        let before = h.stress;
        h.stress = (h.stress - amount).max(0);
        h.affliction = None;
        h.resting = true;
        *self.relief_used.entry(relief.building()).or_insert(0) += 1;
        Some(before - h.stress)
    }

    pub fn treat_cost(&self) -> i32 {
        500 - 100 * i32::from(self.level(Building::Sanitarium))
    }

    /// The sanitarium cuts out a negative quirk.
    pub fn treat_quirk(&mut self, id: u32, quirk: &str) -> bool {
        let cost = self.treat_cost();
        if self.gold < cost {
            return false;
        }
        let Some(h) = self.roster.iter_mut().find(|h| h.id == id) else {
            return false;
        };
        let Some(pos) = h.quirks.iter().position(|q| q == quirk) else {
            return false;
        };
        if super::hero::quirk(quirk).is_none_or(|q| q.positive) || h.resting {
            return false;
        }
        h.quirks.remove(pos);
        h.resting = true;
        self.gold -= cost;
        true
    }

    pub fn max_gear_tier(&self) -> u8 {
        2 + self.level(Building::Blacksmith)
    }

    /// The smith: raise a weapon (`weapon == true`) or armour tier, for gold.
    pub fn upgrade(&mut self, id: u32, weapon: bool) -> bool {
        let cap = self.max_gear_tier();
        let Some(h) = self.roster.iter().find(|h| h.id == id) else {
            return false;
        };
        let tier = if weapon { h.weapon } else { h.armor };
        if tier >= cap || tier >= 5 {
            return false;
        }
        let gold = Hero::upgrade_cost(tier);
        if self.gold < gold {
            return false;
        }
        self.gold -= gold;
        let h = self
            .roster
            .iter_mut()
            .find(|h| h.id == id)
            .expect("found above");
        if weapon {
            h.weapon += 1;
        } else {
            h.armor += 1;
            h.hp = h.max_hp();
        }
        true
    }

    pub fn max_skill_rank(&self) -> u8 {
        2 + self.level(Building::Guild)
    }

    pub fn skill_cost(rank: u8) -> i32 {
        [0, 0, 500, 1250, 2500, 4500][usize::from(rank.min(5))]
    }

    /// The guild: train a skill to its next rank.
    pub fn train_skill(&mut self, id: u32, skill: &str) -> bool {
        let cap = self.max_skill_rank();
        let Some(h) = self.roster.iter().find(|h| h.id == id) else {
            return false;
        };
        let next = h.rank(skill) + 1;
        if next > cap || next > 5 || !h.class.def().skills.iter().any(|s| s.id == skill) {
            return false;
        }
        let cost = Self::skill_cost(next);
        if self.gold < cost {
            return false;
        }
        self.gold -= cost;
        self.hero_mut(id)
            .expect("found above")
            .set_rank(skill, next);
        true
    }

    pub const TALENT_RESET: i32 = 750;

    /// Forget every talent, for gold.
    pub fn reset_talents(&mut self, id: u32) -> bool {
        if self.gold < Self::TALENT_RESET {
            return false;
        }
        let Some(h) = self.hero_mut(id) else {
            return false;
        };
        if h.talents.is_empty() {
            return false;
        }
        h.talents.clear();
        self.gold -= Self::TALENT_RESET;
        true
    }

    /// Spend heirlooms to raise a building.
    pub fn upgrade_building(&mut self, b: Building) -> bool {
        let lvl = self.level(b);
        if lvl >= MAX_BUILDING {
            return false;
        }
        let cost = Building::upgrade_cost(lvl);
        let (h1, h2) = b.heirlooms();
        if self.heirloom(h1) < cost || self.heirloom(h2) < cost {
            return false;
        }
        *self.heirlooms.entry(h1).or_insert(0) -= cost;
        *self.heirlooms.entry(h2).or_insert(0) -= cost;
        self.buildings.0.insert(b, lvl + 1);
        // New wagon stock and recruits arrive at once.
        if b == Building::Wagon {
            let t = roll_trinket(&mut self.rng, 1 + (lvl + 1).min(2));
            self.wagon.push(t.id.to_owned());
        }
        if b == Building::Stagecoach {
            let class = *self.rng.pick(&HeroClass::ALL).expect("classes");
            let h = self.make_hero(class, 0);
            self.stagecoach.push(h);
        }
        true
    }

    pub fn wagon_price(&self, id: &str) -> i32 {
        let sale = if self.event == TownEvent::WagonSale {
            3
        } else {
            0
        };
        trinket(id).map_or(0, |t| {
            t.price() * (10 - i32::from(self.level(Building::Wagon)) - sale) / 10
        })
    }

    pub fn buy_trinket(&mut self, idx: usize) -> bool {
        let Some(id) = self.wagon.get(idx).cloned() else {
            return false;
        };
        let price = self.wagon_price(&id);
        if self.gold < price {
            return false;
        }
        self.gold -= price;
        self.wagon.remove(idx);
        self.trinkets.push(id);
        true
    }

    pub fn sell_trinket(&mut self, idx: usize) -> bool {
        if idx >= self.trinkets.len() {
            return false;
        }
        let id = self.trinkets.remove(idx);
        self.gold += trinket(&id).map_or(0, |t| t.price() / 4);
        true
    }

    /// Put a stash trinket on a hero, sending whatever they wore back to the stash.
    pub fn equip_trinket(
        &mut self,
        hero: u32,
        slot: usize,
        stash_idx: usize,
    ) -> Result<(), &'static str> {
        let id = self
            .trinkets
            .get(stash_idx)
            .cloned()
            .ok_or("no such trinket")?;
        let h = self
            .roster
            .iter_mut()
            .find(|h| h.id == hero)
            .ok_or("no such hero")?;
        let old = h.equip(slot, Some(id))?;
        self.trinkets.remove(stash_idx);
        if let Some(o) = old {
            self.trinkets.push(o);
        }
        Ok(())
    }

    pub fn unequip_trinket(&mut self, hero: u32, slot: usize) {
        if let Some(h) = self.roster.iter_mut().find(|h| h.id == hero) {
            if let Ok(Some(old)) = h.equip(slot, None) {
                self.trinkets.push(old);
            }
        }
    }

    /// Buy provisions for the road; returns whether the purse allowed it.
    pub fn buy(&mut self, pack: &mut HashMap<Item, i32>, item: Item, n: i32) -> bool {
        let cost = item.price() * n;
        if n > 0 && self.gold < cost {
            return false;
        }
        let have = pack.get(&item).copied().unwrap_or(0);
        if n < 0 && have < -n {
            return false;
        }
        self.gold -= cost;
        pack.insert(item, have + n);
        true
    }

    /// Why a hero will not go on this expedition, if they won't.
    pub fn refusal(&self, hero: &Hero, offer: &QuestOffer) -> Option<&'static str> {
        if hero.resting {
            Some("resting this week")
        } else if hero.level > max_resolve_for(offer.level) {
            Some("too experienced for this level")
        } else {
            None
        }
    }

    /// Set out with these heroes (front to back) on an offer.
    pub fn embark(
        &mut self,
        party_ids: &[u32],
        pack: HashMap<Item, i32>,
        offer: &QuestOffer,
    ) -> Option<Expedition> {
        if party_ids.is_empty() || party_ids.len() > 4 {
            return None;
        }
        let mut party = Vec::new();
        for id in party_ids {
            let h = self.roster.iter().find(|h| h.id == *id)?;
            if self.refusal(h, offer).is_some() {
                return None;
            }
            party.push(h.clone());
        }
        for id in party_ids {
            self.roster.retain(|h| h.id != *id);
        }
        let mut rng = Rng::new(self.rng.next_u64());
        let quest = Quest::new(offer.region, offer.length, offer.kind, offer.level);
        let dungeon = Dungeon::generate(&mut rng, quest);
        self.offers.retain(|o| o != offer);
        Some(Expedition::new(party, dungeon, pack, rng))
    }

    /// Bring an expedition home: pay out, bury the dead, teach the living.
    pub fn debrief(
        &mut self,
        mut e: Expedition,
        success: bool,
        offer: Option<&QuestOffer>,
    ) -> Debrief {
        let mut d = Debrief {
            success,
            quest: e.dungeon.quest.title(),
            gold: e.gold,
            ..Default::default()
        };
        self.gold += e.gold;
        for (k, n) in &e.heirlooms {
            *self.heirlooms.entry(*k).or_insert(0) += n;
            d.heirlooms.push((*k, *n));
        }
        self.trinkets.append(&mut e.trinkets.clone());
        d.trinkets = std::mem::take(&mut e.trinkets);
        let q = e.dungeon.quest;
        if success {
            if let Some(o) = offer {
                self.gold += o.reward_gold;
                d.gold += o.reward_gold;
                // Quest heirlooms come as the region's favoured kinds.
                let kinds = match q.region {
                    Region::Crypts => [Heirloom::Bust, Heirloom::Portrait],
                    Region::Warrens => [Heirloom::Deed, Heirloom::Crest],
                    Region::Grotto => [Heirloom::Portrait, Heirloom::Deed],
                };
                for k in kinds {
                    let n = o.reward_heirlooms / 2;
                    *self.heirlooms.entry(k).or_insert(0) += n;
                    d.heirlooms.push((k, n));
                }
                if o.reward_trinket {
                    let min = if o.kind == QuestKind::Boss {
                        Rarity::Rare
                    } else {
                        Rarity::Uncommon
                    };
                    let t = super::items::roll_trinket_at_least(&mut self.rng, min);
                    self.trinkets.push(t.id.to_owned());
                    d.trinkets.push(t.id.to_owned());
                }
            }
            if q.kind == QuestKind::Boss && !self.bosses.contains(&q.region) {
                self.bosses.push(q.region);
                self.boss_slain = true;
            }
        }
        *self.region_runs.entry(q.region).or_insert(0) += 1;
        let xp = if success { 2 + i32::from(q.level) } else { 1 }
            + match q.length {
                Length::Short => 0,
                Length::Medium => 1,
                Length::Long => 2,
            }
            + i32::from(q.kind == QuestKind::Boss) * 2;
        for mut h in e.party.drain(..) {
            h.expeditions += 1;
            if h.grant_xp(xp) {
                let talent = if h.open_talent_tiers().is_empty() {
                    ""
                } else {
                    " A talent awaits in the guild hall."
                };
                d.leveled.push(format!(
                    "{} reached resolve level {}.{talent}",
                    h.name, h.level
                ));
            }
            h.hp = h.max_hp();
            h.virtue = None;
            h.camp_buffs.clear();
            if self.rng.percent(35) {
                let pool: Vec<_> = QUIRKS
                    .iter()
                    .filter(|q| !h.quirks.iter().any(|x| x == q.id))
                    .collect();
                if let Some(qk) = self.rng.pick(&pool) {
                    h.quirks.push(qk.id.to_owned());
                    d.quirks.push(format!(
                        "{} gained {} ({})",
                        h.name,
                        qk.name,
                        if qk.positive { "+" } else { "-" }
                    ));
                }
            }
            if h.quirks.len() > 5 {
                h.quirks.remove(0);
            }
            self.roster.push(h);
        }
        for h in e.fallen.drain(..) {
            d.dead.push(format!("{} the {}", h.name, h.class_name()));
            // What the dead wore is lost with them.
            self.graveyard.push(Grave {
                hero: h,
                week: self.week,
                cause: format!("Fell in {}", q.region.name()),
            });
        }
        self.runs += 1;
        self.end_week();
        d
    }

    pub fn end_week(&mut self) {
        self.week += 1;
        for h in &mut self.roster {
            h.resting = false;
        }
        self.refresh_week();
    }

    pub fn heirloom(&self, h: Heirloom) -> i32 {
        self.heirlooms.get(&h).copied().unwrap_or(0)
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(s: &str) -> anyhow::Result<Campaign> {
        let mut c: Campaign = serde_json::from_str(s)?;
        if c.offers.is_empty() {
            c.make_offers();
        }
        Ok(c)
    }
}

/// The provisions the shop suggests for an expedition's length.
pub fn default_pack(length: Length) -> HashMap<Item, i32> {
    match length {
        Length::Short => HashMap::from([
            (Item::Food, 8),
            (Item::Torch, 6),
            (Item::Bandage, 2),
            (Item::Shovel, 1),
        ]),
        Length::Medium => HashMap::from([
            (Item::Food, 14),
            (Item::Torch, 10),
            (Item::Bandage, 3),
            (Item::Shovel, 2),
            (Item::Key, 1),
        ]),
        Length::Long => HashMap::from([
            (Item::Food, 20),
            (Item::Torch, 14),
            (Item::Bandage, 4),
            (Item::Shovel, 2),
            (Item::Key, 2),
            (Item::Herbs, 1),
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_campaign_has_heroes_recruits_offers() {
        let c = Campaign::new(Some(1));
        assert_eq!(c.roster.len(), 4);
        assert_eq!(c.stagecoach.len(), 3);
        assert_eq!(c.offers.len(), 6);
        assert_eq!(c.wagon.len(), 3);
    }

    #[test]
    fn embark_and_return_round_trips_the_party() {
        let mut c = Campaign::new(Some(2));
        let ids: Vec<u32> = c.roster.iter().map(|h| h.id).collect();
        let offer = c.offers[0].clone();
        let e = c.embark(&ids, default_pack(offer.length), &offer).unwrap();
        assert!(c.roster.is_empty());
        let gold = c.gold;
        let d = c.debrief(e, true, Some(&offer));
        assert!(d.success);
        assert_eq!(c.roster.len(), 4);
        assert!(c.gold >= gold + offer.reward_gold);
        assert_eq!(c.week, 2);
    }

    #[test]
    fn relief_costs_gold_and_rests_the_hero() {
        let mut c = Campaign::new(Some(3));
        let id = c.roster[0].id;
        c.roster[0].stress = 80;
        let gold = c.gold;
        assert!(c.relieve(id, Relief::Tavern).is_some());
        assert!(c.gold < gold);
        assert!(c.roster[0].resting);
        assert!(c.relieve(id, Relief::Chapel).is_none(), "once a week");
    }

    #[test]
    fn buildings_upgrade_with_heirlooms_and_lift_caps() {
        let mut c = Campaign::new(Some(4));
        assert_eq!(c.max_skill_rank(), 2);
        assert!(!c.upgrade_building(Building::Guild));
        c.heirlooms.insert(Heirloom::Portrait, 10);
        c.heirlooms.insert(Heirloom::Crest, 10);
        assert!(c.upgrade_building(Building::Guild));
        assert_eq!(c.max_skill_rank(), 3);
        let id = c.roster[0].id;
        let s = c.roster[0].skills[0].clone();
        assert!(c.train_skill(id, &s));
        assert!(c.train_skill(id, &s));
        assert!(!c.train_skill(id, &s), "capped by the guild");
        assert_eq!(c.roster[0].rank(&s), 3);
    }

    #[test]
    fn trinkets_move_between_stash_and_hero() {
        let mut c = Campaign::new(Some(5));
        c.trinkets.push("tin_locket".into());
        let id = c.roster[0].id;
        assert!(c.equip_trinket(id, 0, 0).is_ok());
        assert!(c.trinkets.is_empty());
        c.unequip_trinket(id, 0);
        assert_eq!(c.trinkets, vec!["tin_locket".to_owned()]);
        let gold = c.gold;
        assert!(c.buy_trinket(0) || c.gold < gold);
    }

    #[test]
    fn saves_round_trip() {
        let c = Campaign::new(Some(6));
        let json = c.to_json().unwrap();
        let back = Campaign::from_json(&json).unwrap();
        assert_eq!(back.roster.len(), c.roster.len());
        assert_eq!(back.offers.len(), c.offers.len());
    }

    #[test]
    fn town_events_bend_prices() {
        let mut c = Campaign::new(Some(8));
        let full = c.relief_cost(Relief::Tavern);
        c.event = TownEvent::TavernDiscount;
        assert_eq!(c.relief_cost(Relief::Tavern), full / 2);
        c.event = TownEvent::ChapelClosed;
        assert_eq!(c.relief_slots(Relief::Chapel), 0);
    }
}
