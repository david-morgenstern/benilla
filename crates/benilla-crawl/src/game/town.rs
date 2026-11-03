//! The hamlet as a place: Darkshire's buildings around a moonlit square, the ruined estate on the
//! horizon, lamplight and fog, and a floating sign over each building to click.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;

use super::stage::StageCamera;
use super::{Scoped, Screen};
use crate::rules::town::Building;
use crate::wow::{ModelSource, Rig, WowModel};

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

fn tiled(w: f32, d: f32, tiles: f32) -> Mesh {
    let (x, z) = (w / 2.0, d / 2.0);
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    m.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[-x, 0.0, -z], [x, 0.0, -z], [x, 0.0, z], [-x, 0.0, z]],
    );
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 4]);
    let (u, v) = (w / tiles, d / tiles);
    m.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, 0.0], [u, 0.0], [u, v], [0.0, v]],
    );
    m.insert_indices(Indices::U32(vec![0, 2, 1, 0, 3, 2]));
    m
}

const STABLE: &str = "World\\wmo\\Azeroth\\Buildings\\Duskwood_Stable\\Duskwood_Stable.wmo";
const INN: &str = "World\\wmo\\Azeroth\\Buildings\\Duskwood_Inn\\Duskwood_Inn.wmo";
const CHAPEL: &str = "World\\wmo\\Azeroth\\Buildings\\Chapel\\DuskwoodChapel.wmo";
const HALL: &str =
    "World\\wmo\\Azeroth\\Buildings\\Duskwood_HumanTwoStory\\Duskwood_HumanTwoStory.wmo";
const SMITH: &str = "World\\wmo\\Azeroth\\Buildings\\Duskwood_Blacksmith\\Duskwood_Blacksmith.wmo";
const TOWER: &str = "World\\wmo\\Azeroth\\Buildings\\Duskwood_MageTower\\Duskwood_MageTower.wmo";
/// The night sky at the horizon (also the fog, so distant hills melt into it) and overhead.
const SKY_HORIZON: Color = Color::srgb(0.09, 0.12, 0.23);
const SKY_ZENITH: Color = Color::srgb(0.008, 0.012, 0.045);

const ESTATE: &str = "World\\wmo\\Azeroth\\Buildings\\RuinedCathedral\\RuinedCathedral.wmo";

