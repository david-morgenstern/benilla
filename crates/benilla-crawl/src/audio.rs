//! Sound: kira mixing on a minimal cpal output, WoW's own music and effects decoded straight off
//! the player's install. `CRAWL_MUTE=1` runs silent.

use std::collections::HashMap;
use std::io::Cursor;
use std::time::Duration;

use bevy::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use kira::backend::{Backend, Renderer};
use kira::sound::static_sound::StaticSoundData;
use kira::sound::streaming::{StreamingSoundData, StreamingSoundHandle};
use kira::sound::FromFileError;
use kira::{AudioManager, AudioManagerSettings, Decibels, Tween};

use crate::rules::regions::Region;
use crate::wow::WowData;

/// kira's output on the default cpal device; the stream lives as long as the backend.
pub struct CpalOut {
    device: cpal::Device,
    config: cpal::StreamConfig,
    stream: Option<cpal::Stream>,
}

impl Backend for CpalOut {
    type Settings = ();
    type Error = anyhow::Error;

    fn setup(_: (), _internal_buffer_size: usize) -> Result<(Self, u32), Self::Error> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("no default audio output"))?;
        let config: cpal::StreamConfig = device.default_output_config()?.into();
        let rate = config.sample_rate;
        Ok((
            CpalOut {
                device,
                config,
                stream: None,
            },
            rate,
        ))
    }

    fn start(&mut self, mut renderer: Renderer) -> Result<(), Self::Error> {
        let channels = self.config.channels;
        let stream = self.device.build_output_stream(
            &self.config,
            move |data: &mut [f32], _| {
                renderer.on_start_processing();
                renderer.process(data, channels);
            },
            |e| warn!("crawl: audio stream error: {e}"),
            None,
        )?;
        stream.play()?;
        self.stream = Some(stream);
        Ok(())
    }
}

/// The track a screen wants playing.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Music {
    #[default]
    None,
    Title,
    Hamlet,
    Crypt,
    Battle,
    Boss,
    Explore(crate::rules::regions::Region),
    Fight(crate::rules::regions::Region),
    Lair(crate::rules::regions::Region),
}

impl Music {
    fn tracks(self) -> &'static [&'static str] {
        match self {
            Music::None => &[],
            Music::Title => &["Sound\\Music\\ZoneMusic\\GhostMusic\\GhostMusic03.mp3"],
            Music::Hamlet => &[
                "Sound\\Music\\ZoneMusic\\EvilForest\\DayEvilForest01.mp3",
                "Sound\\Music\\ZoneMusic\\EvilForest\\DayEvilForest02.mp3",
                "Sound\\Music\\ZoneMusic\\EvilForest\\DayEvilForest03.mp3",
            ],
            Music::Crypt => &[
                "Sound\\Music\\ZoneMusic\\CursedLand\\CursedLand01.mp3",
                "Sound\\Music\\ZoneMusic\\CursedLand\\CursedLand02.mp3",
                "Sound\\Music\\ZoneMusic\\CursedLand\\CursedLand03.mp3",
                "Sound\\Music\\Musical Moments\\haunted\\haunted01.mp3",
                "Sound\\Music\\ZoneMusic\\CursedLand\\CursedLand04.mp3",
                "Sound\\Music\\Musical Moments\\haunted\\haunted02.mp3",
            ],
            Music::Battle => &[
                "Sound\\Music\\ZoneMusic\\Naxxramas\\NaxxramasPlagueWing1.mp3",
                "Sound\\Music\\ZoneMusic\\Naxxramas\\NaxxramasPlagueWing2.mp3",
                "Sound\\Music\\ZoneMusic\\Naxxramas\\NaxxramasPlagueWing3.mp3",
            ],
            Music::Boss => &["Sound\\Music\\ZoneMusic\\Naxxramas\\NaxxramasPlagueBoss1.mp3"],
            Music::Explore(r) => match r {
                Region::Crypts => Music::Crypt.tracks(),
                Region::Warrens => &[
                    "Sound\\Music\\CityMusic\\Gnomeragon\\gnomeragon01-zone.mp3",
                    "Sound\\Music\\Musical Moments\\mystery\\mystery03-zone.mp3",
                    "Sound\\Music\\CityMusic\\Gnomeragon\\gnomeragon02-zone.mp3",
                    "Sound\\Music\\Musical Moments\\mystery\\mystery05-zone.mp3",
                ],
                Region::Grotto => &[
                    "Sound\\Music\\Musical Moments\\swamp\\swamp01.mp3",
                    "Sound\\Music\\Musical Moments\\gloomy\\gloomy01.mp3",
                    "Sound\\Music\\Musical Moments\\mystery\\mystery07-zone.mp3",
                    "Sound\\Music\\Musical Moments\\gloomy\\gloomy02.mp3",
                ],
            },
            Music::Fight(r) => match r {
                Region::Crypts => Music::Battle.tracks(),
                Region::Warrens => &[
                    "Sound\\Music\\ZoneMusic\\Naxxramas\\NaxxramasAbominationWing1.mp3",
                    "Sound\\Music\\ZoneMusic\\Naxxramas\\NaxxramasAbominationWing2.mp3",
                    "Sound\\Music\\ZoneMusic\\Naxxramas\\NaxxramasAbominationWing3.mp3",
                ],
                Region::Grotto => &[
                    "Sound\\Music\\ZoneMusic\\Naxxramas\\NaxxramasFrostwyrm1.mp3",
                    "Sound\\Music\\ZoneMusic\\Naxxramas\\NaxxramasFrostwyrm2.mp3",
                ],
            },
            Music::Lair(r) => match r {
                Region::Crypts => Music::Boss.tracks(),
                Region::Warrens => {
                    &["Sound\\Music\\ZoneMusic\\Naxxramas\\NaxxramasSpiderBoss1.mp3"]
                }
                Region::Grotto => &["Sound\\Music\\ZoneMusic\\Naxxramas\\NaxxramasKelThuzad1.mp3"],
            },
        }
    }
}

