//! The hamlet as a place: Darkshire off the player's own map, seen at night from above its square,
//! the town hall facing the view, with lamplight, fog and a floating sign over each spot to click.

use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;

use super::stage::StageCamera;
use super::{Scoped, Screen};
use crate::rules::town::Building;
use crate::wow::{ModelSource, Rig, WowData, WowModel};

/// What clicking a spot in the hamlet opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    Building(Building),
    Memorial,
    Estate,
}

impl Spot {
    pub fn name(self) -> &'static str {
        match self {
            Spot::Building(b) => b.name(),
            Spot::Memorial => "Memorial",
            Spot::Estate => "The Estate (Embark)",
        }
    }
}

/// A clickable place in the town, with where its sign hangs.
#[derive(Component, Clone, Copy)]
pub struct TownSpot {
    pub spot: Spot,
    pub sign: Vec3,
}

/// The floating sign over a spot; a button.
#[derive(Component)]
pub struct Sign(pub Entity);

/// A sign was clicked.
#[derive(Resource, Default)]
pub struct Clicked(pub Option<Spot>);

#[derive(Component)]
struct Lamp(f32);

pub struct TownPlugin;

impl Plugin for TownPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Clicked>()
            .add_systems(OnEnter(Screen::Hamlet), build)
            .add_systems(OnExit(Screen::Hamlet), restore_camera)
            .add_systems(
                Update,
                (drift_camera, place_signs, click_signs, flicker).run_if(in_state(Screen::Hamlet)),
            );
    }
}

/// The night sky at the horizon (also the fog, so distant hills melt into it) and overhead.
const SKY_HORIZON: Color = Color::srgb(0.09, 0.12, 0.23);
const SKY_ZENITH: Color = Color::srgb(0.008, 0.012, 0.045);

#[allow(clippy::too_many_arguments)]
fn build(
    mut commands: Commands,
    data: Res<WowData>,
    map_res: Option<Res<super::darkshire::DarkshireMap>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut cam: Query<(&mut Transform, &mut DistanceFog, &mut Projection), With<StageCamera>>,
) {
    if let Ok((mut t, mut fog, mut proj)) = cam.single_mut() {
        *t = town_camera(0.0);
        if let Projection::Perspective(p) = &mut *proj {
            p.fov = (*TOWN_FOV).to_radians();
        }
        fog.color = SKY_HORIZON;
        fog.falloff = FogFalloff::Linear {
            start: 90.0,
            end: 330.0,
        };
    }
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.5, 0.56, 0.85),
        brightness: 220.0,
        ..default()
    });
    let scoped = Scoped(Screen::Hamlet);

    // The night sky: a gradient dome from a hazy blue horizon to a deep zenith, and a low moon.
    // The fog only tints geometry, so without a dome the sky is the clear colour (black).
    commands.insert_resource(ClearColor(SKY_HORIZON));
    let mut dome = Sphere::new(650.0).mesh().uv(48, 24);
    if let Some(bevy::mesh::VertexAttributeValues::Float32x3(pos)) =
        dome.attribute(Mesh::ATTRIBUTE_POSITION).cloned()
    {
        let (h, z) = (SKY_HORIZON.to_linear(), SKY_ZENITH.to_linear());
        let colors: Vec<[f32; 4]> = pos
            .iter()
            .map(|p| {
                let k = (p[1] / 650.0).clamp(0.0, 1.0).powf(0.45);
                h.mix(&z, k).to_f32_array()
            })
            .collect();
        dome.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    }
    commands.spawn((
        Mesh3d(meshes.add(dome)),
        MeshMaterial3d(mats.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            fog_enabled: false,
            cull_mode: None,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 150.0),
        bevy::camera::visibility::NoFrustumCulling,
        bevy::light::NotShadowCaster,
        scoped,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(14.0).mesh().uv(32, 16))),
        MeshMaterial3d(mats.add(StandardMaterial {
            base_color: Color::srgb(0.86, 0.9, 1.0),
            unlit: true,
            fog_enabled: false,
            ..default()
        })),
        Transform::from_xyz(125.0, 52.0, -400.0),
        bevy::light::NotShadowCaster,
        scoped,
    ));

    // The moon's light, cold and low.
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.55, 0.62, 0.95),
            illuminance: 3500.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(-90.0, 140.0, 70.0).looking_at(Vec3::ZERO, Vec3::Y),
        scoped,
    ));

    // Darkshire itself, off the player's map, read the first time the hamlet opens.
    let loaded = if map_res.is_none() {
        super::darkshire::DarkshireMap::load(&data)
    } else {
        None
    };
    let Some(map) = map_res.as_deref().or(loaded.as_ref()) else {
        warn!("hamlet: Darkshire's map tiles are missing from the install");
        return;
    };
    let lamp = |commands: &mut Commands, at: Vec3| {
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.72, 0.4),
                intensity: 300_000.0,
                range: 24.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(at),
            Lamp(300_000.0),
            scoped,
        ));
    };
    let buildings = super::darkshire::spawn(
        &mut commands,
        &mut meshes,
        &mut mats,
        &mut images,
        &data,
        map,
        scoped,
        lamp,
        TOWN_EYE,
        55.0,
    );
    // Each hamlet spot is one of Darkshire's own buildings or landmarks, found by where the map
    // puts it.
    for (e, model, at) in &buildings {
        let near = |x: f32, z: f32| (at.x - x).abs() < 4.0 && (at.z - z).abs() < 4.0;
        let role = ROLES
            .iter()
            .find(|r| model.contains(r.model) && near(r.at.0, r.at.1));
        if let Some(r) = role {
            commands.entity(*e).insert(TownSpot {
                spot: r.spot,
                sign: *at + Vec3::Y * r.sign_h,
            });
        }
    }
    // What the map lacks, set on its ground: the nomad's wagon by its fire, and a coach waiting
    // by the inn.
    let ground = |x: f32, z: f32| map.ground_at(Vec3::new(x, 0.0, z)).unwrap_or(0.0);
    let prop = |commands: &mut Commands,
                path: &str,
                x: f32,
                z: f32,
                yaw: f32,
                height: f32,
                spot: Option<(Spot, f32)>| {
        let at = Vec3::new(x, ground(x, z), z);
        let mut e = commands.spawn((
            Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            WowModel {
                source: ModelSource::Prop(path.to_owned(), None),
                height: Some(height),
            },
            scoped,
        ));
        if let Some((spot, h)) = spot {
            e.insert(TownSpot {
                spot,
                sign: at + Vec3::Y * h,
            });
        }
        at
    };
    prop(
        &mut commands,
        "World\\Azeroth\\Duskwood\\Buildings\\GypsyWagon\\GypsyWagon.m2",
        34.0,
        24.0,
        2.4,
        5.5,
        Some((Spot::Building(Building::Wagon), 7.5)),
    );
    let fire = prop(
        &mut commands,
        "World\\Azeroth\\Elwynn\\PassiveDoodads\\Campfire\\ElwynnCampfire.m2",
        26.0,
        19.0,
        0.0,
        0.9,
        None,
    );
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.55, 0.2),
            intensity: 900_000.0,
            range: 26.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_translation(fire + Vec3::Y * 1.5),
        Lamp(900_000.0),
        scoped,
    ));
    prop(
        &mut commands,
        "World\\Azeroth\\Westfall\\PassiveDoodads\\Westfall Wagon\\WestfallWagon01.m2",
        27.0,
        -9.0,
        1.9,
        4.6,
        Some((Spot::Building(Building::Stagecoach), 6.5)),
    );
    if let Some(m) = loaded {
        commands.insert_resource(m);
    }
}

