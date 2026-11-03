//! An expedition: the map of rooms and hallways, the torch, the pack of provisions, the curios and
//! traps along the way, and the camp at night.
//!
//! Maps follow the shape of the genre: rooms sit on a grid, joined by hallways of three to five
//! squares; the entrance is a room, the lair is the farthest room from it, and a few extra
//! hallways close loops so the map is not a pure tree.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::camp::{CampEffect, CampSkill, CampTarget};
use super::data::{monster, MonsterDef, Stat};
use super::hero::{apply_stress, Hero, StressOutcome};
use super::items::{roll_trinket, roll_trinket_at_least, trinket, Rarity};
use super::regions::{gather_target, level_name, Length, QuestKind, Region};
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
            Item::Food => "Hard bread and salt pork. Eaten at hunger checks and at camp.",
            Item::Torch => "Restores 25 light. The dark is hungrier than you are.",
            Item::Bandage => "Use on a hero: stops bleeding and heals a little.",
            Item::Shovel => "Clears rubble from the hallway; opens some curios.",
            Item::Key => "Opens locked chests and stranger things.",
            Item::HolyWater => "Use on a hero: purges blight and calms. The dead hate it.",
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
    pub const ALL: [Heirloom; 4] = [
        Heirloom::Bust,
        Heirloom::Portrait,
        Heirloom::Deed,
        Heirloom::Crest,
    ];

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
    /// A trinket, at least this rare.
    Trinket(Rarity),
    /// The quest item a gather quest wants.
    QuestItem,
    Nothing,
}

#[derive(Debug)]
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

const fn curio_def(
    id: &'static str,
    name: &'static str,
    desc: &'static str,
    model: &'static str,
    outcomes: &'static [(u32, Outcome)],
    key: Option<(Item, Outcome)>,
) -> CurioDef {
    CurioDef {
        id,
        name,
        desc,
        model,
        outcomes,
        key,
    }
}