/// One-shot effects to play this frame: (install path, volume in dB).
#[derive(Resource, Default)]
pub struct Sfx(pub Vec<(String, f32)>);

impl Sfx {
    pub fn play(&mut self, path: impl Into<String>, db: f32) {
        self.0.push((path.into(), db));
    }
}

/// The mixer and what it is doing, main-thread only (the cpal stream is not `Send` everywhere).
struct Mixer {
    manager: AudioManager<CpalOut>,
    playing: Music,
    track: usize,
    handle: Option<StreamingSoundHandle<FromFileError>>,
    cache: HashMap<String, Option<StaticSoundData>>,
}

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Music>().init_resource::<Sfx>();
        if std::env::var("CRAWL_MUTE").is_ok() {
            app.add_systems(Update, |mut s: ResMut<Sfx>| s.0.clear());
            return;
        }
        match AudioManager::<CpalOut>::new(AudioManagerSettings::default()) {
            Ok(manager) => {
                app.insert_non_send_resource(Mixer {
                    manager,
                    playing: Music::None,
                    track: 0,
                    handle: None,
                    cache: HashMap::new(),
                });
                app.add_systems(Update, (music, effects));
            }
            Err(e) => {
                warn!("crawl: no audio ({e:#}); playing silent");
                app.add_systems(Update, |mut s: ResMut<Sfx>| s.0.clear());
            }
        }
    }
}

fn read(data: &WowData, path: &str) -> Option<Vec<u8>> {
    data.chain.lock().ok()?.read_file(path).ok()
}

fn fade(secs: f32) -> Tween {
    Tween {
        duration: Duration::from_secs_f32(secs),
        ..default()
    }
}

fn music(want: Res<Music>, data: Res<WowData>, mut mixer: NonSendMut<Mixer>) {
    let finished = mixer
        .handle
        .as_ref()
        .is_some_and(|h| h.state() == kira::sound::PlaybackState::Stopped);
    if *want == mixer.playing && !finished {
        return;
    }
    if *want != mixer.playing {
        mixer.track = 0;
    } else {
        mixer.track += 1;
    }
    if let Some(mut h) = mixer.handle.take() {
        h.stop(fade(2.0));
    }
    mixer.playing = *want;
    let tracks = want.tracks();
    if tracks.is_empty() {
        return;
    }
    let path = tracks[mixer.track % tracks.len()];
    let Some(bytes) = read(&data, path) else {
        warn!("crawl: music {path} missing");
        return;
    };
    match StreamingSoundData::from_cursor(Cursor::new(bytes)) {
        Ok(sound) => {
            let sound = sound.volume(Decibels(-9.0)).fade_in_tween(fade(2.5));
            match mixer.manager.play(sound) {
                Ok(h) => mixer.handle = Some(h),
                Err(e) => warn!("crawl: could not play {path}: {e}"),
            }
        }
        Err(e) => warn!("crawl: could not decode {path}: {e}"),
    }
}

fn effects(mut sfx: ResMut<Sfx>, data: Res<WowData>, mut mixer: NonSendMut<Mixer>) {
    for (path, db) in sfx.0.drain(..) {
        let sound = mixer
            .cache
            .entry(path.clone())
            .or_insert_with(|| {
                read(&data, &path).and_then(|b| StaticSoundData::from_cursor(Cursor::new(b)).ok())
            })
            .clone();
        if let Some(s) = sound {
            let _ = mixer.manager.play(s.volume(Decibels(db)));
        }
    }
}

