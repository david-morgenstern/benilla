# Darkened Deeps

A turn-based, stress-driven dungeon crawler in the mould of the genre's gothic classics, built on
benilla's asset stack. Every model, texture, icon, font, song and sound is read at runtime from
your own World of Warcraft 1.12.1 install; nothing from the game ships in this repository.

```sh
WOW_DATA=/path/to/WoW/Data cargo run --release -p benilla-crawl
```

The campaign saves to `benilla-config/darkened-deeps.json` at the workspace root (beside the
binary in a packaged build). The install is only read.

## Playing

**The hamlet** is a moonlit Darkshire square. Click a building's sign to enter it, a hero in the
roster for their character sheet, and the estate on the hill (or *Embark*) to choose an
expedition.

| Building | What it does | Rebuilt with heirlooms |
| --- | --- | --- |
| Stagecoach | Recruits, refreshed weekly | More recruits, bigger roster, veterans |
| The Drowned Lantern / Chapel | Stress relief; breaks afflictions; the hero rests a week | Cheaper, stronger, more places |
| Sanitarium | Cuts out negative quirks | Cheaper |
| Blacksmith | Weapon and armour tiers | Higher tier cap |
| Guild Hall | Skill ranks I–V, choose equipped skills, talents | Higher rank cap |
| Nomad Wagon | Buys and sells trinkets | More, rarer stock, discounts |
| Memorial | The names of the dead | |

Each week brings a town event (a discount, a closure, a patron, a festival). Heroes earn resolve
levels; resolve 1, 3 and 5 each open a talent tier. Veterans refuse apprentice-level work.

**The world map** offers expeditions across three regions (the Sunken Crypts, the Kobold
Warrens, the Drowned Grotto) at three levels and three lengths, with four quest kinds: explore,
cleanse, gather and, after two runs in a region, its lair boss. Medium and long expeditions carry
firewood for camping.

**In the dungeon**, click a room on the map to walk its hallway (Space toggles walking; hold D or
the right arrow). The torch burns down as you go: darkness raises stress and danger, and the
loot. Curios reward the right item; rubble wants a shovel; hunger wants rations. Camp in a cleared
room to eat and spend twelve hours of camp skills, and pray the night stays quiet.

**In battle**, the hero whose turn it is takes orders: 1–4 or click a skill, then click a lit
target (hover shows hit, crit and damage); M or the boot icon moves. Skills only work from certain
ranks against certain ranks. Heroes at zero HP stand at Death's Door; stress at 100 tests resolve
(affliction or virtue) and at 200 stops the heart. Death is permanent.

## Development switches

| Variable | Effect |
| --- | --- |
| `CRAWL_AUTOSTART=hamlet\|map\|dungeon\|camp\|battle\|boss\|warrens\|grotto` | Skip the title with a seeded campaign (never saved) |
| `CRAWL_AUTOPLAY=1` | Play unattended: town, map, exploration and battle |
| `CRAWL_RICH=1` | With autostart: gold, heirlooms, worn trinkets and some resolve |
| `CRAWL_MUTE=1` | No audio |
| `CRAWL_SHOT=<png>`, `CRAWL_SHOT_AT=<s>[,<s>…]` | Screenshots at those times, then exit |

The rules (`src/rules/`) are plain Rust with unit tests: `cargo test -p benilla-crawl`. With
`WOW_DATA` set, a test also decodes every track and sound the game names off the install.