/// Where the hamlet is seen from, `t` seconds in: before the town hall, a little above the lane,
/// drifting slowly. `CRAWL_TOWNCAM=x,y,z,lx,ly,lz` overrides it, for framing the view.
fn town_camera(t: f32) -> Transform {
    if let Some(v) = std::env::var("CRAWL_TOWNCAM").ok().and_then(|v| {
        let c: Vec<f32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        (c.len() == 6).then_some(c)
    }) {
        return Transform::from_xyz(v[0], v[1], v[2])
            .looking_at(Vec3::new(v[3], v[4], v[5]), Vec3::Y);
    }
    let d = t * 0.05;
    let pos = TOWN_EYE + Vec3::new(d.sin() * 4.0, (d * 1.3).sin() * 1.0, d.cos() * 2.0);
    Transform::from_translation(pos).looking_at(TOWN_LOOK, Vec3::Y)
}

/// The hamlet's vertical field of view, degrees; `CRAWL_TOWNFOV` overrides it.
static TOWN_FOV: std::sync::LazyLock<f32> = std::sync::LazyLock::new(|| {
    std::env::var("CRAWL_TOWNFOV")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(52.0)
});

/// Above the square's east end, looking west across the fountain at the town hall's front.
const TOWN_EYE: Vec3 = Vec3::new(88.0, 40.0, 6.0);
const TOWN_LOOK: Vec3 = Vec3::new(-12.0, 9.0, -2.0);

/// A hamlet spot found among Darkshire's models: its model, its scene x/z (yards from the town
/// hall's door, +x east, +z south), and how high its sign hangs.
struct Role {
    spot: Spot,
    model: &'static str,
    at: (f32, f32),
    sign_h: f32,
}

