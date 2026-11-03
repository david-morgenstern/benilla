//! The battle: two lines of four ranks, speed-rolled turns, skills that can only be used from
//! certain ranks against certain ranks, and the stress that comes with all of it.
//!
//! The battle is a state machine the presentation drives one turn at a time: [`Battle::step`]
//! runs until a hero needs orders or one actor has acted, and every consequence lands in
//! [`Battle::events`] for the stage to animate before it steps again.

use super::data::{
    has_rank, Effect, MonsterDef, SkillDef, SkillKind, Stat, Tag, Target, DRAIN_SKILL,
    LIFE_TAP_SKILL,
};
use super::hero::{apply_stress, Hero, StressOutcome};
use super::rng::Rng;

pub type UnitId = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Heroes,
    Monsters,
}

impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::Heroes => Side::Monsters,
            Side::Monsters => Side::Heroes,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StatusKind {
    Bleed(i32),
    Blight(i32),
    Stun,
    Mark,
    /// Hits on this unit land on the guardian.
    GuardedBy(UnitId),
    Riposte,
    Taunt,
    Mod(Stat, i32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Status {
    pub kind: StatusKind,
    /// Rounds left; counts down at the start of the bearer's turn.
    pub turns: u8,
}

#[derive(Clone, Debug)]
pub enum UnitKind {
    Hero(Box<Hero>),
    Monster(&'static MonsterDef),
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub id: UnitId,
    pub side: Side,
    pub kind: UnitKind,
    pub hp: i32,
    pub max_hp: i32,
    pub statuses: Vec<Status>,
    pub alive: bool,
    pub deaths_door: bool,
    /// A fallen monster's remains, still holding its rank.
    pub corpse: bool,
    pub size: u8,
    pub uses: Vec<(&'static str, u8)>,
}

impl Unit {
    pub fn name(&self) -> &str {
        match &self.kind {
            UnitKind::Hero(h) => &h.name,
            UnitKind::Monster(m) => m.name,
        }
    }

    pub fn hero(&self) -> Option<&Hero> {
        match &self.kind {
            UnitKind::Hero(h) => Some(h),
            UnitKind::Monster(_) => None,
        }
    }

    pub fn monster(&self) -> Option<&'static MonsterDef> {
        match self.kind {
            UnitKind::Monster(m) => Some(m),
            UnitKind::Hero(_) => None,
        }
    }

    pub fn is_boss(&self) -> bool {
        self.monster().is_some_and(|m| m.boss)
    }

    /// Alive and not a corpse: can act and be the subject of most things.
    pub fn standing(&self) -> bool {
        self.alive && !self.corpse
    }

    pub fn has(&self, pred: impl Fn(&StatusKind) -> bool) -> bool {
        self.statuses.iter().any(|s| pred(&s.kind))
    }

    pub fn stunned(&self) -> bool {
        self.has(|k| matches!(k, StatusKind::Stun))
    }

    pub fn marked(&self) -> bool {
        self.has(|k| matches!(k, StatusKind::Mark))
    }

    pub fn skills(&self) -> Vec<&'static SkillDef> {
        match &self.kind {
            UnitKind::Hero(h) => h.equipped_skills(),
            UnitKind::Monster(m) => m.skills.iter().collect(),
        }
    }

    fn uses_of(&self, id: &str) -> u8 {
        self.uses
            .iter()
            .find(|(s, _)| *s == id)
            .map_or(0, |(_, n)| *n)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Victory,
    Defeat,
    Retreated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DotKind {
    Bleed,
    Blight,
}

#[derive(Clone, Debug)]
pub enum Event {
    RoundStart(u32),
    TurnStart(UnitId),
    Skill {
        actor: UnitId,
        skill: &'static SkillDef,
        targets: Vec<UnitId>,
    },
    Miss(UnitId),
    Damage {
        target: UnitId,
        amount: i32,
        crit: bool,
    },
    Heal {
        target: UnitId,
        amount: i32,
        crit: bool,
    },
    Status {
        target: UnitId,
        label: String,
    },
    Resisted {
        target: UnitId,
        label: &'static str,
    },
    Tick {
        target: UnitId,
        amount: i32,
        kind: DotKind,
    },
    Stress {
        target: UnitId,
        amount: i32,
    },
    DeathsDoor(UnitId),
    DeathBlowResisted(UnitId),
    Died(UnitId),
    Corpse(UnitId),
    CorpseCleared(UnitId),
    /// The formation changed; the stage re-lays both lines.
    Moved,
    Afflicted(UnitId, super::hero::Affliction),
    Virtuous(UnitId, super::hero::Virtue),
    HeartAttack(UnitId),
    Stunned(UnitId),
    Bark(UnitId, String),
    Pass(UnitId),
    Retreat(bool),
    Over(Outcome),
}

/// What the stage should do after a [`Battle::step`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// A hero awaits orders.
    HeroTurn(UnitId),
    /// Someone acted (or lost the turn); play the events and step again.
    Acted,
    Over(Outcome),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Skill {
        skill: usize,
        target: UnitId,
    },
    /// Swap places with the ally at this rank.
    Move {
        to_rank: usize,
    },
    Pass,
    Retreat,
}

/// Light's grip on a battle: the brighter the torch, the better the heroes fare.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LightMods {
    pub hero_acc: i32,
    pub hero_crit: i32,
    pub monster_dmg: i32,
    pub monster_crit: i32,
    pub stress: i32,
}

pub fn light_mods(light: i32) -> LightMods {
    match light {
        76.. => LightMods {
            hero_acc: 5,
            hero_crit: 2,
            monster_dmg: 0,
            monster_crit: 0,
            stress: -10,
        },
        51..=75 => LightMods {
            hero_acc: 0,
            hero_crit: 0,
            monster_dmg: 0,
            monster_crit: 1,
            stress: 0,
        },
        26..=50 => LightMods {
            hero_acc: -2,
            hero_crit: 0,
            monster_dmg: 10,
            monster_crit: 2,
            stress: 15,
        },
        1..=25 => LightMods {
            hero_acc: -5,
            hero_crit: 0,
            monster_dmg: 15,
            monster_crit: 4,
            stress: 25,
        },
        _ => LightMods {
            hero_acc: -7,
            hero_crit: 0,
            monster_dmg: 25,
            monster_crit: 7,
            stress: 40,
        },
    }
}

pub struct Battle {
    pub units: Vec<Unit>,
    pub heroes: Vec<UnitId>,
    pub monsters: Vec<UnitId>,
    pub round: u32,
    order: Vec<UnitId>,
    pub current: Option<UnitId>,
    pub events: Vec<Event>,
    pub light: i32,
    pub outcome: Option<Outcome>,
    pub rng: Rng,
    /// Monsters slain this battle, for the roster's tally.
    pub kills: Vec<(UnitId, UnitId)>,
}

impl Battle {
    pub fn new(heroes: Vec<Hero>, pack: &[&'static MonsterDef], light: i32, rng: Rng) -> Battle {
        let mut units = Vec::new();
        let mut hero_ids = Vec::new();
        let mut monster_ids = Vec::new();
        for h in heroes {
            let id = units.len();
            let max_hp = h.max_hp();
            units.push(Unit {
                id,
                side: Side::Heroes,
                hp: h.hp.clamp(0, max_hp),
                max_hp,
                deaths_door: h.hp <= 0,
                kind: UnitKind::Hero(Box::new(h)),
                statuses: Vec::new(),
                alive: true,
                corpse: false,
                size: 1,
                uses: Vec::new(),
            });
            hero_ids.push(id);
        }
        for m in pack {
            let id = units.len();
            units.push(Unit {
                id,
                side: Side::Monsters,
                kind: UnitKind::Monster(m),
                hp: m.base.max_hp,
                max_hp: m.base.max_hp,
                statuses: Vec::new(),
                alive: true,
                deaths_door: false,
                corpse: false,
                size: m.size,
                uses: Vec::new(),
            });
            monster_ids.push(id);
        }
        Battle {
            units,
            heroes: hero_ids,
            monsters: monster_ids,
            round: 0,
            order: Vec::new(),
            current: None,
            events: Vec::new(),
            light,
            outcome: None,
            rng,
            kills: Vec::new(),
        }
    }

    pub fn line(&self, side: Side) -> &[UnitId] {
        match side {
            Side::Heroes => &self.heroes,
            Side::Monsters => &self.monsters,
        }
    }

    fn line_mut(&mut self, side: Side) -> &mut Vec<UnitId> {
        match side {
            Side::Heroes => &mut self.heroes,
            Side::Monsters => &mut self.monsters,
        }
    }

    /// The 1-based ranks a unit covers (two for a big one), or empty if it holds none.
    pub fn ranks_of(&self, id: UnitId) -> Vec<usize> {
        let side = self.units[id].side;
        let mut rank = 1;
        for &u in self.line(side) {
            let size = usize::from(self.units[u].size);
            if u == id {
                return (rank..rank + size).collect();
            }
            rank += size;
        }
        Vec::new()
    }

    pub fn rank_of(&self, id: UnitId) -> usize {
        self.ranks_of(id).first().copied().unwrap_or(0)
    }

    fn in_ranks(&self, id: UnitId, mask: u8) -> bool {
        self.ranks_of(id).into_iter().any(|r| has_rank(mask, r))
    }

    /// An effective stat: base, gear, quirks, statuses and the light.
    pub fn stat(&self, id: UnitId, stat: Stat) -> i32 {
        let u = &self.units[id];
        let (base, mods) = match &u.kind {
            UnitKind::Hero(h) => (h.stats(), h.mod_total(stat)),
            UnitKind::Monster(m) => (m.base, 0),
        };
        let statuses: i32 = u
            .statuses
            .iter()
            .filter_map(|s| match s.kind {
                StatusKind::Mod(st, v) if st == stat => Some(v),
                _ => None,
            })
            .sum();
        let light = light_mods(self.light);
        let is_hero = u.side == Side::Heroes;
        let base_v = match stat {
            Stat::Acc => base.acc + if is_hero { light.hero_acc } else { 0 },
            Stat::Crit => {
                base.crit
                    + if is_hero {
                        light.hero_crit
                    } else {
                        light.monster_crit
                    }
            }
            Stat::Dmg => {
                if is_hero {
                    0
                } else {
                    light.monster_dmg
                }
            }
            Stat::Dodge => base.dodge,
            Stat::Prot => base.prot,
            Stat::Speed => base.speed,
            Stat::StunRes => base.res.stun,
            Stat::BleedRes => base.res.bleed,
            Stat::BlightRes => base.res.blight,
            Stat::DebuffRes => base.res.debuff,
            Stat::MoveRes => base.res.moves,
            Stat::StressTaken => light.stress,
            Stat::HealRecv => 0,
        };
        // Death's Door saps a hero's hand.
        let door = if u.deaths_door {
            match stat {
                Stat::Acc => -15,
                Stat::Dmg => -20,
                Stat::Speed => -2,
                _ => 0,
            }
        } else {
            0
        };
        base_v + mods + statuses + door
    }

    fn dmg_range(&self, id: UnitId) -> (i32, i32) {
        match &self.units[id].kind {
            UnitKind::Hero(h) => h.stats().dmg,
            UnitKind::Monster(m) => m.base.dmg,
        }
    }

    pub fn standing(&self, side: Side) -> Vec<UnitId> {
        self.line(side)
            .iter()
            .copied()
            .filter(|&u| self.units[u].standing())
            .collect()
    }

    /// Whether a unit could use this skill from where it stands now.
    pub fn can_use(&self, id: UnitId, skill: &SkillDef) -> bool {
        let u = &self.units[id];
        if skill.limit > 0 && u.uses_of(skill.id) >= skill.limit {
            return false;
        }
        self.in_ranks(id, skill.launch) && !self.targets_for(id, skill).is_empty()
    }

    /// Who a skill may be aimed at. For a skill that hits a whole rank set, aiming at any one of
    /// them strikes them all.
    pub fn targets_for(&self, id: UnitId, skill: &SkillDef) -> Vec<UnitId> {
        let side = self.units[id].side;
        match skill.target {
            Target::Enemy(mask) | Target::EnemyAll(mask) => {
                let foes = self.line(side.other());
                let has_living = foes.iter().any(|&f| self.units[f].standing());
                foes.iter()
                    .copied()
                    .filter(|&f| self.units[f].alive && self.in_ranks(f, mask))
                    // Corpses can be struck, but only while something still stands.
                    .filter(|&f| !self.units[f].corpse || has_living)
                    .collect()
            }
            Target::Ally(mask) => self
                .line(side)
                .iter()
                .copied()
                .filter(|&a| self.units[a].standing() && self.in_ranks(a, mask))
                .collect(),
            Target::AllyOther(mask) => self
                .line(side)
                .iter()
                .copied()
                .filter(|&a| a != id && self.units[a].standing() && self.in_ranks(a, mask))
                .collect(),
            Target::AllyAll | Target::SelfOnly => vec![id],
        }
    }

    /// Everyone a skill aimed at `aim` lands on.
    pub fn affected(&self, id: UnitId, skill: &SkillDef, aim: UnitId) -> Vec<UnitId> {
        match skill.target {
            Target::EnemyAll(_) => self.targets_for(id, skill),
            Target::AllyAll => self.standing(self.units[id].side),
            Target::SelfOnly => vec![id],
            _ => vec![aim],
        }
    }

    pub fn hit_chance(&self, actor: UnitId, skill: &SkillDef, target: UnitId) -> i32 {
        if !skill.target.is_enemy() {
            return 100;
        }
        if self.units[target].corpse || self.units[target].stunned() {
            return 95.min(skill.acc + self.stat(actor, Stat::Acc));
        }
        (skill.acc + self.stat(actor, Stat::Acc) - self.stat(target, Stat::Dodge)).clamp(5, 95)
    }

    pub fn crit_chance(&self, actor: UnitId, skill: &SkillDef) -> i32 {
        (self.stat(actor, Stat::Crit) + skill.crit_mod).clamp(0, 95)
    }

    /// The damage a skill would roll before protection, as a range.
    pub fn damage_preview(&self, actor: UnitId, skill: &SkillDef, target: UnitId) -> (i32, i32) {
        if skill.dmg_mod <= -100 || skill.heal.is_some() {
            return (0, 0);
        }
        let pct = self.damage_pct(actor, skill, target);
        let (lo, hi) = self.dmg_range(actor);
        let prot = self.stat(target, Stat::Prot).clamp(0, 80);
        let f = |v: i32| ((v * pct / 100) * (100 - prot) / 100).max(1);
        (f(lo), f(hi))
    }

    fn damage_pct(&self, actor: UnitId, skill: &SkillDef, target: UnitId) -> i32 {
        let mut pct = 100 + skill.dmg_mod + self.stat(actor, Stat::Dmg);
        if let (Some((tag, bonus)), Some(m)) = (skill.bonus_vs, self.units[target].monster()) {
            if m.has_tag(tag) {
                pct += bonus;
            }
        }
        if self.units[target].marked() {
            pct += skill.bonus_marked;
        }
        pct.max(10)
    }

    // ---------------------------------------------------------------- the turn loop

    fn roll_initiative(&mut self) {
        self.round += 1;
        self.events.push(Event::RoundStart(self.round));
        let mut order: Vec<(i32, UnitId)> = self
            .units
            .iter()
            .filter(|u| u.standing())
            .map(|u| u.id)
            .collect::<Vec<_>>()
            .into_iter()
            .map(|id| (self.stat(id, Stat::Speed) + self.rng.range(1, 8), id))
            .collect();
        order.sort_by_key(|a| a.0);
        // Popped from the back: fastest first.
        self.order = order.into_iter().map(|(_, id)| id).collect();
        // The dark gnaws at the party round by round.
        if self.light <= 25 {
            for h in self.standing(Side::Heroes) {
                self.stress(h, if self.light == 0 { 3 } else { 1 });
            }
        }
    }

    /// Run until a hero needs orders, one actor has acted, or the battle ends.
    pub fn step(&mut self) -> Step {
        if let Some(o) = self.outcome {
            return Step::Over(o);
        }
        loop {
            if let Some(o) = self.check_over() {
                return Step::Over(o);
            }
            let Some(id) = self.order.pop() else {
                self.roll_initiative();
                continue;
            };
            if !self.units[id].standing() {
                continue;
            }
            self.current = Some(id);
            self.events.push(Event::TurnStart(id));
            if !self.start_turn(id) {
                // Lost the turn: stunned, or bled out.
                if let Some(o) = self.check_over() {
                    return Step::Over(o);
                }
                return Step::Acted;
            }
            match self.units[id].side {
                Side::Heroes => {
                    if self.act_out(id) {
                        return self.check_over().map_or(Step::Acted, Step::Over);
                    }
                    return Step::HeroTurn(id);
                }
                Side::Monsters => {
                    self.monster_turn(id);
                    return self.check_over().map_or(Step::Acted, Step::Over);
                }
            }
        }
    }

    /// Damage over time, stun and status countdown. Returns whether the unit may act.
    fn start_turn(&mut self, id: UnitId) -> bool {
        let mut dots = Vec::new();
        for s in &self.units[id].statuses {
            match s.kind {
                StatusKind::Bleed(d) => dots.push((d, DotKind::Bleed)),
                StatusKind::Blight(d) => dots.push((d, DotKind::Blight)),
                _ => {}
            }
        }
        for (amount, kind) in dots {
            if !self.units[id].standing() {
                break;
            }
            self.events.push(Event::Tick {
                target: id,
                amount,
                kind,
            });
            self.take_damage(id, amount, None);
        }
        if !self.units[id].standing() {
            return false;
        }
        let stunned = self.units[id].stunned();
        let u = &mut self.units[id];
        u.statuses.retain(|s| !matches!(s.kind, StatusKind::Stun));
        for s in &mut u.statuses {
            s.turns = s.turns.saturating_sub(1);
        }
        u.statuses.retain(|s| s.turns > 0);
        if stunned {
            self.events.push(Event::Stunned(id));
            return false;
        }
        true
    }

    fn check_over(&mut self) -> Option<Outcome> {
        if let Some(o) = self.outcome {
            return Some(o);
        }
        let o = if self.standing(Side::Heroes).is_empty() {
            Outcome::Defeat
        } else if self.standing(Side::Monsters).is_empty() {
            Outcome::Victory
        } else {
            return None;
        };
        self.finish(o);
        Some(o)
    }

    fn finish(&mut self, o: Outcome) {
        if o == Outcome::Victory {
            for id in self.monsters.clone() {
                if self.units[id].corpse {
                    self.units[id].corpse = false;
                    self.units[id].alive = false;
                    self.events.push(Event::CorpseCleared(id));
                }
            }
        }
        self.outcome = Some(o);
        self.current = None;
        self.events.push(Event::Over(o));
    }

    /// An afflicted hero may refuse orders; a virtuous one may rally. Returns whether the turn
    /// was spent.
    fn act_out(&mut self, id: UnitId) -> bool {
        let Some(hero) = self.units[id].hero() else {
            return false;
        };
        if let Some(v) = hero.virtue {
            if self.rng.percent(20) {
                let bark = self.rng.pick(v.barks()).copied().unwrap_or_default();
                self.events.push(Event::Bark(id, bark.to_owned()));
                for h in self.standing(Side::Heroes) {
                    self.stress(h, -5);
                }
            }
            return false;
        }
        let Some(a) = hero.affliction else {
            return false;
        };
        if !self.rng.percent(30) {
            return false;
        }
        let bark = self.rng.pick(a.barks()).copied().unwrap_or_default();
        self.events.push(Event::Bark(id, bark.to_owned()));
        match self.rng.range(0, 2) {
            0 => {
                // Venom on the party.
                for h in self.standing(Side::Heroes) {
                    if h != id {
                        self.stress(h, 5);
                    }
                }
                false
            }
            1 => {
                // Lashes out at whatever is nearest.
                let skills = self.units[id].skills();
                let usable: Vec<_> = skills
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.target.is_enemy() && self.can_use(id, s))
                    .map(|(i, _)| i)
                    .collect();
                if let Some(&i) = self.rng.pick(&usable) {
                    let targets = self.targets_for(id, skills[i]);
                    if let Some(&t) = self.rng.pick(&targets) {
                        self.use_skill(id, skills[i], t);
                        return true;
                    }
                }
                false
            }
            _ => {
                self.events.push(Event::Pass(id));
                true
            }
        }
    }

    /// Carry out a hero's orders. An invalid order is refused and the hero keeps the turn.
    pub fn order(&mut self, id: UnitId, order: Order) -> bool {
        if self.current != Some(id) || self.outcome.is_some() {
            return false;
        }
        match order {
            Order::Skill { skill, target } => {
                let skills = self.units[id].skills();
                let Some(&s) = skills.get(skill) else {
                    return false;
                };
                if !self.can_use(id, s) || !self.targets_for(id, s).contains(&target) {
                    return false;
                }
                self.use_skill(id, s, target);
            }
            Order::Move { to_rank } => {
                let from = self.rank_of(id);
                if to_rank == from || to_rank == 0 || to_rank > self.heroes.len() {
                    return false;
                }
                let delta = from as i32 - to_rank as i32;
                self.shift(id, delta);
            }
            Order::Pass => self.events.push(Event::Pass(id)),
            Order::Retreat => {
                let ok = self.rng.percent(70);
                self.events.push(Event::Retreat(ok));
                if ok {
                    for h in self.standing(Side::Heroes) {
                        self.stress(h, 8);
                    }
                    self.finish(Outcome::Retreated);
                }
            }
        }
        self.current = None;
        self.check_over();
        true
    }

    fn monster_turn(&mut self, id: UnitId) {
        let skills = self.units[id].skills();
        let usable: Vec<usize> = (0..skills.len())
            .filter(|&i| self.can_use(id, skills[i]))
            .collect();
        if usable.is_empty() {
            // Wrong rank for everything: shuffle toward where a skill works.
            let wants_front = skills.iter().any(|s| s.launch & 0b0011 != 0);
            self.shift(id, if wants_front { 1 } else { -1 });
            self.events.push(Event::Pass(id));
            return;
        }
        let weights: Vec<u32> = usable.iter().map(|&i| skills[i].weight.max(1)).collect();
        let pick = usable[self.rng.weighted(&weights).unwrap_or(0)];
        let skill = skills[pick];
        let targets = self.targets_for(id, skill);
        // Heroes are preferred over corpses; marked and taunting heroes draw the eye.
        let weights: Vec<u32> = targets
            .iter()
            .map(|&t| {
                let u = &self.units[t];
                let mut w = if u.corpse { 0 } else { 4 };
                if u.marked() {
                    w += 8;
                }
                if u.has(|k| matches!(k, StatusKind::Taunt)) {
                    w += 16;
                }
                if u.hp * 3 < u.max_hp {
                    w += 2;
                }
                w
            })
            .collect();
        let target = self
            .rng
            .weighted(&weights)
            .map_or(targets[0], |i| targets[i]);
        self.use_skill(id, skill, target);
    }

    // ---------------------------------------------------------------- resolution

    fn use_skill(&mut self, actor: UnitId, skill: &'static SkillDef, aim: UnitId) {
        let affected = self.affected(actor, skill, aim);
        self.events.push(Event::Skill {
            actor,
            skill,
            targets: affected.clone(),
        });
        if skill.limit > 0 {
            let u = &mut self.units[actor];
            match u.uses.iter_mut().find(|(s, _)| *s == skill.id) {
                Some((_, n)) => *n += 1,
                None => u.uses.push((skill.id, 1)),
            }
        }
        let mut dealt = 0;
        for t in affected {
            if !self.units[t].alive {
                continue;
            }
            if skill.target.is_enemy() {
                dealt += self.strike(actor, skill, t);
            } else {
                self.support(actor, skill, t);
            }
        }
        if skill.id == DRAIN_SKILL && dealt > 0 {
            self.heal(actor, (dealt / 2).max(1), false);
        }
        if skill.id == LIFE_TAP_SKILL {
            let cost = (self.units[actor].max_hp / 7).max(1);
            let u = &mut self.units[actor];
            let lose = cost.min(u.hp - 1).max(0);
            if lose > 0 {
                u.hp -= lose;
                self.events.push(Event::Damage {
                    target: actor,
                    amount: lose,
                    crit: false,
                });
            }
        }
        if self.units[actor].standing() {
            for e in skill.self_effects {
                self.apply_effect(actor, actor, e, false);
            }
            if skill.self_move != 0 {
                self.shift(actor, i32::from(skill.self_move));
            }
        }
    }

    /// One hostile hit. Returns damage dealt.
    fn strike(&mut self, actor: UnitId, skill: &'static SkillDef, target: UnitId) -> i32 {
        // A guardian takes the blow.
        let target = self.units[target]
            .statuses
            .iter()
            .find_map(|s| match s.kind {
                StatusKind::GuardedBy(g) if self.units[g].standing() => Some(g),
                _ => None,
            })
            .unwrap_or(target);
        let chance = self.hit_chance(actor, skill, target);
        if !self.rng.percent(chance) {
            self.events.push(Event::Miss(target));
            return 0;
        }
        let crit = skill.dmg_mod > -100 && self.rng.percent(self.crit_chance(actor, skill));
        let mut dealt = 0;
        if skill.dmg_mod > -100 {
            let pct = self.damage_pct(actor, skill, target);
            let (lo, hi) = self.dmg_range(actor);
            let mut dmg = self.rng.range(lo, hi) * pct / 100;
            if crit {
                dmg = dmg * 3 / 2;
            }
            let prot = self.stat(target, Stat::Prot).clamp(0, 80);
            dmg = (dmg * (100 - prot) / 100).max(1);
            self.events.push(Event::Damage {
                target,
                amount: dmg,
                crit,
            });
            dealt = dmg;
            self.take_damage(target, dmg, Some(actor));
            if crit {
                self.on_crit(actor, target);
            }
        }
        if self.units[target].alive {
            for e in skill.effects {
                self.apply_effect(actor, target, e, crit);
            }
        }
        // A riposte answers a blade.
        if skill.kind == SkillKind::Melee
            && self.units[target].standing()
            && self.units[actor].standing()
            && self.units[target].has(|k| matches!(k, StatusKind::Riposte))
        {
            let (lo, hi) = self.dmg_range(target);
            let dmg = (self.rng.range(lo, hi) / 2).max(1);
            self.events.push(Event::Damage {
                target: actor,
                amount: dmg,
                crit: false,
            });
            self.take_damage(actor, dmg, Some(target));
        }
        dealt
    }

    fn on_crit(&mut self, actor: UnitId, target: UnitId) {
        match self.units[actor].side {
            // A hero's great blow heartens the party.
            Side::Heroes => {
                for h in self.standing(Side::Heroes) {
                    self.stress(h, -3);
                }
            }
            // A monster's rattles its victim.
            Side::Monsters => {
                if self.units[target].standing() {
                    self.stress(target, 10);
                }
            }
        }
    }

    fn support(&mut self, actor: UnitId, skill: &'static SkillDef, target: UnitId) {
        let mut crit = false;
        if let Some((lo, hi)) = skill.heal {
            crit = self
                .rng
                .percent(self.stat(actor, Stat::Crit) + skill.crit_mod + 5);
            let mut amt = self.rng.range(lo, hi);
            // Healers scale a little with their level.
            if let Some(h) = self.units[actor].hero() {
                amt += i32::from(h.level) / 2;
            }
            if crit {
                amt *= 2;
            }
            let recv = self.stat(target, Stat::HealRecv);
            amt = (amt * (100 + recv) / 100).max(if recv <= -100 { 0 } else { 1 });
            self.heal(target, amt, crit);
            if crit {
                self.stress(target, -4);
            }
        }
        for e in skill.effects {
            self.apply_effect(actor, target, e, crit);
        }
    }

    pub fn heal(&mut self, target: UnitId, amount: i32, crit: bool) {
        let u = &mut self.units[target];
        if !u.standing() || amount <= 0 {
            return;
        }
        let before = u.hp;
        u.hp = (u.hp + amount).min(u.max_hp);
        u.deaths_door = false;
        self.events.push(Event::Heal {
            target,
            amount: u.hp - before,
            crit,
        });
    }

    fn take_damage(&mut self, target: UnitId, amount: i32, by: Option<UnitId>) {
        let side = self.units[target].side;
        let u = &mut self.units[target];
        if !u.alive {
            return;
        }
        if u.corpse {
            u.hp -= amount;
            if u.hp <= 0 {
                u.alive = false;
                u.corpse = false;
                self.events.push(Event::CorpseCleared(target));
                self.collapse(side);
            }
            return;
        }
        match side {
            Side::Heroes => {
                if u.deaths_door {
                    let resist = match &u.kind {
                        UnitKind::Hero(h) => h.stats().res.death_blow,
                        UnitKind::Monster(_) => 0,
                    };
                    if self.rng.percent(resist) {
                        self.events.push(Event::DeathBlowResisted(target));
                        for h in self.standing(Side::Heroes) {
                            if h != target {
                                self.stress(h, 3);
                            }
                        }
                    } else {
                        self.kill(target, by);
                    }
                } else if u.hp - amount <= 0 {
                    u.hp = 0;
                    u.deaths_door = true;
                    self.events.push(Event::DeathsDoor(target));
                    self.stress(target, 8);
                } else {
                    u.hp -= amount;
                }
            }
            Side::Monsters => {
                u.hp -= amount;
                if u.hp <= 0 {
                    self.kill(target, by);
                }
            }
        }
    }

    fn kill(&mut self, target: UnitId, by: Option<UnitId>) {
        let side = self.units[target].side;
        let has_others = self.standing(side).iter().any(|&o| o != target);
        let u = &mut self.units[target];
        u.hp = 0;
        u.statuses.clear();
        self.events.push(Event::Died(target));
        if let Some(k) = by {
            self.kills.push((k, target));
        }
        match side {
            Side::Heroes => {
                u.alive = false;
                for h in self.standing(Side::Heroes) {
                    self.stress(h, 15);
                }
                self.collapse(side);
            }
            Side::Monsters => {
                if has_others && !u.is_boss() && u.size == 1 {
                    // The remains keep the rank, and the rear stays out of reach a while longer.
                    u.corpse = true;
                    u.hp = (u.max_hp / 3).max(1);
                    self.events.push(Event::Corpse(target));
                } else {
                    u.alive = false;
                    self.collapse(side);
                }
            }
        }
        // Whoever was guarded by the fallen stands alone again.
        for unit in &mut self.units {
            unit.statuses
                .retain(|s| !matches!(s.kind, StatusKind::GuardedBy(g) if g == target));
        }
    }

    /// Drop the dead from a line so the living close ranks.
    fn collapse(&mut self, side: Side) {
        let before = self.line(side).len();
        let alive: Vec<UnitId> = self
            .line(side)
            .iter()
            .copied()
            .filter(|&u| self.units[u].alive)
            .collect();
        *self.line_mut(side) = alive;
        if self.line(side).len() != before {
            self.events.push(Event::Moved);
        }
    }

    /// Move a unit `delta` places toward the front (negative: toward the back).
    fn shift(&mut self, id: UnitId, delta: i32) {
        let side = self.units[id].side;
        let line = self.line_mut(side);
        let Some(pos) = line.iter().position(|&u| u == id) else {
            return;
        };
        let new = (pos as i32 - delta).clamp(0, line.len() as i32 - 1) as usize;
        if new != pos {
            line.remove(pos);
            line.insert(new, id);
            self.events.push(Event::Moved);
        }
    }

    fn resist(
        &mut self,
        target: UnitId,
        stat: Stat,
        chance: i32,
        crit: bool,
        label: &'static str,
    ) -> bool {
        let chance = chance + if crit { 20 } else { 0 } - self.stat(target, stat);
        if self.rng.percent(chance) {
            false
        } else {
            self.events.push(Event::Resisted { target, label });
            true
        }
    }

    fn add_status(&mut self, target: UnitId, kind: StatusKind, turns: u8) {
        let u = &mut self.units[target];
        // A fresh application of the same kind refreshes it rather than stacking forever.
        if let Some(s) = u.statuses.iter_mut().find(|s| {
            std::mem::discriminant(&s.kind) == std::mem::discriminant(&kind)
                && !matches!(
                    kind,
                    StatusKind::Mod(..) | StatusKind::Bleed(_) | StatusKind::Blight(_)
                )
        }) {
            s.kind = kind;
            s.turns = s.turns.max(turns);
            return;
        }
        u.statuses.push(Status { kind, turns });
    }

    fn apply_effect(&mut self, actor: UnitId, target: UnitId, e: &Effect, crit: bool) {
        if !self.units[target].standing() {
            return;
        }
        let enemy = self.units[actor].side != self.units[target].side;
        match *e {
            Effect::Bleed { dmg, turns, chance } => {
                if !self.resist(target, Stat::BleedRes, chance, crit, "Bleed") {
                    self.add_status(target, StatusKind::Bleed(dmg), turns);
                    self.events.push(Event::Status {
                        target,
                        label: "Bleed".into(),
                    });
                }
            }
            Effect::Blight { dmg, turns, chance } => {
                if !self.resist(target, Stat::BlightRes, chance, crit, "Blight") {
                    self.add_status(target, StatusKind::Blight(dmg), turns);
                    self.events.push(Event::Status {
                        target,
                        label: "Blight".into(),
                    });
                }
            }
            Effect::Stun { chance } => {
                if !self.resist(target, Stat::StunRes, chance, crit, "Stun") {
                    self.add_status(target, StatusKind::Stun, 2);
                    self.events.push(Event::Status {
                        target,
                        label: "Stun".into(),
                    });
                }
            }
            Effect::Push { ranks, chance } => {
                if !self.resist(target, Stat::MoveRes, chance, crit, "Move") {
                    self.shift(target, -i32::from(ranks));
                }
            }
            Effect::Pull { ranks, chance } => {
                if !self.resist(target, Stat::MoveRes, chance, crit, "Move") {
                    self.shift(target, i32::from(ranks));
                }
            }
            Effect::Mark { turns } => {
                self.add_status(target, StatusKind::Mark, turns + 1);
                self.events.push(Event::Status {
                    target,
                    label: "Marked".into(),
                });
            }
            Effect::Debuff {
                stat,
                amt,
                turns,
                chance,
            } => {
                if !enemy || !self.resist(target, Stat::DebuffRes, chance, crit, "Debuff") {
                    self.add_status(target, StatusKind::Mod(stat, amt), turns + 1);
                    self.events.push(Event::Status {
                        target,
                        label: format!("{amt:+} {}", stat.label()),
                    });
                }
            }
            Effect::Buff { stat, amt, turns } => {
                if amt != 0 {
                    self.add_status(target, StatusKind::Mod(stat, amt), turns + 1);
                    self.events.push(Event::Status {
                        target,
                        label: format!("{amt:+} {}", stat.label()),
                    });
                }
            }
            Effect::Guard { turns } => {
                if target != actor {
                    self.units[target]
                        .statuses
                        .retain(|s| !matches!(s.kind, StatusKind::GuardedBy(_)));
                    self.add_status(target, StatusKind::GuardedBy(actor), turns + 1);
                    self.events.push(Event::Status {
                        target,
                        label: "Guarded".into(),
                    });
                }
            }
            Effect::Stress(n) => {
                if self.units[target].side == Side::Heroes {
                    self.stress(target, n + if crit { n / 2 } else { 0 });
                }
            }
            Effect::StressHeal(n) => {
                if self.units[target].side == Side::Heroes {
                    self.stress(target, -n);
                }
            }
            Effect::Cure => {
                let u = &mut self.units[target];
                let before = u.statuses.len();
                u.statuses
                    .retain(|s| !matches!(s.kind, StatusKind::Bleed(_) | StatusKind::Blight(_)));
                if u.statuses.len() != before {
                    self.events.push(Event::Status {
                        target,
                        label: "Cured".into(),
                    });
                }
            }
            Effect::Riposte { turns } => {
                self.add_status(target, StatusKind::Riposte, turns + 1);
                self.events.push(Event::Status {
                    target,
                    label: "Riposte".into(),
                });
            }
            Effect::Taunt { turns } => {
                self.add_status(target, StatusKind::Taunt, turns + 1);
                self.events.push(Event::Status {
                    target,
                    label: "Taunt".into(),
                });
            }
        }
    }

    /// Stress a hero (negative relieves), with the resolve test and heart attack.
    pub fn stress(&mut self, target: UnitId, amount: i32) {
        if self.units[target].side != Side::Heroes || !self.units[target].standing() || amount == 0
        {
            return;
        }
        let taken = self.stat(target, Stat::StressTaken);
        let mut rng = std::mem::replace(&mut self.rng, Rng::new(0));
        let UnitKind::Hero(h) = &mut self.units[target].kind else {
            unreachable!("checked side");
        };
        let (delta, outcome) = apply_stress(
            &mut h.stress,
            &mut h.affliction,
            &mut h.virtue,
            amount,
            taken,
            &mut rng,
        );
        self.rng = rng;
        if delta != 0 {
            self.events.push(Event::Stress {
                target,
                amount: delta,
            });
        }
        match outcome {
            StressOutcome::None => {}
            StressOutcome::Afflicted(a) => {
                self.events.push(Event::Afflicted(target, a));
                let bark = self.rng.pick(a.barks()).copied().unwrap_or_default();
                self.events.push(Event::Bark(target, bark.to_owned()));
                for h in self.standing(Side::Heroes) {
                    if h != target {
                        self.stress(h, 4);
                    }
                }
            }
            StressOutcome::Virtuous(v) => {
                self.events.push(Event::Virtuous(target, v));
                let bark = self.rng.pick(v.barks()).copied().unwrap_or_default();
                self.events.push(Event::Bark(target, bark.to_owned()));
                for h in self.standing(Side::Heroes) {
                    if h != target {
                        self.stress(h, -4);
                    }
                }
            }
            StressOutcome::HeartAttack => {
                self.events.push(Event::HeartAttack(target));
                if self.units[target].deaths_door {
                    self.kill(target, None);
                } else {
                    let u = &mut self.units[target];
                    u.hp = 0;
                    u.deaths_door = true;
                    self.events.push(Event::DeathsDoor(target));
                }
            }
        }
    }

    /// The surviving heroes in formation order with the battle's hp and kills written back, and
    /// the fallen.
    pub fn into_heroes(self) -> (Vec<Hero>, Vec<Hero>) {
        let order = self.heroes.clone();
        let kills = self.kills.clone();
        let mut slots: Vec<Option<Unit>> = self.units.into_iter().map(Some).collect();
        let mut take = |id: UnitId| -> Option<(Hero, bool)> {
            let u = slots.get_mut(id)?.take()?;
            match u.kind {
                UnitKind::Hero(h) => {
                    let mut h = *h;
                    h.hp = u.hp;
                    h.kills += kills.iter().filter(|(k, _)| *k == id).count() as u32;
                    Some((h, u.alive))
                }
                UnitKind::Monster(_) => None,
            }
        };
        let mut living = Vec::new();
        let mut dead = Vec::new();
        for id in order {
            if let Some((h, alive)) = take(id) {
                if alive {
                    living.push(h);
                } else {
                    dead.push(h);
                }
            }
        }
        for id in 0..kills.len().max(64) {
            if let Some((h, _)) = take(id) {
                dead.push(h);
            }
        }
        (living, dead)
    }
}

/// Which monster tags a skill is strong against, for the tooltip.
pub fn bonus_label(skill: &SkillDef) -> Option<String> {
    skill.bonus_vs.map(|(tag, pct)| {
        let name = match tag {
            Tag::Undead => "Undead",
            Tag::Beast => "Beasts",
            Tag::Humanoid => "Humanoids",
            Tag::Demon => "Demons",
            Tag::Eldritch => "Eldritch",
            Tag::Kobold => "Kobolds",
        };
        format!("+{pct}% vs {name}")
    })
}

#[cfg(test)]
mod tests {
    use super::super::data::{monster, HeroClass};
    use super::*;

    fn party(rng: &mut Rng) -> Vec<Hero> {
        [
            HeroClass::Warrior,
            HeroClass::Paladin,
            HeroClass::Hunter,
            HeroClass::Priest,
        ]
        .iter()
        .enumerate()
        .map(|(i, c)| Hero::new(i as u32, *c, format!("H{i}"), rng))
        .collect()
    }

    fn pack() -> Vec<&'static MonsterDef> {
        ["skeleton_soldier", "ghoul", "banshee", "skeleton_archer"]
            .iter()
            .map(|id| monster(id).unwrap())
            .collect()
    }

    /// Plays a battle with a simple hero policy: first usable skill on its first target.
    fn autoplay(b: &mut Battle) -> Outcome {
        for _ in 0..2000 {
            match b.step() {
                Step::Over(o) => return o,
                Step::Acted => {}
                Step::HeroTurn(id) => {
                    let skills = b.units[id].skills();
                    let pick = (0..skills.len()).find(|&i| b.can_use(id, skills[i]));
                    let order = match pick {
                        Some(i) => {
                            let t = b.targets_for(id, skills[i])[0];
                            Order::Skill {
                                skill: i,
                                target: t,
                            }
                        }
                        None => Order::Pass,
                    };
                    assert!(b.order(id, order));
                }
            }
            b.events.clear();
        }
        panic!("battle never ended");
    }

    #[test]
    fn battles_end() {
        for seed in 0..40 {
            let mut rng = Rng::new(seed);
            let mut b = Battle::new(party(&mut rng), &pack(), 80, Rng::new(seed + 100));
            autoplay(&mut b);
        }
    }

    #[test]
    fn ranks_count_big_units_twice() {
        let mut rng = Rng::new(1);
        let packs: Vec<_> = ["abomination", "banshee"]
            .iter()
            .map(|i| monster(i).unwrap())
            .collect();
        let b = Battle::new(party(&mut rng), &packs, 80, Rng::new(2));
        let abom = b.monsters[0];
        let banshee = b.monsters[1];
        assert_eq!(b.ranks_of(abom), vec![1, 2]);
        assert_eq!(b.ranks_of(banshee), vec![3]);
    }

    #[test]
    fn heroes_at_zero_go_to_deaths_door_before_dying() {
        let mut rng = Rng::new(1);
        let mut b = Battle::new(party(&mut rng), &pack(), 80, Rng::new(2));
        let h = b.heroes[0];
        b.take_damage(h, 999, None);
        assert!(b.units[h].alive);
        assert!(b.units[h].deaths_door);
        assert_eq!(b.units[h].hp, 0);
    }

    #[test]
    fn rank_rules_gate_skills() {
        let mut rng = Rng::new(1);
        let b = Battle::new(party(&mut rng), &pack(), 80, Rng::new(2));
        let priest = b.heroes[3];
        let skills = b.units[priest].skills();
        let heal = skills.iter().find(|s| s.id == "heal").unwrap();
        assert!(b.can_use(priest, heal), "the priest heals from rank 4");
        let warrior = b.heroes[0];
        let ms = b.units[warrior].skills()[0];
        assert!(b.can_use(warrior, ms));
        // The archer at rank 4 is out of a front-line blade's reach.
        let archer = b.monsters[3];
        assert!(!b.targets_for(warrior, ms).contains(&archer));
    }

    #[test]
    fn moving_swaps_places() {
        let mut rng = Rng::new(1);
        let mut b = Battle::new(party(&mut rng), &pack(), 80, Rng::new(2));
        let first = b.heroes[0];
        b.shift(first, -1);
        assert_eq!(b.rank_of(first), 2);
    }

    #[test]
    fn corpses_hold_the_rank() {
        let mut rng = Rng::new(1);
        let mut b = Battle::new(party(&mut rng), &pack(), 80, Rng::new(2));
        let front = b.monsters[0];
        b.take_damage(front, 999, None);
        assert!(b.units[front].corpse);
        assert_eq!(b.rank_of(b.monsters[1]), 2);
    }
}