fn build(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut cam: Query<(&mut Transform, &mut DistanceFog), With<StageCamera>>,
) {
    if let Ok((mut t, mut fog)) = cam.single_mut() {
        *t = Transform::from_xyz(0.0, 26.0, 62.0).looking_at(Vec3::new(0.0, 6.0, -13.8), Vec3::Y);
        fog.color = SKY_HORIZON;
        fog.falloff = FogFalloff::Linear {
            start: 160.0,
            end: 520.0,
        };
    }
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.45, 0.5, 0.8),
        brightness: 55.0,
        ..default()
    });
    let scoped = Scoped(Screen::Hamlet);
    let tex = |p: &str| asset_server.load(benilla_assets::texture_url(p, (true, true)));
    // Ground: grass, and a cobbled square.
    let grass = mats.add(StandardMaterial {
        base_color_texture: Some(tex("Tileset\\Duskwood\\DuskwoodGrassBase02")),
        base_color: Color::srgb(0.55, 0.6, 0.55),
        perceptual_roughness: 1.0,
        ..default()
    });
    let cobble = mats.add(StandardMaterial {
        base_color_texture: Some(tex("Tileset\\Duskwood\\DuskwoodCobblestone")),
        base_color: Color::srgb(0.7, 0.7, 0.72),
        perceptual_roughness: 0.95,
        ..default()
    });
    let dirt = mats.add(StandardMaterial {
        base_color_texture: Some(tex("Tileset\\Duskwood\\DuskwoodDirt")),
        perceptual_roughness: 1.0,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(tiled(900.0, 900.0, 8.0))),
        MeshMaterial3d(grass),
        Transform::default(),
        scoped,
    ));
    commands.spawn((
        Mesh3d(meshes.add(tiled(150.0, 80.0, 6.0))),
        MeshMaterial3d(cobble),
        Transform::from_xyz(0.0, 0.03, 5.0),
        scoped,
    ));
    commands.spawn((
        Mesh3d(meshes.add(tiled(16.0, 300.0, 6.0))),
        MeshMaterial3d(dirt.clone()),
        Transform::from_xyz(23.0, 0.02, -150.0),
        scoped,
    ));

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
            illuminance: 2200.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(-90.0, 140.0, 70.0).looking_at(Vec3::ZERO, Vec3::Y),
        scoped,
    ));

    let place = |commands: &mut Commands,
                 path: &str,
                 pos: Vec3,
                 yaw: f32,
                 spot: Option<Spot>,
                 sign_h: f32,
                 wmo: bool,
                 height: Option<f32>| {
        let source = if wmo {
            ModelSource::Wmo(path.to_owned())
        } else {
            ModelSource::Prop(path.to_owned(), None)
        };
        let mut e = commands.spawn((
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            WowModel { source, height },
            scoped,
        ));
        if let Some(spot) = spot {
            e.insert(TownSpot {
                spot,
                sign: pos + Vec3::Y * sign_h,
            });
        }
    };
    // The square, as a horseshoe opening toward the viewer, the guild tower rising behind.
    let b = |x: Building| Some(Spot::Building(x));
    place(
        &mut commands,
        STABLE,
        Vec3::new(-66.0, 0.0, 8.0),
        1.25,
        b(Building::Stagecoach),
        16.0,
        true,
        None,
    );
    place(
        &mut commands,
        INN,
        Vec3::new(-50.0, 0.0, -38.0),
        0.6,
        b(Building::Tavern),
        26.0,
        true,
        None,
    );
    place(
        &mut commands,
        CHAPEL,
        Vec3::new(8.0, 0.0, -62.0),
        0.0,
        b(Building::Chapel),
        34.0,
        true,
        None,
    );
    place(
        &mut commands,
        HALL,
        Vec3::new(58.0, 0.0, -40.0),
        -0.6,
        b(Building::Sanitarium),
        22.0,
        true,
        None,
    );
    place(
        &mut commands,
        SMITH,
        Vec3::new(68.0, 0.0, 8.0),
        -1.3,
        b(Building::Blacksmith),
        16.0,
        true,
        None,
    );
    place(
        &mut commands,
        TOWER,
        Vec3::new(-28.0, 0.0, -96.0),
        0.5,
        b(Building::Guild),
        44.0,
        true,
        None,
    );
    // The estate, on its hill above the hamlet.
    let hill = mats.add(StandardMaterial {
        base_color_texture: Some(tex("Tileset\\Duskwood\\DuskwoodGrassBase04")),
        base_color: Color::srgb(0.35, 0.4, 0.35),
        perceptual_roughness: 1.0,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(32, 16))),
        MeshMaterial3d(hill),
        Transform::from_xyz(30.0, -40.0, -340.0).with_scale(Vec3::new(170.0, 66.0, 100.0)),
        scoped,
    ));
    commands.spawn((
        Transform::from_xyz(30.0, 24.0, -340.0).with_scale(Vec3::splat(1.35)),
        Visibility::default(),
        WowModel {
            source: ModelSource::Wmo(ESTATE.to_owned()),
            height: None,
        },
        TownSpot {
            spot: Spot::Estate,
            sign: Vec3::new(30.0, 78.0, -340.0),
        },
        scoped,
    ));
    // The nomad wagon by its fire.
    place(
        &mut commands,
        "World\\Azeroth\\Duskwood\\Buildings\\GypsyWagon\\GypsyWagon.m2",
        Vec3::new(40.0, 0.0, 36.0),
        -0.9,
        b(Building::Wagon),
        9.0,
        false,
        Some(7.0),
    );
    place(
        &mut commands,
        "World\\Azeroth\\Elwynn\\PassiveDoodads\\Campfire\\ElwynnCampfire.m2",
        Vec3::new(27.0, 0.0, 40.0),
        0.0,
        None,
        0.0,
        false,
        Some(2.0),
    );
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.55, 0.2),
            intensity: 1_600_000.0,
            range: 40.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(27.0, 3.0, 40.0),
        Lamp(1_600_000.0),
        scoped,
    ));
    // The memorial: a crowded graveyard at the front-left, fenced.
    let graves = [
        "World\\Azeroth\\Duskwood\\PassiveDoodads\\Tombs\\TombStone05.m2",
        "World\\Azeroth\\Duskwood\\PassiveDoodads\\Tombs\\TombStone06.m2",
        "World\\Azeroth\\Duskwood\\PassiveDoodads\\Tombs\\TombStone07.m2",
        "World\\Azeroth\\Duskwood\\PassiveDoodads\\Tombs\\WoodCross01.m2",
        "World\\Azeroth\\Duskwood\\PassiveDoodads\\Tombs\\TombStone08.m2",
        "World\\Azeroth\\Duskwood\\PassiveDoodads\\Tombs\\WoodCross02.m2",
    ];
    for i in 0..15 {
        let x = -72.0 + (i % 5) as f32 * 6.0 + ((i * 7) % 3) as f32;
        let z = 40.0 + (i / 5) as f32 * 7.0;
        place(
            &mut commands,
            graves[i % graves.len()],
            Vec3::new(x, 0.0, z),
            0.25 * i as f32 - 0.6,
            None,
            0.0,
            false,
            Some(2.6),
        );
    }
    place(
        &mut commands,
        "World\\Azeroth\\Duskwood\\PassiveDoodads\\Tombs\\TombStoneMonument01.m2",
        Vec3::new(-58.0, 0.0, 34.0),
        0.3,
        Some(Spot::Memorial),
        10.0,
        false,
        Some(6.0),
    );
    for i in 0..8 {
        let x = -78.0 + i as f32 * 5.0;
        place(
            &mut commands,
            "World\\Azeroth\\Duskwood\\PassiveDoodads\\Fence\\DuskWoodFenceSegment.m2",
            Vec3::new(x, 0.0, 64.0),
            0.0,
            None,
            0.0,
            false,
            Some(2.2),
        );
    }
    // Hamlet clutter.
    let clutter: [(&str, f32, f32, f32, f32); 10] = [
        (
            "World\\Azeroth\\Duskwood\\PassiveDoodads\\DuskwoodHaywagon\\DuskwoodHayWagon.m2",
            -22.0,
            30.0,
            0.7,
            5.0,
        ),
        (
            "World\\Azeroth\\Duskwood\\PassiveDoodads\\DuskwoodScarecrow\\DuskScarecrow.m2",
            70.0,
            48.0,
            -0.4,
            6.0,
        ),
        (
            "World\\Azeroth\\Duskwood\\PassiveDoodads\\Bush\\PumpkinPatch01.m2",
            62.0,
            58.0,
            0.0,
            2.0,
        ),
        (
            "World\\Azeroth\\Duskwood\\PassiveDoodads\\Bush\\PumpkinPatch01.m2",
            80.0,
            54.0,
            1.0,
            2.0,
        ),
        (
            "World\\Generic\\PassiveDoodads\\Barrel\\Barrel01.m2",
            -36.0,
            -18.0,
            0.0,
            2.2,
        ),
        (
            "World\\Generic\\PassiveDoodads\\Barrel\\Barrel01.m2",
            -32.0,
            -16.0,
            0.5,
            2.2,
        ),
        (
            "World\\Azeroth\\Karazahn\\PassiveDoodads\\Crates\\KarazahnCrate01.m2",
            66.0,
            -10.0,
            0.3,
            2.4,
        ),
        (
            "World\\Azeroth\\Karazahn\\PassiveDoodads\\Crates\\KarazahnCrate02.m2",
            70.0,
            -6.0,
            1.2,
            2.0,
        ),
        (
            "World\\Azeroth\\Duskwood\\PassiveDoodads\\Bush\\DuskWoodSpookyBush02.m2",
            -10.0,
            52.0,
            0.0,
            5.0,
        ),
        (
            "World\\Azeroth\\Duskwood\\PassiveDoodads\\Bush\\DuskwoodSpookyBush03.m2",
            14.0,
            60.0,
            1.0,
            5.0,
        ),
    ];
    for (path, x, z, yaw, h) in clutter {
        place(
            &mut commands,
            path,
            Vec3::new(x, 0.0, z),
            yaw,
            None,
            0.0,
            false,
            Some(h),
        );
    }
    // A forest wall all around.
    let trees = [
        "World\\Azeroth\\Duskwood\\PassiveDoodads\\Trees\\DuskwoodSpookyTree01.m2",
        "World\\Azeroth\\Duskwood\\PassiveDoodads\\Trees\\DuskwoodSpookyTree02.m2",
        "World\\Azeroth\\Duskwood\\PassiveDoodads\\Trees\\DuskwoodSpookyTree03.m2",
        "World\\Azeroth\\Duskwood\\PassiveDoodads\\Trees\\DuskwoodTree06.m2",
        "World\\Azeroth\\Duskwood\\PassiveDoodads\\Trees\\DuskwoodTree07.m2",
    ];
    let mut k = 0usize;
    for ring in 0..3 {
        let r = 125.0 + ring as f32 * 40.0;
        for j in 0..22 {
            let a = std::f32::consts::PI * (0.95 + 1.1 * j as f32 / 21.0) + ring as f32 * 0.07;
            let (x, z) = (a.cos() * r * 1.25, a.sin() * r * 0.85 - 40.0);
            // Leave the view of the estate open.
            if x.abs() < 45.0 && z < -100.0 {
                continue;
            }
            k += 1;
            place(
                &mut commands,
                trees[k % trees.len()],
                Vec3::new(x, 0.0, z),
                k as f32 * 1.3,
                None,
                0.0,
                false,
                Some(22.0 + (k % 5) as f32 * 5.0),
            );
        }
    }
    for (x, z) in [
        (-118.0, 70.0),
        (122.0, 72.0),
        (-135.0, 30.0),
        (138.0, 28.0),
        (-100.0, 95.0),
        (104.0, 98.0),
    ] {
        k += 1;
        place(
            &mut commands,
            trees[k % trees.len()],
            Vec3::new(x, 0.0, z),
            k as f32,
            None,
            0.0,
            false,
            Some(30.0),
        );
    }
    // A paved square and the road to the estate.
    let _ = &dirt;
    // Lamp posts around the square, each with its pool of light.
    for (x, z) in [
        (-40.0, 18.0),
        (40.0, 18.0),
        (-24.0, -22.0),
        (30.0, -24.0),
        (-62.0, -10.0),
        (64.0, -12.0),
        (8.0, -36.0),
    ] {
        place(
            &mut commands,
            "World\\Azeroth\\Elwynn\\PassiveDoodads\\LampPost\\LampPost.m2",
            Vec3::new(x, 0.0, z),
            0.0,
            None,
            0.0,
            false,
            Some(7.0),
        );
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.72, 0.4),
                intensity: 700_000.0,
                range: 32.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(x, 7.0, z),
            Lamp(700_000.0),
            scoped,
        ));
    }
    // The estate's sickly glow over the hill.
    commands.spawn((
        PointLight {
            color: Color::srgb(0.35, 0.95, 0.45),
            intensity: 120_000_000.0,
            range: 220.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(30.0, 70.0, -290.0),
        Lamp(120_000_000.0),
        scoped,
    ));
    let _ = asset_server;
}

