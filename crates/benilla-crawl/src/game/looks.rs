//! What a hero looks like: their seed picks a race, sex, face and hair fitting their class, and
//! the class dresses them in its dungeon set, all from the install's own tables.

use bevy::math::Rect;

use crate::rules::data::HeroClass;
use crate::rules::hero::Hero;
use crate::rules::rng::Rng;
use crate::wow::{CharLook, WowData};

const HUMAN: u8 = 1;
const ORC: u8 = 2;
const DWARF: u8 = 3;
const NIGHTELF: u8 = 4;
const UNDEAD: u8 = 5;
const TAUREN: u8 = 6;
const GNOME: u8 = 7;
const TROLL: u8 = 8;

struct Outfit {
    races: &'static [u8],
    /// Shirt, chest, belt, pants, boots, wrist, gloves, tabard.
    body: [u32; 8],
    helm: u32,
    shoulders: u32,
    main_hand: u32,
    off_hand: u32,
    shield: bool,
}

fn outfit(class: HeroClass) -> Outfit {
    match class {
        HeroClass::Warrior => Outfit {
            races: &[HUMAN, DWARF, ORC, UNDEAD, NIGHTELF, TROLL],
            body: [0, 29958, 29959, 29963, 29960, 29961, 29962, 0],
            helm: 0,
            shoulders: 29964,
            main_hand: 1621,
            off_hand: 0,
            shield: false,
        },
        HeroClass::Paladin => Outfit {
            races: &[HUMAN, DWARF],
            body: [0, 29969, 29966, 29972, 29967, 29968, 29970, 0],
            helm: 0,
            shoulders: 29971,
            main_hand: 1161,
            off_hand: 1705,
            shield: true,
        },
        HeroClass::Rogue => Outfit {
            races: &[HUMAN, NIGHTELF, UNDEAD, GNOME, ORC, DWARF, TROLL],
            body: [0, 31105, 31339, 31340, 31109, 31106, 31107, 0],
            helm: 0,
            shoulders: 31343,
            main_hand: 2635,
            off_hand: 2669,
            shield: false,
        },
        HeroClass::Priest => Outfit {
            races: &[HUMAN, DWARF, NIGHTELF, UNDEAD, TROLL],
            body: [0, 30422, 30425, 30423, 30429, 30426, 30427, 0],
            helm: 0,
            shoulders: 31103,
            main_hand: 1469,
            off_hand: 0,
            shield: false,
        },
        HeroClass::Mage => Outfit {
            races: &[HUMAN, GNOME, UNDEAD, TROLL],
            body: [0, 29591, 29596, 0, 29594, 29595, 29593, 0],
            helm: 0,
            shoulders: 29592,
            main_hand: 1376,
            off_hand: 0,
            shield: false,
        },
        HeroClass::Hunter => Outfit {
            races: &[NIGHTELF, DWARF, ORC, TROLL, TAUREN],
            body: [0, 31402, 31404, 31403, 31407, 31405, 31406, 0],
            helm: 0,
            shoulders: 31409,
            main_hand: 0,
            off_hand: 30927,
            shield: false,
        },
        HeroClass::Warlock => Outfit {
            races: &[HUMAN, UNDEAD, GNOME, ORC],
            body: [0, 29792, 29793, 29797, 29799, 29795, 29800, 0],
            helm: 0,
            shoulders: 29798,
            main_hand: 1190,
            off_hand: 0,
            shield: false,
        },
    }
}

/// How many consecutive values from 0 a CharSections probe accepts.
fn count(mut probe: impl FnMut(u8) -> bool) -> u8 {
    let mut n = 0;
    while n < 32 && probe(n) {
        n += 1;
    }
    n.max(1)
}

pub fn hero_look(data: &WowData, hero: &Hero) -> CharLook {
    let o = outfit(hero.class);
    let mut rng = Rng::new(u64::from(hero.look_seed));
    let race = *rng.pick(o.races).expect("races");
    let sex = u8::from(rng.percent(40));
    let skins = count(|s| data.sections.skin_texture(race, sex, s).is_some());
    let skin = rng.index(usize::from(skins)) as u8;
    let styles = count(|s| data.sections.hair_texture(race, sex, s, 0).is_some());
    let hair_style = rng.index(usize::from(styles)) as u8;
    let colors = count(|c| {
        data.sections
            .hair_texture(race, sex, hair_style, c)
            .is_some()
    });
    let hair_color = rng.index(usize::from(colors)) as u8;
    CharLook {
        race,
        sex,
        skin,
        face: rng.range(0, 4) as u8,
        hair_style,
        hair_color,
        facial_hair: if sex == 0 { rng.range(0, 4) as u8 } else { 0 },
        body: o.body,
        helm: o.helm,
        shoulders: o.shoulders,
        main_hand: o.main_hand,
        off_hand: o.off_hand,
        shield: o.shield,
    }
}

/// On-stage height for a hero, by race.
pub fn hero_height(look: &CharLook) -> f32 {
    match look.race {
        GNOME => 1.15,
        DWARF => 1.45,
        TAUREN => 2.25,
        TROLL => 2.0,
        UNDEAD => 1.75,
        NIGHTELF => 2.05,
        _ => 1.85,
    }
}

pub const CLASS_ATLAS: &str = "Interface\\Glues\\CharacterCreate\\UI-CharacterCreate-Classes";

/// The class's cell in the character-create class atlas (256² pixels).
pub fn class_icon_rect(class: HeroClass) -> Rect {
    let (u0, u1, v0, v1) = match class {
        HeroClass::Warrior => (0.0, 0.25, 0.0, 0.25),
        HeroClass::Mage => (0.25, 0.496, 0.0, 0.25),
        HeroClass::Rogue => (0.496, 0.742, 0.0, 0.25),
        HeroClass::Hunter => (0.0, 0.25, 0.25, 0.5),
        HeroClass::Priest => (0.496, 0.742, 0.25, 0.5),
        HeroClass::Warlock => (0.742, 0.988, 0.25, 0.5),
        HeroClass::Paladin => (0.0, 0.25, 0.5, 0.75),
    };
    Rect::new(u0 * 256.0, v0 * 256.0, u1 * 256.0, v1 * 256.0)
}