/// The voice a monster makes: its creature folder and file stems.
fn creature_voice(
    monster: &str,
) -> (
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    &'static str,
) {
    match monster {
        "ghoul" => (
            "Sound\\Creature\\Ghoul\\",
            &["mGhoulAttack1", "mGhoulAttack2", "mGhoulAttack3"],
            &["mGhoulWound1", "mGhoulWound2"],
            "mGhoulDeath1",
        ),
        "banshee" => (
            "Sound\\Creature\\Banshee\\",
            &["BansheeAttackA", "BansheeAttackB", "BansheeAttackC"],
            &["BansheeWoundA", "BansheeWoundB"],
            "BansheeDeathA",
        ),
        "necrolyte" => (
            "Sound\\Creature\\SkeletonMage\\",
            &[
                "SkeletonMageAttackA",
                "SkeletonMageAttackB",
                "SkeletonMageAttackC",
            ],
            &["SkeletonMageWoundA", "SkeletonMageWoundB"],
            "SkeletonMageDeathA",
        ),
        "abomination" => (
            "Sound\\Creature\\FleshGolem\\",
            &[
                "FleshGolemAttackA",
                "FleshGolemAttackB",
                "FleshGolemAttackC",
            ],
            &["FleshGolemWoundA"],
            "FleshGolemDeathA",
        ),
        "lich" => (
            "Sound\\Creature\\Lich\\",
            &["LichAttackA", "LichAttackB", "LichAttackC"],
            &["LichWoundA", "LichWoundB"],
            "LichDeath",
        ),
        "kobold_digger" | "kobold_geomancer" | "kobold_taskmaster" => (
            "Sound\\Creature\\Kobold\\",
            &["mKoboldAttackA", "mKoboldAttackB", "mKoboldAttackC"],
            &["mKoboldWoundA", "mKoboldWoundB"],
            "mKoboldDeathA",
        ),
        "rat" | "cave_creeper" | "gut_worm" | "broodmother" => (
            "Sound\\Creature\\GiantSpider\\",
            &["GiantSpiderAttackA", "GiantSpiderAttackB"],
            &["GiantSpiderWoundA"],
            "GiantSpiderWoundDeath",
        ),
        "murloc_raider" | "murloc_oracle" => (
            "Sound\\Creature\\Murloc\\",
            &["mMurlocAttackA", "mMurlocAttackB"],
            &["mMurlocWoundA", "mMurlocWoundB"],
            "mMurlocDeath2a",
        ),
        "siren" | "myrmidon" | "matriarch" | "crab" | "lobstrok" => (
            "Sound\\Creature\\Banshee\\",
            &["BansheeAttackA", "BansheeAttackB", "BansheeAttackC"],
            &["BansheeWoundA", "BansheeWoundB"],
            "BansheeDeathA",
        ),
        _ => (
            "Sound\\Creature\\Skeleton\\",
            &[
                "mArmoredSkeletonAttackA",
                "mArmoredSkeletonAttackB",
                "mArmoredSkeletonAttackC",
            ],
            &[
                "mArmoredSkeletonWoundA",
                "mArmoredSkeletonWoundB",
                "mArmoredSkeletonWoundC",
            ],
            "mArmoredSkeletonDeathA",
        ),
    }
}

#[derive(Clone, Copy)]
pub enum Voice {
    Attack,
    Wound,
    Death,
}

pub fn monster_sound(monster: &str, voice: Voice, roll: usize) -> String {
    let (dir, attacks, wounds, death) = creature_voice(monster);
    let stem = match voice {
        Voice::Attack => attacks[roll % attacks.len()],
        Voice::Wound => wounds[roll % wounds.len()],
        Voice::Death => death,
    };
    format!("{dir}{stem}.wav")
}