pub static CURIOS: &[CurioDef] = &[
    curio_def(
        "chest",
        "Iron-Bound Chest",
        "Heavy, locked, and someone died guarding it.",
        "World\\Generic\\ActiveDoodads\\Chest02\\Chest02.m2",
        &[(5, Outcome::Gold(250, 600)), (3, Outcome::Heirlooms(Heirloom::Deed, 2)), (2, Outcome::Hurt(4)), (1, Outcome::Trinket(Rarity::Common))],
        Some((Item::Key, Outcome::Trinket(Rarity::Uncommon))),
    ),
    curio_def(
        "altar",
        "Defiled Altar",
        "An altar to the Light, smeared with something that is not wine.",
        "World\\KhazModan\\Blackrock\\ActiveDoodads\\AltarRitual\\BlackrockAltarRitual.m2",
        &[(4, Outcome::Stress(10)), (3, Outcome::StressHeal(10)), (3, Outcome::Quirk(false))],
        Some((Item::HolyWater, Outcome::StressHeal(25))),
    ),
    curio_def(
        "bookshelf",
        "Mouldering Bookshelf",
        "Tomes in a hand that hurts to read.",
        "World\\Lordaeron\\Scholomance\\PassiveDoodads\\Bookshelves\\Scholme_Bookshelf.m2",
        &[(4, Outcome::Buff(Stat::Acc, 10)), (3, Outcome::Stress(8)), (2, Outcome::Quirk(true)), (1, Outcome::Nothing)],
        None,
    ),
    curio_def(
        "sarcophagus",
        "Sealed Sarcophagus",
        "The lid is warm.",
        "World\\Azeroth\\Duskwood\\PassiveDoodads\\Coffin\\Coffin.m2",
        &[(4, Outcome::Heirlooms(Heirloom::Bust, 2)), (3, Outcome::Hurt(5)), (3, Outcome::Gold(150, 400))],
        Some((Item::Shovel, Outcome::Heirlooms(Heirloom::Bust, 4))),
    ),
    curio_def(
        "cauldron",
        "Bubbling Cauldron",
        "A plague-green brew that smells faintly of apples.",
        "World\\Lordaeron\\Scholomance\\PassiveDoodads\\Cauldrons\\GreenBubblingcauldron.m2",
        &[(4, Outcome::Hurt(4)), (3, Outcome::Heal(25)), (3, Outcome::Quirk(false))],
        Some((Item::Herbs, Outcome::Heal(50))),
    ),
    curio_def(
        "barrel",
        "Supply Barrel",
        "Left by an earlier expedition that did not need it anymore.",
        "World\\Generic\\PassiveDoodads\\Barrel\\Barrel01.m2",
        &[(4, Outcome::Supplies(Item::Food, 2)), (3, Outcome::Supplies(Item::Torch, 1)), (2, Outcome::Supplies(Item::Bandage, 1)), (1, Outcome::Nothing)],
        None,
    ),
    curio_def(
        "brazier",
        "Cold Brazier",
        "Ashes, still holding some heat.",
        "World\\Lordaeron\\Scholomance\\PassiveDoodads\\Brazier\\ScholomanceBrazier01Purple.m2",
        &[(6, Outcome::Light(40)), (3, Outcome::StressHeal(6)), (1, Outcome::Nothing)],
        None,
    ),
    curio_def(
        "portrait",
        "Ancestral Portrait",
        "A face you almost recognise.",
        "World\\KhazModan\\Uldaman\\PassiveDoodads\\Paintings\\PaintingRuined01.m2",
        &[(5, Outcome::Heirlooms(Heirloom::Portrait, 2)), (3, Outcome::Stress(6)), (2, Outcome::Heirlooms(Heirloom::Crest, 1))],
        None,
    ),
    curio_def(
        "minecart",
        "Abandoned Mine Cart",
        "Half full of ore, half full of something that moved.",
        "World\\Azeroth\\Elwynn\\PassiveDoodads\\MineCart\\ElwynnMineCart.m2",
        &[(5, Outcome::Gold(200, 500)), (3, Outcome::Hurt(4)), (2, Outcome::Heirlooms(Heirloom::Crest, 1))],
        Some((Item::Shovel, Outcome::Gold(600, 1000))),
    ),
    curio_def(
        "ore_vein",
        "Silver-Shot Rubble",
        "Veins of silver glint through the stone.",
        "World\\Azeroth\\Karazahn\\PassiveDoodads\\Rubble\\KarazahnRockRubble02.m2",
        &[(5, Outcome::Gold(150, 350)), (3, Outcome::Hurt(3)), (2, Outcome::Nothing)],
        Some((Item::Shovel, Outcome::Gold(500, 900))),
    ),
    curio_def(
        "kobold_cache",
        "Kobold Candle Cache",
        "A hoard of stolen wax, and stranger trinkets.",
        "World\\Azeroth\\Karazahn\\PassiveDoodads\\Crates\\KarazahnCrate01.m2",
        &[(4, Outcome::Supplies(Item::Torch, 2)), (3, Outcome::Trinket(Rarity::Common)), (3, Outcome::Stress(6))],
        Some((Item::Key, Outcome::Trinket(Rarity::Uncommon))),
    ),
    curio_def(
        "clam",
        "Giant Clam",
        "It is breathing. Slowly.",
        "World\\Generic\\ActiveDoodads\\Clam\\GiantClamActive.m2",
        &[(4, Outcome::Heirlooms(Heirloom::Bust, 2)), (3, Outcome::Hurt(5)), (2, Outcome::Trinket(Rarity::Common)), (1, Outcome::Nothing)],
        Some((Item::Shovel, Outcome::Heirlooms(Heirloom::Bust, 4))),
    ),
    curio_def(
        "shipwreck_chest",
        "Waterlogged Strongbox",
        "From a ship the sea swallowed whole.",
        "World\\Generic\\ActiveDoodads\\Chest01\\Chest01.m2",
        &[(5, Outcome::Gold(300, 700)), (3, Outcome::Heirlooms(Heirloom::Deed, 2)), (2, Outcome::Hurt(4))],
        Some((Item::Key, Outcome::Trinket(Rarity::Uncommon))),
    ),
    curio_def(
        "tide_shrine",
        "Shrine of the Tides",
        "Fish bones and pearls, arranged in a pattern that hurts the eye.",
        "World\\Azeroth\\Stranglethorn\\PassiveDoodads\\SerpentStatue02\\SerpentStatue02.m2",
        &[(4, Outcome::Stress(12)), (3, Outcome::Buff(Stat::Dmg, 15)), (3, Outcome::Heal(20))],
        Some((Item::HolyWater, Outcome::StressHeal(20))),
    ),
    curio_def(
        "reliquary",
        "Family Reliquary",
        "The bones of an ancestor, sealed in silver. The hamlet wants it back.",
        "World\\Azeroth\\Duskwood\\PassiveDoodads\\Tombs\\TombStoneMonument01.m2",
        &[(1, Outcome::QuestItem)],
        None,
    ),
    curio_def(
        "silver_cache",
        "Silver Cache",
        "Ingots stamped with the family crest. Owed to the hamlet.",
        "World\\Generic\\DarkIronDwarf\\Passive Doodads\\Crates\\DarkIronCrate01.m2",
        &[(1, Outcome::QuestItem)],
        None,
    ),
    curio_def(
        "tide_idol",
        "Drowned Idol",
        "A stone god from before the estate. The sea wants it; so does the hamlet.",
        "World\\Azeroth\\Stranglethorn\\PassiveDoodads\\TrollDungeonSerpentStatue\\TrollDungeonSerpentStatue.m2",
        &[(1, Outcome::QuestItem)],
        None,
    ),
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
    /// Squares whose contents a scout has seen without walking them.
    #[serde(default)]
    pub scouted: Vec<bool>,
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

/// What an expedition is for, and where.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quest {
    pub region: Region,
    pub length: Length,
    pub kind: QuestKind,
    pub level: u8,
}