const ROLES: &[Role] = &[
    Role {
        spot: Spot::Building(Building::Guild),
        model: "duskwood_townhall",
        at: (0.0, 0.0),
        sign_h: 24.0,
    },
    Role {
        spot: Spot::Building(Building::Tavern),
        model: "duskwood_inn",
        at: (1.1, -30.3),
        sign_h: 17.0,
    },
    Role {
        spot: Spot::Building(Building::Blacksmith),
        model: "duskwood_blacksmith",
        at: (-0.7, 40.0),
        sign_h: 13.0,
    },
    Role {
        spot: Spot::Building(Building::Sanitarium),
        model: "duskwood_humantwostory",
        at: (53.2, -33.4),
        sign_h: 15.0,
    },
    Role {
        spot: Spot::Building(Building::Chapel),
        model: "duskwood_humantwostory",
        at: (50.5, 36.6),
        sign_h: 15.0,
    },
    Role {
        spot: Spot::Building(Building::Stagecoach),
        model: "westfallwagon01",
        at: (54.4, -9.9),
        sign_h: 5.5,
    },
    Role {
        spot: Spot::Memorial,
        model: "stormwindfountain01",
        at: (10.2, 9.9),
        sign_h: 7.0,
    },
];

/// Back to the stage's framing and fog for the crypts.
fn restore_camera(
    mut commands: Commands,
    mut cam: Query<(&mut Transform, &mut DistanceFog, &mut Projection), With<StageCamera>>,
) {
    commands.insert_resource(ClearColor(Color::BLACK));
    if let Ok((mut t, mut fog, mut proj)) = cam.single_mut() {
        if let Projection::Perspective(p) = &mut *proj {
            p.fov = super::stage::STAGE_FOV.to_radians();
        }
        *t = Transform::from_xyz(0.0, 1.7, 10.5).looking_at(Vec3::new(0.0, 1.7, 0.0), Vec3::Y);
        fog.color = Color::srgb(0.01, 0.008, 0.012);
        fog.falloff = FogFalloff::Linear {
            start: 11.0,
            end: 26.0,
        };
    }
}

fn drift_camera(time: Res<Time>, mut cam: Query<&mut Transform, With<StageCamera>>) {
    if let Ok(mut c) = cam.single_mut() {
        *c = town_camera(time.elapsed_secs());
    }
}

fn flicker(time: Res<Time>, mut q: Query<(&mut PointLight, &Lamp)>) {
    let t = time.elapsed_secs();
    for (i, (mut l, lamp)) in q.iter_mut().enumerate() {
        let p = i as f32 * 2.1;
        l.intensity = lamp.0 * (1.0 + 0.12 * (t * 6.0 + p).sin() * (t * 2.7 + p).cos());
    }
}

/// Keep each sign over its building, creating the signs as buildings appear.
#[allow(clippy::type_complexity)]
fn place_signs(
    mut commands: Commands,
    fonts: Res<super::ui::Fonts>,
    cams: Query<(&Camera, &GlobalTransform), With<StageCamera>>,
    spots: Query<(Entity, &TownSpot, Option<&Rig>)>,
    mut signs: Query<(
        &Sign,
        &mut Node,
        &Interaction,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
) {
    let Ok((cam, cam_t)) = cams.single() else {
        return;
    };
    let have: Vec<Entity> = signs.iter().map(|(s, ..)| s.0).collect();
    for (e, spot, _) in &spots {
        if !have.contains(&e) {
            commands
                .spawn((
                    Button,
                    Node {
                        position_type: PositionType::Absolute,
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.03, 0.02, 0.02, 0.8)),
                    BorderColor::all(super::ui::EDGE),
                    GlobalZIndex(3),
                    Sign(e),
                    Scoped(Screen::Hamlet),
                ))
                .with_children(|p| {
                    p.spawn((
                        Text::new(spot.spot.name()),
                        TextFont {
                            font: fonts.title.clone(),
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(super::ui::BONE),
                        TextShadow::default(),
                    ));
                });
        }
    }
    for (sign, mut node, i, mut bg, mut border) in &mut signs {
        let Ok((_, spot, _)) = spots.get(sign.0) else {
            continue;
        };
        // A spot outside the view (behind the camera, past an edge) hides its sign.
        let view = cam.logical_viewport_size().unwrap_or(Vec2::ZERO);
        match cam.world_to_viewport(cam_t, spot.sign) {
            Ok(p) if p.x > 0.0 && p.y > 0.0 && p.x < view.x && p.y < view.y => {
                node.display = Display::Flex;
                node.left = Val::Px(p.x - 70.0);
                node.top = Val::Px(p.y - 16.0);
            }
            _ => node.display = Display::None,
        }
        let hot = matches!(i, Interaction::Hovered | Interaction::Pressed);
        bg.0 = if hot {
            Color::srgba(0.25, 0.14, 0.06, 0.92)
        } else {
            Color::srgba(0.03, 0.02, 0.02, 0.8)
        };
        *border = BorderColor::all(if hot {
            super::ui::GOLD
        } else {
            super::ui::EDGE
        });
    }
}

fn click_signs(
    signs: Query<(&Interaction, &Sign), Changed<Interaction>>,
    spots: Query<&TownSpot>,
    mut clicked: ResMut<Clicked>,
    mut sfx: ResMut<crate::audio::Sfx>,
) {
    for (i, s) in &signs {
        if *i == Interaction::Pressed {
            if let Ok(spot) = spots.get(s.0) {
                clicked.0 = Some(spot.spot);
                sfx.play(crate::audio::CLICK, -12.0);
            }
        }
    }
}