/// What a hero's skill sounds like as it lands.
pub fn skill_sound(
    skill_id: &str,
    kind: crate::rules::data::SkillKind,
    crit: bool,
    roll: usize,
) -> String {
    use crate::rules::data::SkillKind;
    let holy = [
        "holy_strike",
        "judgement",
        "hammer_justice",
        "consecration",
        "smite",
        "holy_light",
        "heal",
        "prayer_healing",
        "pw_shield",
        "devotion",
    ];
    let fire = ["fireball"];
    let frost = ["frostbolt", "frost_nova", "frost_tomb"];
    let shadow = [
        "corruption",
        "shadow_bolt",
        "drain_life",
        "fear",
        "curse_weakness",
        "life_tap",
        "psychic_scream",
        "shadow_word",
        "whispers",
        "death_bolt",
        "dread_gaze",
        "forbidden_page",
        "dirge",
        "wail",
    ];
    if holy.contains(&skill_id) {
        return if kind == SkillKind::Heal {
            "Sound\\Spells\\Heal_Low_Base.wav".into()
        } else {
            "Sound\\Spells\\DirectDamage\\HolyImpactDDMedium.wav".into()
        };
    }
    if fire.contains(&skill_id) {
        return "Sound\\Spells\\Cast\\FireCast.wav".into();
    }
    if frost.contains(&skill_id) {
        return "Sound\\Spells\\Cast\\IceCast.wav".into();
    }
    if shadow.contains(&skill_id) {
        return "Sound\\Spells\\Cast\\ShadowCast.wav".into();
    }
    match kind {
        SkillKind::Heal => "Sound\\Spells\\Heal_Low_Base.wav".into(),
        SkillKind::Spell | SkillKind::Support => "Sound\\Spells\\Cast\\NatureCast.wav".into(),
        SkillKind::Ranged => format!(
            "Sound\\Item\\Weapons\\Bow\\ArrowHit{}.wav",
            ["A", "B", "C"][roll % 3]
        ),
        SkillKind::Melee => {
            if crit {
                "Sound\\Item\\Weapons\\Axe2H\\m2hAxeHitFleshCrit.wav".into()
            } else {
                format!(
                    "Sound\\Item\\Weapons\\Axe2H\\m2hAxeHitFlesh1{}.wav",
                    ["a", "b", "c"][roll % 3]
                )
            }
        }
    }
}

pub const MISS: &str = "Sound\\Item\\Weapons\\MissSwings\\MissWhoosh1Handed.wav";
pub const COIN: &str = "Sound\\interface\\LootCoinLarge.wav";
pub const AFFLICTION: &str = "Sound\\interface\\RaidWarning.wav";
pub const VIRTUE: &str = "Sound\\interface\\LevelUp.wav";
pub const CLICK: &str = "Sound\\interface\\MagicClick.wav";

#[cfg(test)]
mod tests {
    use super::*;

    /// The output opens on the default device; it plays nothing, only silence. Needs a sound
    /// device, so it runs on request: `cargo test -p benilla-crawl -- --ignored`.
    #[test]
    #[ignore = "opens the audio device"]
    fn output_opens() {
        let m = AudioManager::<CpalOut>::new(AudioManagerSettings::default());
        assert!(m.is_ok(), "{:?}", m.err());
        std::thread::sleep(Duration::from_millis(300));
    }

    /// Every track and cue the game names decodes off the install (skipped with no `WOW_DATA`).
    #[test]
    fn named_sounds_decode() {
        let Some(dir) = std::env::var_os("WOW_DATA") else {
            eprintln!("skipped: no WOW_DATA");
            return;
        };
        let mut chain = benilla_formats::open_chain(std::path::Path::new(&dir)).expect("chain");
        let mut paths: Vec<String> = Vec::new();
        let mut all = vec![
            Music::Title,
            Music::Hamlet,
            Music::Crypt,
            Music::Battle,
            Music::Boss,
        ];
        for r in Region::ALL {
            all.extend([Music::Explore(r), Music::Fight(r), Music::Lair(r)]);
        }
        for m in all {
            paths.extend(m.tracks().iter().map(|s| (*s).to_owned()));
        }
        for id in [
            "skeleton_soldier",
            "ghoul",
            "banshee",
            "necrolyte",
            "abomination",
            "lich",
            "kobold_digger",
            "rat",
            "cave_creeper",
            "broodmother",
            "murloc_raider",
            "siren",
            "crab",
            "matriarch",
            "myrmidon",
        ] {
            for v in [Voice::Attack, Voice::Wound, Voice::Death] {
                paths.push(monster_sound(id, v, 1));
            }
        }
        use crate::rules::data::SkillKind;
        for (id, kind) in [
            ("smite", SkillKind::Spell),
            ("heal", SkillKind::Heal),
            ("fireball", SkillKind::Spell),
            ("frostbolt", SkillKind::Spell),
            ("shadow_bolt", SkillKind::Spell),
            ("aimed_shot", SkillKind::Ranged),
            ("cleave", SkillKind::Melee),
            ("blink", SkillKind::Support),
        ] {
            paths.push(skill_sound(id, kind, false, 1));
        }
        paths.push(skill_sound("cleave", SkillKind::Melee, true, 1));
        paths.extend(
            [MISS, COIN, AFFLICTION, VIRTUE, CLICK]
                .iter()
                .map(|s| (*s).to_owned()),
        );
        let mut bad = Vec::new();
        for p in &paths {
            match chain.read_file(p) {
                Ok(bytes) if p.ends_with(".mp3") => {
                    if StreamingSoundData::from_cursor(Cursor::new(bytes)).is_err() {
                        bad.push(format!("{p}: undecodable"));
                    }
                }
                Ok(bytes) => {
                    if StaticSoundData::from_cursor(Cursor::new(bytes)).is_err() {
                        bad.push(format!("{p}: undecodable"));
                    }
                }
                Err(e) => bad.push(format!("{p}: {e}")),
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }
}