impl Quest {
    pub fn new(region: Region, length: Length, kind: QuestKind, level: u8) -> Quest {
        Quest {
            region,
            length,
            kind,
            level,
        }
    }

    pub fn label(self) -> String {
        match self.kind {
            QuestKind::Explore => "Explore 90% of the rooms".to_owned(),
            QuestKind::Cleanse => "Cleanse every battle room".to_owned(),
            QuestKind::Gather => format!(
                "Recover {} {}",
                gather_target(self.length),
                self.region.delve_name()
            ),
            QuestKind::Boss => format!("Slay {}", self.region.boss_name()),
        }
    }

    pub fn title(self) -> String {
        format!(
            "{} {} — {}",
            level_name(self.level),
            self.length.name(),
            self.region.name()
        )
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
    /// Quest curios recovered so far.
    pub gathered: u32,
}

impl Dungeon {
    pub fn generate(rng: &mut Rng, quest: Quest) -> Dungeon {
        let rooms = quest.length.rooms();
        let region = quest.region;
        let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        let mut grid: Vec<(i32, i32)> = vec![(0, 0)];
        let mut edges: Vec<(usize, usize)> = Vec::new();
        // Grow outward, favouring the newest rooms so the map reaches out rather than clumping.
        while grid.len() < rooms {
            let from = if rng.percent(60) {
                grid.len() - 1 - rng.index(grid.len().min(3))
            } else {
                rng.index(grid.len())
            };
            let (dx, dy) = *rng.pick(&dirs).expect("dirs");
            let cell = (grid[from].0 + dx, grid[from].1 + dy);
            if grid.contains(&cell) {
                continue;
            }
            grid.push(cell);
            edges.push((from, grid.len() - 1));
        }
        // A few loops, so the map is not a pure tree.
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
        let far = farthest(grid.len(), &edges, 0);
        let pick_curio = |rng: &mut Rng| (*rng.pick(region.curios()).expect("curios")).to_owned();
        let mut room_list = Vec::new();
        for (i, cell) in grid.iter().enumerate() {
            let kind = if i == 0 {
                RoomKind::Entrance
            } else if quest.kind == QuestKind::Boss && i == far {
                RoomKind::Boss(region.boss_pack().iter().map(|s| (*s).to_owned()).collect())
            } else {
                match rng.range(0, 99) {
                    0..=47 => RoomKind::Battle(pack(rng, region.packs())),
                    48..=74 => RoomKind::Curio(pick_curio(rng)),
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
        // A gather quest seeds its quest curios into the rooms, one spare.
        if quest.kind == QuestKind::Gather {
            let mut spots: Vec<usize> = (1..room_list.len())
                .filter(|&i| !matches!(room_list[i].kind, RoomKind::Boss(_)))
                .collect();
            rng.shuffle(&mut spots);
            for &i in spots.iter().take(gather_target(quest.length) as usize + 1) {
                room_list[i].kind = RoomKind::Curio(region.quest_curio().to_owned());
            }
        }
        // A cleanse quest needs battles worth cleansing.
        if quest.kind == QuestKind::Cleanse
            && !room_list
                .iter()
                .any(|r| matches!(r.kind, RoomKind::Battle(_)))
        {
            room_list[far].kind = RoomKind::Battle(pack(rng, region.packs()));
        }
        let halls = edges
            .into_iter()
            .map(|(a, b)| {
                let len = rng.range(3, 5) as usize;
                let segs: Vec<Seg> = (0..len)
                    .map(|_| match rng.range(0, 99) {
                        0..=12 => Seg::Battle(pack(rng, region.hall_packs())),
                        13..=22 => Seg::Trap,
                        23..=35 => Seg::Curio(pick_curio(rng)),
                        36..=40 => Seg::Obstacle,
                        _ => Seg::Empty,
                    })
                    .collect();
                Hall {
                    a,
                    b,
                    segs,
                    visited: vec![false; len],
                    scouted: vec![false; len],
                }
            })
            .collect();
        Dungeon {
            name: region.name().to_owned(),
            rooms: room_list,
            halls,
            at: Location::Room(0),
            quest,
            steps: 0,
            gathered: 0,
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

    /// Segment index along a hall as walked toward `toward`.
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

    /// Room battles won, of all there are.
    pub fn cleansed(&self) -> (usize, usize) {
        let battles: Vec<&Room> = self
            .rooms
            .iter()
            .filter(|r| matches!(r.kind, RoomKind::Battle(_) | RoomKind::Boss(_)))
            .collect();
        (battles.iter().filter(|r| r.cleared).count(), battles.len())
    }

    pub fn progress(&self) -> String {
        match self.quest.kind {
            QuestKind::Explore => format!("explored {}%", self.explored_pct()),
            QuestKind::Cleanse => {
                let (c, t) = self.cleansed();
                format!("cleansed {c}/{t}")
            }
            QuestKind::Gather => format!(
                "recovered {}/{}",
                self.gathered,
                gather_target(self.quest.length)
            ),
            QuestKind::Boss => {
                if self.quest_done() {
                    "the lair is silent".to_owned()
                } else {
                    format!("explored {}%", self.explored_pct())
                }
            }
        }
    }

    pub fn quest_done(&self) -> bool {
        match self.quest.kind {
            QuestKind::Explore => self.explored_pct() >= 90,
            QuestKind::Cleanse => {
                let (c, t) = self.cleansed();
                c >= t
            }
            QuestKind::Gather => self.gathered >= gather_target(self.quest.length),
            QuestKind::Boss => self
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

    /// A scout's look down every hallway leaving `room`.
    pub fn scout_from(&mut self, room: usize) {
        for (h, other) in self.exits(room) {
            self.halls[h].scouted.fill(true);
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
    Trinket(&'static str),
    HeroStress { hero: usize, amount: i32 },
    HeroHurt { hero: usize, amount: i32 },
    HeroHeal { hero: usize, amount: i32 },
    Afflicted { hero: usize, name: &'static str },
    Virtuous { hero: usize, name: &'static str },
    Light(i32),
    Died { hero: usize },
    Scouted,
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
    /// Trinkets found, by id.
    #[serde(default)]
    pub trinkets: Vec<String>,
    /// Temporary curio buffs, applied to the next battle.
    pub buffs: Vec<(Stat, i32)>,
    pub rng: Rng,
    pub battles_won: u32,
    #[serde(default)]
    pub firewood: u8,
}

/// What a night at camp came to.
#[derive(Clone, Debug, Default)]
pub struct CampReport {
    pub notes: Vec<Note>,
    /// The night was interrupted: these monsters attack.
    pub ambush: Option<Vec<String>>,
}

impl Expedition {
    pub fn new(party: Vec<Hero>, dungeon: Dungeon, pack: HashMap<Item, i32>, rng: Rng) -> Self {
        let firewood = dungeon.quest.length.firewood();
        Expedition {
            party,
            fallen: Vec::new(),
            dungeon,
            light: TORCH_MAX,
            pack,
            gold: 0,
            heirlooms: HashMap::new(),
            trinkets: Vec::new(),
            buffs: Vec::new(),
            rng,
            battles_won: 0,
            firewood,
        }
    }

    pub fn level(&self) -> u8 {
        self.dungeon.quest.level
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

    fn heal_hero(&mut self, hero: usize, pct: i32, notes: &mut Vec<Note>) {
        if let Some(h) = self.party.get_mut(hero) {
            let amt = (h.max_hp() * pct / 100)
                .max(1)
                .min(h.max_hp() - h.hp)
                .max(0);
            h.hp += amt;
            notes.push(Note::HeroHeal { hero, amount: amt });
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
        if self.light < 50 && !self.party.is_empty() && self.rng.percent(30) {
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
        let first = !self.dungeon.rooms[room].visited;
        let r = &mut self.dungeon.rooms[room];
        r.visited = true;
        r.scouted = true;
        self.dungeon.reveal_around(room);
        // A good light lets the party read the halls ahead.
        if first && self.rng.percent(15 + self.light / 4) {
            self.dungeon.scout_from(room);
            notes.push(Note::Scouted);
        }
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
            let line = match self.dungeon.quest.region {
                Region::Crypts => {
                    "The air turns to ice. Something ancient looks up from its reading."
                }
                Region::Warrens => "The walls are silk. The silk is moving.",
                Region::Grotto => "The water rises, and something vast rises with it.",
            };
            notes.push(Note::Text(line.into()));
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

    /// The encounter where the party stands.
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

    /// Whether the room the party stands in is safe to camp in.
    pub fn can_camp(&self) -> bool {
        self.firewood > 0
            && matches!(self.dungeon.at, Location::Room(r) if self.dungeon.rooms[r].cleared)
    }

    fn hunger(&mut self, notes: &mut Vec<Note>) {
        let need = self.party.len() as i32;
        if self.count(Item::Food) >= need {
            *self.pack.get_mut(&Item::Food).expect("food") -= need;
            notes.push(Note::Text("The party eats. A small mercy.".into()));
            notes.push(Note::Item(Item::Food, -need));
            for h in &mut self.party {
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
                self.heal_hero(hero, 20, &mut notes);
            }
            Item::Herbs => {
                self.take(item);
                self.stress_hero(hero, -10, &mut notes);
            }
            Item::Food => {
                self.take(item);
                self.heal_hero(hero, 10, &mut notes);
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
                    let amt = self.rng.range(3, 6) + i32::from(self.level());
                    self.hurt_hero(lead, amt, &mut notes);
                }
                1 => {
                    let line = match self.dungeon.quest.region {
                        Region::Crypts => "Grave gas seeps up through the flagstones.",
                        Region::Warrens => "A pocket of foul mine-damp bursts underfoot.",
                        Region::Grotto => "A rush of black water knocks the party off its feet.",
                    };
                    notes.push(Note::Text(line.into()));
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

    fn give_trinket(&mut self, min: Rarity, notes: &mut Vec<Note>) {
        let level = self.level();
        let t = if min == Rarity::Common {
            roll_trinket(&mut self.rng, level)
        } else {
            roll_trinket_at_least(&mut self.rng, min)
        };
        self.trinkets.push(t.id.to_owned());
        notes.push(Note::Trinket(t.id));
    }

    fn apply(&mut self, o: Outcome, hero: usize, notes: &mut Vec<Note>) {
        match o {
            Outcome::Gold(lo, hi) => {
                let lvl = i32::from(self.level());
                let g = self.rng.range(lo, hi) * (100 + self.dark_bonus()) / 100 * (2 + lvl) / 3;
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
                for i in 0..self.party.len() {
                    self.heal_hero(i, pct, notes);
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
            Outcome::Trinket(min) => self.give_trinket(min, notes),
            Outcome::QuestItem => {
                self.dungeon.gathered += 1;
                notes.push(Note::Text(format!(
                    "Recovered: {} of {} {}.",
                    self.dungeon.gathered,
                    gather_target(self.dungeon.quest.length),
                    self.dungeon.quest.region.delve_name()
                )));
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
        let lvl = i32::from(self.level());
        let base = if boss {
            self.rng.range(1500, 2500)
        } else {
            self.rng.range(120, 380)
        };
        let g = base * (100 + self.dark_bonus()) / 100 * (2 + lvl) / 3;
        self.gold += g;
        notes.push(Note::Gold(g));
        if boss || self.rng.percent(30) {
            let kind = *self.rng.pick(&Heirloom::ALL).expect("heirlooms");
            let n = if boss { 6 } else { self.rng.range(1, 2) } * lvl;
            *self.heirlooms.entry(kind).or_insert(0) += n;
            notes.push(Note::Heirloom(kind, n));
        }
        if boss {
            let id = match self.dungeon.quest.region {
                Region::Crypts => "archivist_quill",
                Region::Warrens => "broodmother_eye",
                Region::Grotto => "matriarch_scale",
            };
            if trinket(id).is_some() {
                self.trinkets.push(id.to_owned());
                notes.push(Note::Trinket(id));
            }
        } else if self.rng.percent(8 + 4 * lvl + self.dark_bonus() / 5) {
            self.give_trinket(Rarity::Common, &mut notes);
        }
        if self.rng.percent(25) {
            self.give(Item::Food, 1);
            notes.push(Note::Item(Item::Food, 1));
        }
        notes
    }

    /// Make camp: eat, then spend the twelve hours on the chosen (hero, skill, target) uses. A
    /// target of `None` means the user, or the party for a party skill.
    pub fn camp(&mut self, uses: &[(usize, &'static CampSkill, Option<usize>)]) -> CampReport {
        let mut r = CampReport::default();
        if self.firewood == 0 || self.party.is_empty() {
            return r;
        }
        self.firewood -= 1;
        let need = self.party.len() as i32;
        if self.count(Item::Food) >= need {
            *self.pack.get_mut(&Item::Food).expect("food") -= need;
            r.notes.push(Note::Text(
                "A hot meal by the fire. For a moment, it is almost home.".into(),
            ));
            for i in 0..self.party.len() {
                self.heal_hero(i, 15, &mut r.notes);
                self.stress_hero(i, -10, &mut r.notes);
            }
        } else {
            r.notes.push(Note::Text(
                "Not enough food. The party goes to sleep hungry.".into(),
            ));
            for i in 0..self.party.len() {
                self.stress_hero(i, 10, &mut r.notes);
            }
        }
        let mut ward = 0;
        for &(user, skill, target) in uses {
            if user >= self.party.len() {
                continue;
            }
            let targets: Vec<usize> = match skill.target {
                CampTarget::Myself => vec![user],
                CampTarget::Ally => vec![target.unwrap_or(user).min(self.party.len() - 1)],
                CampTarget::Party => (0..self.party.len()).collect(),
            };
            r.notes.push(Note::Text(format!(
                "{} uses {}.",
                self.party[user].name, skill.name
            )));
            for e in skill.effects {
                match *e {
                    CampEffect::Heal(p) => {
                        for &t in &targets {
                            self.heal_hero(t, p, &mut r.notes);
                        }
                    }
                    CampEffect::StressHeal(n) => {
                        for &t in &targets {
                            self.stress_hero(t, -n, &mut r.notes);
                        }
                    }
                    CampEffect::Buff(stat, amt) => {
                        for &t in &targets {
                            if let Some(h) = self.party.get_mut(t) {
                                h.camp_buffs.push((stat, amt));
                            }
                        }
                    }
                    CampEffect::Cure => {}
                    CampEffect::Ward(w) => ward += w,
                }
            }
            match skill.id {
                "wl_ritual" => self.stress_hero(user, 10, &mut r.notes),
                "r_gamble" => self.stress_hero(user, -20, &mut r.notes),
                "w_war_stories" => {
                    if let Some(h) = self.party.get_mut(user) {
                        h.camp_buffs.push((Stat::Dmg, 10));
                    }
                }
                _ => {}
            }
        }
        // The night: something may come for the sleepers.
        let chance = (30 - ward - self.light / 10).max(3);
        if self.rng.percent(chance) {
            r.notes.push(Note::Text(
                "Shapes in the dark beyond the fire. Ambush!".into(),
            ));
            r.ambush = Some(pack(&mut self.rng, self.dungeon.quest.region.packs()));
            for i in 0..self.party.len() {
                self.stress_hero(i, 8, &mut r.notes);
            }
        } else {
            r.notes.push(Note::Text(
                "The night passes without incident. The fire burns low.".into(),
            ));
            self.light = TORCH_MAX.min(self.light + 20);
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::super::camp::known;
    use super::super::data::HeroClass;
    use super::*;

    fn quest(kind: QuestKind, length: Length) -> Quest {
        Quest::new(Region::Crypts, length, kind, 1)
    }

    #[test]
    fn maps_are_connected_with_a_lair() {
        for seed in 0..30 {
            let mut rng = Rng::new(seed);
            for region in Region::ALL {
                let d = Dungeon::generate(
                    &mut rng,
                    Quest::new(region, Length::Medium, QuestKind::Boss, 2),
                );
                assert_eq!(d.rooms.len(), 14);
                assert!(d.halls.len() >= 13);
                assert_eq!(
                    d.rooms
                        .iter()
                        .filter(|r| matches!(r.kind, RoomKind::Boss(_)))
                        .count(),
                    1
                );
                assert!(d.halls.iter().all(|h| (3..=5).contains(&h.segs.len())));
                for r in &d.rooms {
                    match &r.kind {
                        RoomKind::Battle(p) | RoomKind::Boss(p) => {
                            assert!(!monsters_of(p).is_empty())
                        }
                        RoomKind::Curio(c) => assert!(curio(c).is_some(), "{c}"),
                        _ => {}
                    }
                }
            }
        }
    }

    #[test]
    fn gather_quests_seed_enough_quest_curios() {
        for seed in 0..20 {
            let mut rng = Rng::new(seed);
            let d = Dungeon::generate(&mut rng, quest(QuestKind::Gather, Length::Short));
            let n = d
                .rooms
                .iter()
                .filter(|r| r.kind == RoomKind::Curio("reliquary".into()))
                .count();
            assert!(n as u32 >= gather_target(Length::Short));
        }
    }

    #[test]
    fn walking_a_hall_spends_light_and_arrives() {
        let mut rng = Rng::new(5);
        let d = Dungeon::generate(&mut rng, quest(QuestKind::Explore, Length::Short));
        let party = vec![Hero::new(0, HeroClass::Warrior, "A".into(), &mut rng)];
        let mut e = Expedition::new(party, d, HashMap::new(), Rng::new(9));
        let (hall, to) = e.dungeon.exits(0)[0];
        let len = e.dungeon.halls[hall].segs.len();
        assert!(e.enter_hall(hall));
        for _ in 0..len {
            e.advance();
        }
        assert!(e.light < TORCH_MAX);
        e.advance();
        assert_eq!(e.dungeon.at, Location::Room(to));
        assert!(e.dungeon.rooms[to].visited);
    }

    #[test]
    fn camping_spends_firewood_and_buffs() {
        let mut rng = Rng::new(2);
        let d = Dungeon::generate(&mut rng, quest(QuestKind::Explore, Length::Long));
        let party = vec![
            Hero::new(0, HeroClass::Rogue, "A".into(), &mut rng),
            Hero::new(1, HeroClass::Mage, "B".into(), &mut rng),
        ];
        let mut e = Expedition::new(party, d, HashMap::from([(Item::Food, 8)]), Rng::new(3));
        assert_eq!(e.firewood, 2);
        assert!(e.can_camp());
        let study = known(HeroClass::Mage)
            .into_iter()
            .find(|s| s.id == "m_study")
            .unwrap();
        e.camp(&[(1, study, None)]);
        assert_eq!(e.firewood, 1);
        assert!(!e.party[1].camp_buffs.is_empty());
    }
}
