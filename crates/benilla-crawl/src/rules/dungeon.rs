//! An expedition: the map of rooms and hallways, the torch, the pack of provisions, and the
//! curios and traps found along the way.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::data::{monster, MonsterDef, Stat, CRYPT_BOSS_PACK, CRYPT_HALL_PACKS, CRYPT_PACKS};
use super::hero::{apply_stress, Hero, StressOutcome};
use super::rng::Rng;

pub const TORCH_MAX: i32 = 100;
/// Light lost per hallway step.
pub const TORCH_STEP: i32 = 6;
/// Light a torch restores.
pub const TORCH_REFILL: i32 = 25;
/// Hallway steps between hunger checks.
pub const HUNGER_EVERY: u32 = 16;
/// A hall position before its first segment, just out of the room.
pub const HALL_MOUTH: usize = usize::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Item {
    Food,
    Torch,
    Bandage,
    Shovel,
    Key,
    HolyWater,
    Herbs,
}

impl Item {
    pub const ALL: [Item; 7] = [
        Item::Food,
        Item::Torch,
        Item::Bandage,
        Item::Shovel,
        Item::Key,
        Item::HolyWater,
        Item::Herbs,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Item::Food => "Field Rations",
            Item::Torch => "Torch",
            Item::Bandage => "Linen Bandage",
            Item::Shovel => "Shovel",
            Item::Key => "Skeleton Key",
            Item::HolyWater => "Holy Water",
            Item::Herbs => "Peacebloom",
        }
    }

    /// The install's inventory icon (`Interface\Icons\…`).
    pub fn icon(self) -> &'static str {
        match self {
            Item::Food => "INV_Misc_Food_14",
            Item::Torch => "INV_Torch_Lit",
            Item::Bandage => "INV_Misc_Bandage_15",
            Item::Shovel => "INV_Pick_02",
            Item::Key => "INV_Misc_Key_03",
            Item::HolyWater => "INV_Potion_75",
            Item::Herbs => "INV_Misc_Flower_02",
        }
    }

    pub fn price(self) -> i32 {
        match self {
            Item::Food => 75,
            Item::Torch => 75,
            Item::Bandage => 150,
            Item::Shovel => 250,
            Item::Key => 200,
            Item::HolyWater => 150,
            Item::Herbs => 200,
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Item::Food => "Hard bread and salt pork. Eat when the stomach growls, or suffer.",
            Item::Torch => "Restores 25 light. The dark is hungrier than you are.",
            Item::Bandage => "Use on a hero: stops bleeding and heals a little.",
            Item::Shovel => "Clears rubble from the hallway.",
            Item::Key => "Opens locked chests and stranger things.",
            Item::HolyWater => "Use on a hero: purges blight. The dead hate it.",
            Item::Herbs => "Use on a hero: relieves 10 stress.",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Heirloom {
    Bust,
    Portrait,
    Deed,
    Crest,
}

impl Heirloom {
    pub fn name(self) -> &'static str {
        match self {
            Heirloom::Bust => "Marble Bust",
            Heirloom::Portrait => "Faded Portrait",
            Heirloom::Deed => "Old Deed",
            Heirloom::Crest => "House Crest",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Heirloom::Bust => "INV_Misc_Head_Human_01",
            Heirloom::Portrait => "INV_Misc_Note_02",
            Heirloom::Deed => "INV_Scroll_03",
            Heirloom::Crest => "INV_Shield_06",
        }
    }
}

/// What a curio or trap does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    Gold(i32, i32),
    Heirlooms(Heirloom, i32),
    Supplies(Item, i32),
    /// Heal every hero by a percent of their max hp.
    Heal(i32),
    StressHeal(i32),
    Stress(i32),
    /// Hurt the one who touched it.
    Hurt(i32),
    Buff(Stat, i32),
    Quirk(bool),
    Light(i32),
    Nothing,
}

pub struct CurioDef {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    /// The doodad M2 the stage places for it.
    pub model: &'static str,
    pub outcomes: &'static [(u32, Outcome)],
    /// An item that, used on it, guarantees a better result.
    pub key: Option<(Item, Outcome)>,
}