/// Back to the stage's framing and fog for the crypts.
fn restore_camera(
    mut commands: Commands,
    mut cam: Query<(&mut Transform, &mut DistanceFog), With<StageCamera>>,
) {
    commands.insert_resource(ClearColor(Color::BLACK));
    if let Ok((mut t, mut fog)) = cam.single_mut() {
        *t = Transform::from_xyz(0.0, 1.7, 10.5).looking_at(Vec3::new(0.0, 1.7, 0.0), Vec3::Y);
        fog.color = Color::srgb(0.01, 0.008, 0.012);
        fog.falloff = FogFalloff::Linear {
            start: 11.0,
            end: 26.0,
        };
    }
}

fn drift_camera(time: Res<Time>, mut cam: Query<&mut Transform, With<StageCamera>>) {
    let t = time.elapsed_secs() * 0.05;
    if let Ok(mut c) = cam.single_mut() {
        let pos = Vec3::new(
            t.sin() * 14.0,
            55.0 + (t * 1.3).sin() * 2.0,
            150.0 + t.cos() * 4.0,
        );
        *c = Transform::from_translation(pos).looking_at(Vec3::new(0.0, 14.0, -40.0), Vec3::Y);
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
        if let Ok(p) = cam.world_to_viewport(cam_t, spot.sign) {
            node.left = Val::Px(p.x - 70.0);
            node.top = Val::Px(p.y - 16.0);
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
