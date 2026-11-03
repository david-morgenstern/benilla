//! The hamlet between expeditions: the roster and the stagecoach, the tavern and chapel that
//! take stress off for coin, the smith, the graveyard, and the week that turns between runs.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::data::HeroClass;
use super::dungeon::{Dungeon, Expedition, Heirloom, Item, Quest};
use super::hero::{Hero, HERO_NAMES, QUIRKS};
use super::rng::Rng;

pub const ROSTER_MAX: usize = 12;
pub const STAGECOACH_SIZE: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Relief {
    /// The tavern's ale and dice.
    Tavern,
    /// The chapel's quiet prayer.
    Chapel,
}

impl Relief {
    pub fn name(self) -> &'static str {
        match self {
            Relief::Tavern => "The Drowned Lantern (Tavern)",
            Relief::Chapel => "Chapel of the Quiet Light",
        }
    }

    pub fn cost(self) -> i32 {
        match self {
            Relief::Tavern => 300,
            Relief::Chapel => 350,
        }
    }

    pub fn amount(self) -> (i32, i32) {
        match self {
            Relief::Tavern => (30, 70),
            Relief::Chapel => (45, 60),
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
    pub gold: i32,
    pub heirlooms: Vec<(Heirloom, i32)>,
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
    /// Expeditions completed, which deepens the crypts.
    pub runs: u32,
    #[serde(default)]
    pub boss_slain: bool,
}

impl Campaign {
    pub fn new(seed: Option<u64>) -> Campaign {
        let rng = seed.map_or_else(Rng::from_time, Rng::new);
        let mut c = Campaign {
            week: 1,
            gold: 1500,
            heirlooms: HashMap::new(),
            roster: Vec::new(),
            graveyard: Vec::new(),
            stagecoach: Vec::new(),
            next_id: 1,
            rng,
            runs: 0,
            boss_slain: false,
        };
        for class in [
            HeroClass::Warrior,
            HeroClass::Rogue,
            HeroClass::Hunter,
            HeroClass::Priest,
        ] {
            let h = c.make_hero(class);
            c.roster.push(h);
        }
        c.refresh_stagecoach();
        c
    }

    fn make_hero(&mut self, class: HeroClass) -> Hero {
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
        Hero::new(id, class, name, &mut self.rng)
    }

    pub fn refresh_stagecoach(&mut self) {
        self.stagecoach.clear();
        for _ in 0..STAGECOACH_SIZE {
            let class = *self.rng.pick(&HeroClass::ALL).expect("classes");
            let h = self.make_hero(class);
            self.stagecoach.push(h);
        }
    }

    pub fn recruit(&mut self, idx: usize) -> bool {
        if idx >= self.stagecoach.len() || self.roster.len() >= ROSTER_MAX {
            return false;
        }
        let h = self.stagecoach.remove(idx);
        self.roster.push(h);
        true
    }

    pub fn dismiss(&mut self, id: u32) {
        self.roster.retain(|h| h.id != id);
    }

    pub fn hero_mut(&mut self, id: u32) -> Option<&mut Hero> {
        self.roster.iter_mut().find(|h| h.id == id)
    }

    /// Send a hero to drink or pray. They sit out the next expedition.
    pub fn relieve(&mut self, id: u32, relief: Relief) -> Option<i32> {
        if self.gold < relief.cost() {
            return None;
        }
        let (lo, hi) = relief.amount();
        let amount = self.rng.range(lo, hi);
        let h = self.roster.iter_mut().find(|h| h.id == id)?;
        if h.resting || h.stress == 0 && h.affliction.is_none() {
            return None;
        }
        self.gold -= relief.cost();
        let before = h.stress;
        h.stress = (h.stress - amount).max(0);
        // A night away from the dark breaks an affliction's hold.
        h.affliction = None;
        h.resting = true;
        Some(before - h.stress)
    }

    /// The sanitarium cuts out a negative quirk.
    pub fn treat_quirk(&mut self, id: u32, quirk: &str) -> bool {
        const COST: i32 = 500;
        if self.gold < COST {
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
        self.gold -= COST;
        true
    }

    /// The smith: raise a weapon (`weapon == true`) or armour tier, for gold and heirlooms.
    pub fn upgrade(&mut self, id: u32, weapon: bool) -> bool {
        let Some(h) = self.roster.iter().find(|h| h.id == id) else {
            return false;
        };
        let tier = if weapon { h.weapon } else { h.armor };
        if tier >= 5 || tier > h.level + 1 {
            return false;
        }
        let gold = Hero::upgrade_cost(tier);
        let crests = i32::from(tier);
        let have = self.heirlooms.get(&Heirloom::Crest).copied().unwrap_or(0);
        if self.gold < gold || have < crests - 1 {
            return false;
        }
        self.gold -= gold;
        *self.heirlooms.entry(Heirloom::Crest).or_insert(0) -= (crests - 1).max(0);
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

    /// Set out with these heroes (front to back) and this pack.
    pub fn embark(
        &mut self,
        party_ids: &[u32],
        pack: HashMap<Item, i32>,
        quest: Quest,
    ) -> Option<Expedition> {
        if party_ids.is_empty() || party_ids.len() > 4 {
            return None;
        }
        let mut party = Vec::new();
        for id in party_ids {
            let pos = self.roster.iter().position(|h| h.id == *id && !h.resting)?;
            party.push(self.roster[pos].clone());
        }
        for id in party_ids {
            self.roster.retain(|h| h.id != *id);
        }
        let rooms = match quest {
            Quest::Boss => 11,
            Quest::Explore(_) => 9,
        };
        let mut rng = Rng::new(self.rng.next_u64());
        let dungeon = Dungeon::generate(&mut rng, rooms, quest);
        Some(Expedition::new(party, dungeon, pack, rng))
    }

    /// Bring an expedition home: pay out, bury the dead, teach the living.
    pub fn debrief(&mut self, mut e: Expedition, success: bool) -> Debrief {
        let mut d = Debrief {
            success,
            gold: e.gold,
            ..Default::default()
        };
        self.gold += e.gold;
        for (k, n) in &e.heirlooms {
            *self.heirlooms.entry(*k).or_insert(0) += n;
            d.heirlooms.push((*k, *n));
        }
        if success && e.dungeon.quest == Quest::Boss {
            self.boss_slain = true;
        }
        let xp = if success { 3 } else { 1 } + i32::from(e.dungeon.quest == Quest::Boss) * 2;
        for mut h in e.party.drain(..) {
            h.expeditions += 1;
            if h.grant_xp(xp) {
                d.leveled
                    .push(format!("{} reached resolve level {}", h.name, h.level));
            }
            // Wounds mend at home; the mind is another matter.
            h.hp = h.max_hp();
            h.virtue = None;
            if self.rng.percent(35) {
                let pool: Vec<_> = QUIRKS
                    .iter()
                    .filter(|q| !h.quirks.iter().any(|x| x == q.id))
                    .collect();
                if let Some(q) = self.rng.pick(&pool) {
                    h.quirks.push(q.id.to_owned());
                    d.quirks.push(format!(
                        "{} gained {} ({})",
                        h.name,
                        q.name,
                        if q.positive { "+" } else { "-" }
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
            self.graveyard.push(Grave {
                hero: h,
                week: self.week,
                cause: "Fell in the Sunken Crypts".into(),
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
        self.refresh_stagecoach();
    }

    pub fn heirloom(&self, h: Heirloom) -> i32 {
        self.heirlooms.get(&h).copied().unwrap_or(0)
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(s: &str) -> anyhow::Result<Campaign> {
        Ok(serde_json::from_str(s)?)
    }
}

/// The provisions the shop suggests for a short crypt run.
pub fn default_pack() -> HashMap<Item, i32> {
    HashMap::from([
        (Item::Food, 8),
        (Item::Torch, 6),
        (Item::Bandage, 2),
        (Item::Shovel, 1),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_campaign_has_four_heroes_and_recruits() {
        let c = Campaign::new(Some(1));
        assert_eq!(c.roster.len(), 4);
        assert_eq!(c.stagecoach.len(), STAGECOACH_SIZE);
    }

    #[test]
    fn embark_and_return_round_trips_the_party() {
        let mut c = Campaign::new(Some(2));
        let ids: Vec<u32> = c.roster.iter().map(|h| h.id).collect();
        let e = c.embark(&ids, default_pack(), Quest::Explore(80)).unwrap();
        assert!(c.roster.is_empty());
        let d = c.debrief(e, true);
        assert!(d.success);
        assert_eq!(c.roster.len(), 4);
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
    fn saves_round_trip() {
        let c = Campaign::new(Some(4));
        let json = c.to_json().unwrap();
        let back = Campaign::from_json(&json).unwrap();
        assert_eq!(back.roster.len(), c.roster.len());
    }
}