pub static CURIOS: &[CurioDef] = &[
    CurioDef {
        id: "chest",
        name: "Iron-Bound Chest",
        desc: "Heavy, locked, and someone died guarding it.",
        model: "World\\Generic\\ActiveDoodads\\Chest02\\Chest02.m2",
        outcomes: &[
            (5, Outcome::Gold(250, 600)),
            (3, Outcome::Heirlooms(Heirloom::Deed, 2)),
            (2, Outcome::Hurt(4)),
        ],
        key: Some((Item::Key, Outcome::Gold(700, 1200))),
    },
    CurioDef {
        id: "altar",
        name: "Defiled Altar",
        desc: "An altar to the Light, smeared with something that is not wine.",
        model: "World\\KhazModan\\Blackrock\\ActiveDoodads\\AltarRitual\\BlackrockAltarRitual.m2",
        outcomes: &[
            (4, Outcome::Stress(10)),
            (3, Outcome::StressHeal(10)),
            (3, Outcome::Quirk(false)),
        ],
        key: Some((Item::HolyWater, Outcome::StressHeal(25))),
    },
    CurioDef {
        id: "bookshelf",
        name: "Mouldering Bookshelf",
        desc: "Tomes in a hand that hurts to read.",
        model: "World\\Lordaeron\\Scholomance\\PassiveDoodads\\Bookshelves\\Scholme_Bookshelf.m2",
        outcomes: &[
            (4, Outcome::Buff(Stat::Acc, 10)),
            (3, Outcome::Stress(8)),
            (2, Outcome::Quirk(true)),
            (1, Outcome::Nothing),
        ],
        key: None,
    },
    CurioDef {
        id: "sarcophagus",
        name: "Sealed Sarcophagus",
        desc: "The lid is warm.",
        model: "World\\Azeroth\\Duskwood\\PassiveDoodads\\Coffin\\Coffin.m2",
        outcomes: &[
            (4, Outcome::Heirlooms(Heirloom::Bust, 2)),
            (3, Outcome::Hurt(5)),
            (3, Outcome::Gold(150, 400)),
        ],
        key: Some((Item::Shovel, Outcome::Heirlooms(Heirloom::Bust, 4))),
    },
    CurioDef {
        id: "cauldron",
        name: "Bubbling Cauldron",
        desc: "A plague-green brew that smells faintly of apples.",
        model: "World\\Lordaeron\\Scholomance\\PassiveDoodads\\Cauldrons\\GreenBubblingcauldron.m2",
        outcomes: &[
            (4, Outcome::Hurt(4)),
            (3, Outcome::Heal(25)),
            (3, Outcome::Quirk(false)),
        ],
        key: Some((Item::Herbs, Outcome::Heal(50))),
    },
    CurioDef {
        id: "barrel",
        name: "Supply Barrel",
        desc: "Left by an earlier expedition that did not need it anymore.",
        model: "World\\Generic\\PassiveDoodads\\Barrel\\Barrel01.m2",
        outcomes: &[
            (4, Outcome::Supplies(Item::Food, 2)),
            (3, Outcome::Supplies(Item::Torch, 1)),
            (2, Outcome::Supplies(Item::Bandage, 1)),
            (1, Outcome::Nothing),
        ],
        key: None,
    },
    CurioDef {
        id: "brazier",
        name: "Cold Brazier",
        desc: "Ashes, still holding some heat.",
        model:
            "World\\Lordaeron\\Scholomance\\PassiveDoodads\\Brazier\\ScholomanceBrazier01Purple.m2",
        outcomes: &[
            (6, Outcome::Light(40)),
            (3, Outcome::StressHeal(6)),
            (1, Outcome::Nothing),
        ],
        key: None,
    },
    CurioDef {
        id: "portrait",
        name: "Ancestral Portrait",
        desc: "A face you almost recognise.",
        model: "World\\KhazModan\\Uldaman\\PassiveDoodads\\Paintings\\PaintingRuined01.m2",
        outcomes: &[
            (5, Outcome::Heirlooms(Heirloom::Portrait, 2)),
            (3, Outcome::Stress(6)),
            (2, Outcome::Heirlooms(Heirloom::Crest, 1)),
        ],
        key: None,
    },
];

pub fn curio(id: &str) -> Option<&'static CurioDef> {
    CURIOS.iter().find(|c| c.id == id)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RoomKind {
    Entrance,
    Empty,
    Battle(Vec<String>),
    Curio(String),
    Boss(Vec<String>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Room {
    pub grid: (i32, i32),
    pub kind: RoomKind,
    pub visited: bool,
    /// Its battle won or curio used.
    pub cleared: bool,
    pub scouted: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Seg {
    Empty,
    Battle(Vec<String>),
    Trap,
    Curio(String),
    Obstacle,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hall {
    pub a: usize,
    pub b: usize,
    pub segs: Vec<Seg>,
    pub visited: Vec<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Location {
    Room(usize),
    /// Standing on segment `seg` of `hall`, headed to room `toward`.
    Hall {
        hall: usize,
        seg: usize,
        toward: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quest {
    /// Explore this percent of the rooms.
    Explore(u32),
    /// Slay the boss in its lair.
    Boss,
}

impl Quest {
    pub fn label(self) -> String {
        match self {
            Quest::Explore(p) => format!("Explore {p}% of the rooms"),
            Quest::Boss => "Slay the Pale Archivist".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dungeon {
    pub name: String,
    pub rooms: Vec<Room>,
    pub halls: Vec<Hall>,
    pub at: Location,
    pub quest: Quest,
    pub steps: u32,
}

impl Dungeon {
    pub fn generate(rng: &mut Rng, rooms: usize, quest: Quest) -> Dungeon {
        let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        let mut grid: Vec<(i32, i32)> = vec![(0, 0)];
        let mut edges: Vec<(usize, usize)> = Vec::new();
        while grid.len() < rooms {
            let from = rng.index(grid.len());
            let (dx, dy) = *rng.pick(&dirs).expect("dirs");
            let cell = (grid[from].0 + dx, grid[from].1 + dy);
            if grid.contains(&cell) {
                continue;
            }
            grid.push(cell);
            edges.push((from, grid.len() - 1));
        }
        // A couple of loops, so the map is not a pure tree.
        for _ in 0..rooms / 4 {
            let a = rng.index(grid.len());
            for (b, cell) in grid.iter().enumerate() {
                let adj = (cell.0 - grid[a].0).abs() + (cell.1 - grid[a].1).abs() == 1;
                if adj && !edges.iter().any(|e| *e == (a, b) || *e == (b, a)) {
                    edges.push((a, b));
                    break;
                }
            }
        }
        // The lair is the room farthest from the entrance by hops.
        let far = farthest(grid.len(), &edges, 0);
        let mut room_list = Vec::new();
        for (i, cell) in grid.iter().enumerate() {
            let kind = if i == 0 {
                RoomKind::Entrance
            } else if quest == Quest::Boss && i == far {
                RoomKind::Boss(CRYPT_BOSS_PACK.iter().map(|s| (*s).to_owned()).collect())
            } else {
                match rng.range(0, 99) {
                    0..=44 => RoomKind::Battle(pack(rng, CRYPT_PACKS)),
                    45..=74 => RoomKind::Curio(rng.pick(CURIOS).expect("curios").id.to_owned()),
                    _ => RoomKind::Empty,
                }
            };
            room_list.push(Room {
                grid: *cell,
                kind,
                visited: i == 0,
                cleared: i == 0,
                scouted: i == 0,
            });
        }
        let halls = edges
            .into_iter()
            .map(|(a, b)| {
                let segs: Vec<Seg> = (0..4)
                    .map(|_| match rng.range(0, 99) {
                        0..=13 => Seg::Battle(pack(rng, CRYPT_HALL_PACKS)),
                        14..=23 => Seg::Trap,
                        24..=37 => Seg::Curio(rng.pick(CURIOS).expect("curios").id.to_owned()),
                        38..=42 => Seg::Obstacle,
                        _ => Seg::Empty,
                    })
                    .collect();
                let n = segs.len();
                Hall {
                    a,
                    b,
                    segs,
                    visited: vec![false; n],
                }
            })
            .collect();
        Dungeon {
            name: "The Sunken Crypts".to_owned(),
            rooms: room_list,
            halls,
            at: Location::Room(0),
            quest,
            steps: 0,
        }
    }

    /// The halls leaving a room, with the room at the far end.
    pub fn exits(&self, room: usize) -> Vec<(usize, usize)> {
        self.halls
            .iter()
            .enumerate()
            .filter_map(|(i, h)| {
                if h.a == room {
                    Some((i, h.b))
                } else if h.b == room {
                    Some((i, h.a))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Segment index along a hall as walked from `from`.
    pub fn seg_index(&self, hall: usize, step: usize, toward: usize) -> usize {
        let h = &self.halls[hall];
        if toward == h.b {
            step
        } else {
            h.segs.len() - 1 - step
        }
    }

    pub fn explored_pct(&self) -> u32 {
        let v = self.rooms.iter().filter(|r| r.visited).count();
        (v * 100 / self.rooms.len().max(1)) as u32
    }

    pub fn quest_done(&self) -> bool {
        match self.quest {
            Quest::Explore(p) => self.explored_pct() >= p,
            Quest::Boss => self
                .rooms
                .iter()
                .any(|r| matches!(r.kind, RoomKind::Boss(_)) && r.cleared),
        }
    }

    /// Mark the rooms next to `room` as seen on the map.
    pub fn reveal_around(&mut self, room: usize) {
        for (_, other) in self.exits(room) {
            self.rooms[other].scouted = true;
        }
    }
}

fn pack(rng: &mut Rng, packs: &[&[&str]]) -> Vec<String> {
    rng.pick(packs)
        .expect("packs")
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
}

fn farthest(n: usize, edges: &[(usize, usize)], from: usize) -> usize {
    let mut dist = vec![usize::MAX; n];
    dist[from] = 0;
    let mut queue = std::collections::VecDeque::from([from]);
    while let Some(u) = queue.pop_front() {
        for &(a, b) in edges {
            let v = if a == u {
                b
            } else if b == u {
                a
            } else {
                continue;
            };
            if dist[v] == usize::MAX {
                dist[v] = dist[u] + 1;
                queue.push_back(v);
            }
        }
    }
    (0..n).max_by_key(|&i| dist[i]).unwrap_or(0)
}

pub fn monsters_of(ids: &[String]) -> Vec<&'static MonsterDef> {
    ids.iter().filter_map(|id| monster(id)).collect()
}

/// What a step or an interaction reported, for the narrator and the log.
#[derive(Clone, Debug, PartialEq)]
pub enum Note {
    Text(String),
    Gold(i32),
    Heirloom(Heirloom, i32),
    Item(Item, i32),
    HeroStress { hero: usize, amount: i32 },
    HeroHurt { hero: usize, amount: i32 },
    HeroHeal { hero: usize, amount: i32 },
    Afflicted { hero: usize, name: &'static str },
    Virtuous { hero: usize, name: &'static str },
    Light(i32),
    Died { hero: usize },
}

/// What waits at the party's new position.
#[derive(Clone, Debug, PartialEq)]
pub enum Encounter {
    None,
    Battle(Vec<String>),
    Curio(String),
    Obstacle,
    Trap,
}

/// Everything that rides along on an expedition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Expedition {
    pub party: Vec<Hero>,
    pub fallen: Vec<Hero>,
    pub dungeon: Dungeon,
    pub light: i32,
    pub pack: HashMap<Item, i32>,
    pub gold: i32,
    pub heirlooms: HashMap<Heirloom, i32>,
    /// Temporary curio buffs, applied to the next battle.
    pub buffs: Vec<(Stat, i32)>,
    pub rng: Rng,
    pub battles_won: u32,
}

impl Expedition {
    pub fn new(party: Vec<Hero>, dungeon: Dungeon, pack: HashMap<Item, i32>, rng: Rng) -> Self {
        Expedition {
            party,
            fallen: Vec::new(),
            dungeon,
            light: TORCH_MAX,
            pack,
            gold: 0,
            heirlooms: HashMap::new(),
            buffs: Vec::new(),
            rng,
            battles_won: 0,
        }
    }

    pub fn count(&self, item: Item) -> i32 {
        self.pack.get(&item).copied().unwrap_or(0)
    }

    pub fn take(&mut self, item: Item) -> bool {
        let n = self.pack.entry(item).or_insert(0);
        if *n > 0 {
            *n -= 1;
            true
        } else {
            false
        }
    }

    pub fn give(&mut self, item: Item, n: i32) {
        *self.pack.entry(item).or_insert(0) += n;
    }

    pub fn stress_hero(&mut self, hero: usize, amount: i32, notes: &mut Vec<Note>) {
        let Some(h) = self.party.get_mut(hero) else {
            return;
        };
        let taken = h.mod_total(Stat::StressTaken) + super::combat::light_mods(self.light).stress;
        let (delta, out) = apply_stress(
            &mut h.stress,
            &mut h.affliction,
            &mut h.virtue,
            amount,
            taken,
            &mut self.rng,
        );
        if delta != 0 {
            notes.push(Note::HeroStress {
                hero,
                amount: delta,
            });
        }
        match out {
            StressOutcome::None => {}
            StressOutcome::Afflicted(a) => notes.push(Note::Afflicted {
                hero,
                name: a.name(),
            }),
            StressOutcome::Virtuous(v) => notes.push(Note::Virtuous {
                hero,
                name: v.name(),
            }),
            StressOutcome::HeartAttack => {
                if h.hp <= 0 {
                    notes.push(Note::Died { hero });
                    let dead = self.party.remove(hero);
                    self.fallen.push(dead);
                } else {
                    h.hp = 0;
                    notes.push(Note::Text(format!("{}'s heart gives out!", h.name)));
                }
            }
        }
    }

    fn hurt_hero(&mut self, hero: usize, amount: i32, notes: &mut Vec<Note>) {
        if let Some(h) = self.party.get_mut(hero) {
            // Out of battle, wounds stop at the brink.
            let dmg = amount.min(h.hp.max(1) - 1).max(0);
            h.hp -= dmg;
            notes.push(Note::HeroHurt { hero, amount: dmg });
        }
    }

    /// Walk one hallway step toward `toward` (or into the room at the end).
    pub fn advance(&mut self) -> (Encounter, Vec<Note>) {
        let mut notes = Vec::new();
        let Location::Hall { hall, seg, toward } = self.dungeon.at else {
            return (Encounter::None, notes);
        };
        let len = self.dungeon.halls[hall].segs.len();
        if seg != HALL_MOUTH && seg + 1 >= len {
            self.dungeon.at = Location::Room(toward);
            return self.enter_room(toward, notes);
        }
        let step = if seg == HALL_MOUTH { 0 } else { seg + 1 };
        self.dungeon.at = Location::Hall {
            hall,
            seg: step,
            toward,
        };
        self.dungeon.steps += 1;
        let old = self.light;
        self.light = (self.light - TORCH_STEP).max(0);
        if old != self.light {
            notes.push(Note::Light(self.light - old));
        }
        // The dark itself frays nerves.
        if self.light < 50 && self.rng.percent(30) {
            let h = self.rng.index(self.party.len());
            self.stress_hero(h, if self.light == 0 { 6 } else { 3 }, &mut notes);
        }
        if self.dungeon.steps.is_multiple_of(HUNGER_EVERY) {
            self.hunger(&mut notes);
        }
        let idx = self.dungeon.seg_index(hall, step, toward);
        self.dungeon.halls[hall].visited[idx] = true;
        let enc = match self.dungeon.halls[hall].segs[idx].clone() {
            Seg::Empty => Encounter::None,
            Seg::Battle(p) => Encounter::Battle(p),
            Seg::Curio(c) => Encounter::Curio(c),
            Seg::Obstacle => Encounter::Obstacle,
            Seg::Trap => Encounter::Trap,
        };
        (enc, notes)
    }

    pub fn enter_room(&mut self, room: usize, mut notes: Vec<Note>) -> (Encounter, Vec<Note>) {
        self.dungeon.at = Location::Room(room);
        let r = &mut self.dungeon.rooms[room];
        r.visited = true;
        r.scouted = true;
        self.dungeon.reveal_around(room);
        let r = &self.dungeon.rooms[room];
        if r.cleared {
            return (Encounter::None, notes);
        }
        let boss = matches!(r.kind, RoomKind::Boss(_));
        let enc = match &r.kind {
            RoomKind::Battle(p) | RoomKind::Boss(p) => Encounter::Battle(p.clone()),
            RoomKind::Curio(c) => Encounter::Curio(c.clone()),
            RoomKind::Empty | RoomKind::Entrance => Encounter::None,
        };
        if enc == Encounter::None {
            self.dungeon.rooms[room].cleared = true;
        }
        if boss {
            notes.push(Note::Text(
                "The air turns to ice. Something ancient looks up from its reading.".into(),
            ));
            for h in 0..self.party.len() {
                self.stress_hero(h, 6, &mut notes);
            }
        }
        (enc, notes)
    }

    /// Leave a room down one of its halls; the party stands at its mouth, before segment 0.
    pub fn enter_hall(&mut self, hall: usize) -> bool {
        let Location::Room(room) = self.dungeon.at else {
            return false;
        };
        let Some(&(_, toward)) = self.dungeon.exits(room).iter().find(|(h, _)| *h == hall) else {
            return false;
        };
        self.dungeon.at = Location::Hall {
            hall,
            seg: HALL_MOUTH,
            toward,
        };
        true
    }

    /// Turn around in a hallway and head back the way the party came.
    pub fn turn_back(&mut self) {
        if let Location::Hall { hall, seg, toward } = self.dungeon.at {
            let h = &self.dungeon.halls[hall];
            let back = if toward == h.b { h.a } else { h.b };
            let len = h.segs.len();
            if seg == HALL_MOUTH {
                self.dungeon.at = Location::Room(back);
                return;
            }
            self.dungeon.at = Location::Hall {
                hall,
                seg: len - 1 - seg,
                toward: back,
            };
        }
    }

    /// The encounter on the segment the party stands on, for entering a hall's first step.
    pub fn here(&self) -> Encounter {
        match self.dungeon.at {
            Location::Hall {
                seg: HALL_MOUTH, ..
            } => Encounter::None,
            Location::Hall { hall, seg, toward } => {
                let idx = self.dungeon.seg_index(hall, seg, toward);
                match &self.dungeon.halls[hall].segs[idx] {
                    Seg::Empty => Encounter::None,
                    Seg::Battle(p) => Encounter::Battle(p.clone()),
                    Seg::Curio(c) => Encounter::Curio(c.clone()),
                    Seg::Obstacle => Encounter::Obstacle,
                    Seg::Trap => Encounter::Trap,
                }
            }
            Location::Room(r) => {
                let room = &self.dungeon.rooms[r];
                if room.cleared {
                    return Encounter::None;
                }
                match &room.kind {
                    RoomKind::Battle(p) | RoomKind::Boss(p) => Encounter::Battle(p.clone()),
                    RoomKind::Curio(c) => Encounter::Curio(c.clone()),
                    _ => Encounter::None,
                }
            }
        }
    }

    /// Clear whatever was at the party's position (a won battle, a used curio, a trap sprung).
    pub fn clear_here(&mut self) {
        match self.dungeon.at {
            Location::Hall {
                seg: HALL_MOUTH, ..
            } => {}
            Location::Hall { hall, seg, toward } => {
                let idx = self.dungeon.seg_index(hall, seg, toward);
                self.dungeon.halls[hall].segs[idx] = Seg::Empty;
            }
            Location::Room(r) => self.dungeon.rooms[r].cleared = true,
        }
    }

    fn hunger(&mut self, notes: &mut Vec<Note>) {
        let need = self.party.len() as i32;
        if self.count(Item::Food) >= need {
            *self.pack.get_mut(&Item::Food).expect("food") -= need;
            notes.push(Note::Text("The party eats. A small mercy.".into()));
            notes.push(Note::Item(Item::Food, -need));
            for i in 0..self.party.len() {
                let h = &mut self.party[i];
                let heal = (h.max_hp() / 10).max(1);
                h.hp = (h.hp + heal).min(h.max_hp());
            }
        } else {
            self.pack.insert(Item::Food, 0);
            notes.push(Note::Text(
                "Starving. Stomachs knot and tempers fray.".into(),
            ));
            for i in 0..self.party.len() {
                let amount = (self.party[i].max_hp() / 5).max(1);
                self.hurt_hero(i, amount, notes);
                self.stress_hero(i, 15, notes);
            }
        }
    }

    pub fn use_torch(&mut self) -> bool {
        if self.light >= TORCH_MAX || !self.take(Item::Torch) {
            return false;
        }
        self.light = (self.light + TORCH_REFILL).min(TORCH_MAX);
        true
    }

    /// Damp the torch for the brave and greedy (better loot, worse odds).
    pub fn snuff(&mut self) {
        self.light = (self.light - TORCH_REFILL).max(0);
    }

    /// Use a consumable on a hero outside battle.
    pub fn use_on_hero(&mut self, item: Item, hero: usize) -> Vec<Note> {
        let mut notes = Vec::new();
        if hero >= self.party.len() || self.count(item) == 0 {
            return notes;
        }
        match item {
            Item::Bandage => {
                self.take(item);
                let h = &mut self.party[hero];
                let amt = (h.max_hp() / 5).max(2).min(h.max_hp() - h.hp);
                h.hp += amt;
                notes.push(Note::HeroHeal { hero, amount: amt });
            }
            Item::Herbs => {
                self.take(item);
                self.stress_hero(hero, -10, &mut notes);
            }
            Item::Food => {
                self.take(item);
                let h = &mut self.party[hero];
                let amt = (h.max_hp() / 10).max(1).min(h.max_hp() - h.hp);
                h.hp += amt;
                notes.push(Note::HeroHeal { hero, amount: amt });
            }
            Item::HolyWater => {
                self.take(item);
                self.stress_hero(hero, -5, &mut notes);
            }
            _ => {}
        }
        notes
    }

    /// Investigate a curio, optionally with an item. `hero` is who reaches in.
    pub fn interact(&mut self, curio_id: &str, item: Option<Item>, hero: usize) -> Vec<Note> {
        let mut notes = Vec::new();
        let Some(c) = curio(curio_id) else {
            return notes;
        };
        let outcome = match (item, c.key) {
            (Some(i), Some((k, out))) if i == k && self.take(i) => out,
            _ => {
                let weights: Vec<u32> = c.outcomes.iter().map(|(w, _)| *w).collect();
                let i = self.rng.weighted(&weights).unwrap_or(0);
                c.outcomes[i].1
            }
        };
        self.apply(outcome, hero, &mut notes);
        self.clear_here();
        notes
    }

    /// Spring a hallway trap on the lead hero, unless they spot it.
    pub fn spring_trap(&mut self) -> Vec<Note> {
        let mut notes = Vec::new();
        let lead = 0;
        let dodge = self.party.get(lead).map_or(0, |h| h.stats().dodge);
        if self.rng.percent(25 + dodge + self.light / 5) {
            notes.push(Note::Text(
                "A pressure plate, spotted just in time. Disarmed.".into(),
            ));
        } else {
            match self.rng.range(0, 2) {
                0 => {
                    notes.push(Note::Text("Darts hiss from the walls!".into()));
                    let amt = self.rng.range(3, 6);
                    self.hurt_hero(lead, amt, &mut notes);
                }
                1 => {
                    notes.push(Note::Text(
                        "Grave gas seeps up through the flagstones.".into(),
                    ));
                    for i in 0..self.party.len() {
                        let amt = self.rng.range(1, 3);
                        self.hurt_hero(i, amt, &mut notes);
                    }
                }
                _ => {
                    notes.push(Note::Text(
                        "A swinging blade, a scream, a near thing.".into(),
                    ));
                    self.stress_hero(lead, 12, &mut notes);
                }
            }
        }
        self.clear_here();
        notes
    }

    /// Dig through rubble, or force a way and pay for it.
    pub fn clear_obstacle(&mut self) -> Vec<Note> {
        let mut notes = Vec::new();
        if self.take(Item::Shovel) {
            notes.push(Note::Text("The shovel bites; the way is clear.".into()));
            notes.push(Note::Item(Item::Shovel, -1));
        } else {
            notes.push(Note::Text(
                "Bare hands and broken nails. The way is clear.".into(),
            ));
            for i in 0..self.party.len() {
                self.stress_hero(i, 6, &mut notes);
            }
            self.light = (self.light - 15).max(0);
        }
        self.clear_here();
        notes
    }

    fn apply(&mut self, o: Outcome, hero: usize, notes: &mut Vec<Note>) {
        match o {
            Outcome::Gold(lo, hi) => {
                let g = self.rng.range(lo, hi) * (100 + self.dark_bonus()) / 100;
                self.gold += g;
                notes.push(Note::Gold(g));
            }
            Outcome::Heirlooms(kind, n) => {
                *self.heirlooms.entry(kind).or_insert(0) += n;
                notes.push(Note::Heirloom(kind, n));
            }
            Outcome::Supplies(item, n) => {
                self.give(item, n);
                notes.push(Note::Item(item, n));
            }
            Outcome::Heal(pct) => {
                for (i, h) in self.party.iter_mut().enumerate() {
                    let amt = (h.max_hp() * pct / 100).min(h.max_hp() - h.hp);
                    h.hp += amt;
                    notes.push(Note::HeroHeal {
                        hero: i,
                        amount: amt,
                    });
                }
            }
            Outcome::StressHeal(n) => {
                for i in 0..self.party.len() {
                    self.stress_hero(i, -n, notes);
                }
            }
            Outcome::Stress(n) => self.stress_hero(hero, n, notes),
            Outcome::Hurt(n) => self.hurt_hero(hero, n, notes),
            Outcome::Buff(stat, amt) => {
                self.buffs.push((stat, amt));
                notes.push(Note::Text(format!(
                    "A strange clarity: {amt:+} {} for the next battle.",
                    stat.label()
                )));
            }
            Outcome::Quirk(positive) => {
                let pool: Vec<_> = super::hero::QUIRKS
                    .iter()
                    .filter(|q| q.positive == positive)
                    .collect();
                if let (Some(q), Some(h)) = (self.rng.pick(&pool), self.party.get_mut(hero)) {
                    if !h.quirks.iter().any(|x| x == q.id) {
                        h.quirks.push(q.id.to_owned());
                        notes.push(Note::Text(format!(
                            "{} gains the quirk: {}.",
                            h.name, q.name
                        )));
                    }
                }
            }
            Outcome::Light(n) => {
                let old = self.light;
                self.light = (self.light + n).min(TORCH_MAX);
                notes.push(Note::Light(self.light - old));
            }
            Outcome::Nothing => notes.push(Note::Text("Nothing. Only dust.".into())),
        }
    }

    /// Loot is richer the darker it gets.
    pub fn dark_bonus(&self) -> i32 {
        match self.light {
            76.. => 0,
            51..=75 => 10,
            26..=50 => 20,
            1..=25 => 30,
            _ => 50,
        }
    }

    /// The spoils of a won battle.
    pub fn battle_loot(&mut self, boss: bool) -> Vec<Note> {
        let mut notes = Vec::new();
        self.battles_won += 1;
        let base = if boss {
            self.rng.range(1500, 2500)
        } else {
            self.rng.range(120, 380)
        };
        let g = base * (100 + self.dark_bonus()) / 100;
        self.gold += g;
        notes.push(Note::Gold(g));
        if boss || self.rng.percent(30) {
            let kind = *self
                .rng
                .pick(&[
                    Heirloom::Bust,
                    Heirloom::Portrait,
                    Heirloom::Deed,
                    Heirloom::Crest,
                ])
                .expect("heirlooms");
            let n = if boss { 6 } else { self.rng.range(1, 2) };
            *self.heirlooms.entry(kind).or_insert(0) += n;
            notes.push(Note::Heirloom(kind, n));
        }
        if self.rng.percent(25) {
            self.give(Item::Food, 1);
            notes.push(Note::Item(Item::Food, 1));
        }
        notes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_are_connected_with_a_lair() {
        for seed in 0..30 {
            let mut rng = Rng::new(seed);
            let d = Dungeon::generate(&mut rng, 9, Quest::Boss);
            assert_eq!(d.rooms.len(), 9);
            assert!(d.halls.len() >= 8);
            assert_eq!(
                d.rooms
                    .iter()
                    .filter(|r| matches!(r.kind, RoomKind::Boss(_)))
                    .count(),
                1
            );
            assert!(
                farthest(
                    9,
                    &d.halls.iter().map(|h| (h.a, h.b)).collect::<Vec<_>>(),
                    0
                ) > 0
            );
        }
    }

    #[test]
    fn walking_a_hall_spends_light_and_arrives() {
        let mut rng = Rng::new(5);
        let d = Dungeon::generate(&mut rng, 6, Quest::Explore(80));
        let party = vec![Hero::new(
            0,
            super::super::data::HeroClass::Warrior,
            "A".into(),
            &mut rng,
        )];
        let mut e = Expedition::new(party, d, HashMap::new(), Rng::new(9));
        let (hall, to) = e.dungeon.exits(0)[0];
        assert!(e.enter_hall(hall));
        for _ in 0..4 {
            e.advance();
        }
        assert!(e.light < TORCH_MAX);
        e.advance();
        assert_eq!(e.dungeon.at, Location::Room(to));
        assert!(e.dungeon.rooms[to].visited);
    }
}
