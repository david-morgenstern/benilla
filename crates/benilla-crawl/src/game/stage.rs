//! The stage: a side-on crypt corridor built from the install's own crypt textures and props, a
//! torch that sets the mood, and the actors who stand in two lines and fight.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;

use super::{Run, Scoped, Screen};
use crate::rules::combat::Side;
use crate::rules::regions::Region;
use crate::wow::{ModelSource, PlayAnim, Rig, WowModel};

/// The span of one repeat of the corridor's decoration; the scroll wraps on it.
pub const REPEAT: f32 = 24.0;
/// Distance walked per hallway segment.
pub const SEG_LEN: f32 = 9.0;

const WALL_Z: f32 = -3.6;
const FLOOR_FRONT: f32 = 3.5;

#[derive(Component)]
pub struct StageCamera;

#[derive(Component)]
struct Scroller;

#[derive(Component)]
pub struct PartyTorch;

#[derive(Component)]
pub struct Sconce {
    pub base: f32,
    pub phase: f32,
}

/// How far the party has walked, in stage units; the scenery scrolls by it.
#[derive(Resource, Default)]
pub struct Walk {
    pub offset: f32,
    /// Camera shake, decaying.
    pub shake: f32,
    /// Camera push-in for the big moments, 0..1.
    pub zoom: f32,
    pub zoom_target: f32,
}

/// Someone standing on the stage.
#[derive(Component, Clone, Copy, Debug)]
pub struct Actor {
    pub side: Side,
    pub hero: Option<u32>,
    pub unit: Option<usize>,
    /// Where the actor belongs; it glides there.
    pub home: Vec3,
    /// A temporary push toward the centre (a lunge), decaying.
    pub lunge: f32,
    pub alive: bool,
}

/// What the actor's plate shows; the screens keep it current.
#[derive(Component, Clone, Debug, Default)]
pub struct ActorView {
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub stress: Option<i32>,
    pub status: String,
    pub deaths_door: bool,
    pub mark: Mark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mark {
    #[default]
    None,
    /// Whose turn it is.
    Active,
    /// A legal target for the chosen skill.
    Target,
    /// A legal ally target.
    Friend,
    /// Selected outside battle.
    Selected,
}

/// The clickable plate under an actor.
#[derive(Component)]
pub struct Plate {
    pub actor: Entity,
}

#[derive(Component)]
struct PlateHp;
#[derive(Component)]
struct PlateStress(usize);
#[derive(Component)]
struct PlateStatus;
/// A glowing ring on the floor under an actor, coloured by its [`Mark`].
#[derive(Component)]
pub struct Ring(Handle<StandardMaterial>);

/// Scenery that should not throw shadows across the stage.
#[derive(Component)]
pub struct NoShadows;

#[derive(Component)]
struct Vignette;

pub struct StagePlugin;

impl Plugin for StagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Walk>()
            .add_systems(Startup, spawn_camera)
            .add_systems(OnEnter(Screen::Dungeon), build_scenery)
            .add_systems(
                Update,
                (
                    scroll,
                    glide_actors,
                    light_the_torch,
                    flicker,
                    camera_feel,
                    spawn_plates,
                    place_plates,
                    add_rings,
                    color_rings,
                    unshadow,
                )
                    .run_if(in_state(Screen::Dungeon)),
            );
    }
}

/// The stage camera's vertical field of view, degrees.
pub const STAGE_FOV: f32 = 34.0;

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Projection::from(PerspectiveProjection {
            fov: STAGE_FOV.to_radians(),
            near: 0.1,
            far: 200.0,
            ..default()
        }),
        Transform::from_xyz(0.0, 1.7, 10.5).looking_at(Vec3::new(0.0, 1.7, 0.0), Vec3::Y),
        DistanceFog {
            color: Color::srgb(0.01, 0.008, 0.012),
            falloff: FogFalloff::Linear {
                start: 11.0,
                end: 26.0,
            },
            ..default()
        },
        StageCamera,
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.55, 0.55, 0.8),
        brightness: 6.0,
        ..default()
    });
}

/// A quad in the XY plane facing +Z, `u`×`v` texture repeats across it.
fn quad(w: f32, h: f32, u: f32, v: f32) -> Mesh {
    let (x, y) = (w / 2.0, h / 2.0);
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    m.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[-x, -y, 0.0], [x, -y, 0.0], [x, y, 0.0], [-x, y, 0.0]],
    );
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 4]);
    m.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, v], [u, v], [u, 0.0], [0.0, 0.0]],
    );
    m.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
    m
}

fn tex(asset_server: &AssetServer, path: &str) -> Handle<Image> {
    asset_server.load(benilla_assets::texture_url(path, (true, true)))
}

fn build_scenery(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut walk: ResMut<Walk>,
    mut images: ResMut<Assets<Image>>,
    run: Option<Res<Run>>,
    mut fogs: Query<&mut DistanceFog, With<StageCamera>>,
) {
    *walk = Walk::default();
    spawn_vignette(&mut commands, &mut images);
    let mat = |mats: &mut Assets<StandardMaterial>, path: &str, tint: Color| {
        mats.add(StandardMaterial {
            base_color_texture: Some(tex(&asset_server, path)),
            base_color: tint,
            perceptual_roughness: 0.95,
            reflectance: 0.05,
            ..default()
        })
    };
    let theme = theme(
        run.as_ref()
            .map_or(Region::Crypts, |r| r.0.dungeon.quest.region),
    );
    let floor_m = mat(&mut mats, theme.floor, theme.tint);
    let wall_m = mat(&mut mats, theme.wall, theme.tint);
    let trim_m = mat(&mut mats, theme.trim, theme.tint);
    let pillar_m = mat(&mut mats, theme.pillar, theme.tint);
    let ceiling_m = mat(&mut mats, theme.ceiling, Color::srgb(0.5, 0.48, 0.48));
    if let Ok(mut fog) = fogs.single_mut() {
        fog.color = theme.fog;
    }

    let length = REPEAT * 8.0;
    let root = commands
        .spawn((
            Transform::default(),
            Visibility::default(),
            Scroller,
            Scoped(Screen::Dungeon),
        ))
        .id();
    let depth = FLOOR_FRONT - WALL_Z;
    let floor = meshes.add(quad(length, depth, length / 3.0, depth / 3.0));
    let wall = meshes.add(quad(length, 7.0, length / 3.5, 2.0));
    let trim = meshes.add(quad(length, 0.7, length / 2.0, 1.0));
    let ceiling = meshes.add(quad(length, depth, length / 4.0, depth / 4.0));
    // A cuboid maps the whole texture onto each face; repeat it up the pillar's height instead
    // of smearing it into a flat brown beam.
    let mut pillar = Cuboid::new(0.9, 7.0, 0.9).mesh().build();
    if let Some(bevy::mesh::VertexAttributeValues::Float32x2(uvs)) =
        pillar.attribute_mut(Mesh::ATTRIBUTE_UV_0)
    {
        for uv in uvs.iter_mut() {
            uv[1] *= 5.0;
        }
    }
    let pillar = meshes.add(pillar);
    commands.entity(root).with_children(|p| {
        p.spawn((
            Mesh3d(floor),
            MeshMaterial3d(floor_m),
            Transform::from_xyz(0.0, 0.0, (FLOOR_FRONT + WALL_Z) / 2.0)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        ));
        p.spawn((
            Mesh3d(wall),
            MeshMaterial3d(wall_m),
            Transform::from_xyz(0.0, 3.5, WALL_Z),
        ));
        p.spawn((
            Mesh3d(trim),
            MeshMaterial3d(trim_m),
            Transform::from_xyz(0.0, 0.35, WALL_Z + 0.02),
        ));
        p.spawn((
            Mesh3d(ceiling),
            MeshMaterial3d(ceiling_m),
            Transform::from_xyz(0.0, 6.2, (FLOOR_FRONT + WALL_Z) / 2.0)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        ));
        let start = -length / 2.0;
        let mut x = start;
        let mut i = 0usize;
        while x < length / 2.0 {
            // Pillars every 8 units.
            p.spawn((
                Mesh3d(pillar.clone()),
                MeshMaterial3d(pillar_m.clone()),
                Transform::from_xyz(x, 3.5, WALL_Z + 0.4),
                bevy::light::NotShadowCaster,
            ));
            // Between pillars, a rotating cast of crypt furnishings.
            let mid = x + 4.0;
            let prop = theme.props[i % theme.props.len()];
            p.spawn((
                Transform::from_xyz(mid, prop.2, WALL_Z + 0.35),
                Visibility::default(),
                WowModel {
                    source: ModelSource::Prop(prop.0.to_owned(), None),
                    height: Some(prop.1),
                },
                NoShadows,
            ));
            if i.is_multiple_of(3) {
                p.spawn((
                    PointLight {
                        color: theme.sconce,
                        intensity: 25_000.0,
                        range: 7.0,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(mid, 2.4, WALL_Z + 0.9),
                    Sconce {
                        base: 25_000.0,
                        phase: i as f32 * 1.7,
                    },
                ));
            }
            if i % 4 == 2 {
                p.spawn((
                    Transform::from_xyz(mid + 2.0, 0.0, WALL_Z + 0.8),
                    Visibility::default(),
                    WowModel {
                        source: ModelSource::Prop(theme.clutter.into(), None),
                        height: Some(0.6),
                    },
                ));
            }
            x += 8.0;
            i += 1;
        }
    });
    // The party's torch, held up behind the front rank.
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.7, 0.4),
            intensity: 900_000.0,
            range: 18.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(-2.6, 3.2, 2.4),
        PartyTorch,
        Scoped(Screen::Dungeon),
    ));
    // A cold glow from ahead, so the enemy line is never fully black.
    commands.spawn((
        PointLight {
            color: Color::srgb(0.45, 0.5, 0.9),
            intensity: 120_000.0,
            range: 14.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(5.0, 3.0, 3.0),
        Scoped(Screen::Dungeon),
    ));
}

fn scroll(walk: Res<Walk>, mut q: Query<&mut Transform, With<Scroller>>) {
    for mut t in &mut q {
        t.translation.x = -walk.offset.rem_euclid(REPEAT);
    }
}

/// Stage x of a rank on a side; `rank` may be fractional for a big body's centre.
pub fn slot(side: Side, rank: f32) -> Vec3 {
    let sign = match side {
        Side::Heroes => -1.0,
        Side::Monsters => 1.0,
    };
    let x = sign * (1.0 + (rank - 1.0) * 1.2);
    let z = if (rank.round() as i32) % 2 == 0 {
        -0.35
    } else {
        0.25
    };
    Vec3::new(x, 0.0, z)
}

pub fn facing(side: Side) -> Quat {
    match side {
        Side::Heroes => Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2 + 0.35),
        Side::Monsters => Quat::from_rotation_y(std::f32::consts::FRAC_PI_2 - 0.35),
    }
}

pub fn spawn_actor(
    commands: &mut Commands,
    actor: Actor,
    source: ModelSource,
    height: f32,
    view: ActorView,
    at: Vec3,
) -> Entity {
    commands
        .spawn((
            Transform::from_translation(at).with_rotation(facing(actor.side)),
            Visibility::default(),
            WowModel {
                source,
                height: Some(height),
            },
            PlayAnim::looped(crate::rules::data::anim::STAND),
            actor,
            view,
            Scoped(Screen::Dungeon),
        ))
        .id()
}

fn glide_actors(time: Res<Time>, mut q: Query<(&mut Actor, &mut Transform)>) {
    let dt = time.delta_secs();
    for (mut a, mut t) in &mut q {
        let dir = match a.side {
            Side::Heroes => 1.0,
            Side::Monsters => -1.0,
        };
        let target = a.home + Vec3::X * dir * a.lunge;
        let k = 1.0 - (-dt * 7.0).exp();
        t.translation = t.translation.lerp(target, k);
        a.lunge *= (-dt * 2.5).exp();
    }
}

/// The torch's light follows the expedition's light level.
fn light_the_torch(
    run: Option<Res<Run>>,
    time: Res<Time>,
    mut q: Query<&mut PointLight, With<PartyTorch>>,
) {
    let Some(run) = run else {
        return;
    };
    let light = run.0.light as f32 / 100.0;
    let t = time.elapsed_secs();
    let flick = 1.0 + 0.06 * (t * 9.0).sin() * (t * 3.7).cos() + 0.03 * (t * 23.0).sin();
    for mut l in &mut q {
        l.intensity = (60_000.0 + 520_000.0 * light) * flick;
        l.range = 7.0 + 9.0 * light;
    }
}

fn flicker(time: Res<Time>, mut q: Query<(&mut PointLight, &Sconce)>) {
    let t = time.elapsed_secs();
    for (mut l, s) in &mut q {
        l.intensity = s.base * (1.0 + 0.25 * (t * 7.0 + s.phase).sin() * (t * 2.3 + s.phase).cos());
    }
}

fn camera_feel(
    time: Res<Time>,
    mut walk: ResMut<Walk>,
    mut q: Query<&mut Transform, With<StageCamera>>,
) {
    let dt = time.delta_secs();
    walk.shake *= (-dt * 6.0).exp();
    let target = walk.zoom_target;
    walk.zoom += (target - walk.zoom) * (1.0 - (-dt * 4.0).exp());
    let Ok(mut t) = q.single_mut() else {
        return;
    };
    let tt = time.elapsed_secs();
    let shake = Vec3::new((tt * 61.0).sin(), (tt * 47.0).cos(), 0.0) * walk.shake * 0.12;
    let z = 10.5 - walk.zoom * 1.6;
    let y = 1.7 - walk.zoom * 0.15;
    *t = Transform::from_xyz(0.0, y, z)
        .looking_at(Vec3::new(0.0, 1.7 - walk.zoom * 0.15, 0.0), Vec3::Y);
    t.translation += shake;
}

fn spawn_plates(
    mut commands: Commands,
    fonts: Res<super::ui::Fonts>,
    new: Query<(Entity, &ActorView), Added<ActorView>>,
) {
    for (e, view) in &new {
        let hero = view.stress.is_some();
        commands
            .spawn((
                Button,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(120.0),
                    height: Val::Px(250.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::FlexEnd,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Plate { actor: e },
                Scoped(Screen::Dungeon),
                GlobalZIndex(5),
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(""),
                    TextFont {
                        font: fonts.body.clone(),
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(super::ui::GOLD),
                    TextShadow {
                        offset: Vec2::splat(1.0),
                        color: Color::BLACK,
                    },
                    TextLayout::new_with_justify(Justify::Center),
                    PlateStatus,
                ));
                p.spawn((
                    Node {
                        width: Val::Px(84.0),
                        height: Val::Px(9.0),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.1, 0.02, 0.02)),
                    BorderColor::all(Color::BLACK),
                ))
                .with_children(|b| {
                    b.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(super::ui::BLOOD),
                        PlateHp,
                    ));
                });
                if hero {
                    p.spawn(Node {
                        column_gap: Val::Px(1.0),
                        ..default()
                    })
                    .with_children(|r| {
                        for i in 0..10 {
                            r.spawn((
                                Node {
                                    width: Val::Px(7.4),
                                    height: Val::Px(7.4),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.12, 0.12, 0.14)),
                                PlateStress(i),
                            ));
                        }
                    });
                }
            });
    }
}

#[allow(clippy::type_complexity)]
fn place_plates(
    mut commands: Commands,
    cams: Query<(&Camera, &GlobalTransform), With<StageCamera>>,
    actors: Query<(&GlobalTransform, &ActorView, Option<&Rig>, &Actor)>,
    mut plates: Query<(Entity, &Plate, &mut Node, &Children)>,
    mut hp: Query<(&mut Node, &mut BackgroundColor), (With<PlateHp>, Without<Plate>)>,
    mut stress: Query<(&PlateStress, &mut BackgroundColor), Without<PlateHp>>,
    mut status: Query<(&mut Text, &mut TextColor), With<PlateStatus>>,
    children: Query<&Children>,
) {
    let Ok((cam, cam_t)) = cams.single() else {
        return;
    };
    for (pe, plate, mut node, kids) in &mut plates {
        let Ok((gt, view, rig, actor)) = actors.get(plate.actor) else {
            commands.entity(pe).despawn();
            continue;
        };
        let feet = gt.translation();
        let head = feet + Vec3::Y * rig.map_or(1.9, |r| r.height);
        let (Ok(f), Ok(h)) = (
            cam.world_to_viewport(cam_t, feet),
            cam.world_to_viewport(cam_t, head),
        ) else {
            continue;
        };
        let height = (f.y - h.y).max(40.0) + 40.0;
        node.left = Val::Px(f.x - 60.0);
        node.top = Val::Px(h.y - 10.0);
        node.height = Val::Px(height);
        node.display = if actor.alive || view.hp > 0 {
            Display::Flex
        } else {
            Display::None
        };
        // Walk the plate's descendants to refresh bars and text.
        let mut stack: Vec<Entity> = kids.iter().collect();
        while let Some(c) = stack.pop() {
            if let Ok((mut n, mut bg)) = hp.get_mut(c) {
                let frac = if view.max_hp > 0 {
                    view.hp as f32 / view.max_hp as f32
                } else {
                    0.0
                };
                n.width = Val::Percent(frac.clamp(0.0, 1.0) * 100.0);
                bg.0 = if view.deaths_door {
                    Color::srgb(0.35, 0.0, 0.0)
                } else {
                    super::ui::BLOOD
                };
            }
            if let Ok((ps, mut bg)) = stress.get_mut(c) {
                let s = view.stress.unwrap_or(0);
                let i = ps.0 as i32;
                bg.0 = if s >= 100 + (i + 1) * 10 {
                    super::ui::BLOOD
                } else if s >= (i + 1) * 10 {
                    super::ui::STRESS
                } else {
                    Color::srgb(0.12, 0.12, 0.14)
                };
            }
            if let Ok((mut t, mut col)) = status.get_mut(c) {
                let s = if view.deaths_door {
                    format!("DEATH'S DOOR\n{}", view.status)
                } else {
                    view.status.clone()
                };
                if t.0 != s {
                    t.0 = s;
                }
                col.0 = if view.deaths_door {
                    super::ui::BLOOD
                } else {
                    super::ui::GOLD
                };
            }
            if let Ok(cc) = children.get(c) {
                stack.extend(cc.iter());
            }
        }
    }
}

fn add_rings(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    new: Query<Entity, Added<Actor>>,
) {
    for e in &new {
        let mesh = meshes.add(Mesh::from(Annulus::new(0.5, 0.66)));
        let mat = mats.add(StandardMaterial {
            base_color: Color::NONE,
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        });
        let ring = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(mat.clone()),
                Transform::from_xyz(0.0, 0.03, 0.0)
                    .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                bevy::light::NotShadowCaster,
                Ring(mat),
            ))
            .id();
        commands.entity(e).add_child(ring);
    }
}

fn color_rings(
    time: Res<Time>,
    actors: Query<(&ActorView, &Children)>,
    rings: Query<&Ring>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let pulse = 0.75 + 0.25 * (time.elapsed_secs() * 4.0).sin();
    for (view, kids) in &actors {
        for k in kids.iter() {
            let Ok(r) = rings.get(k) else { continue };
            let Some(m) = mats.get_mut(&r.0) else {
                continue;
            };
            let lin = |r: f32, g: f32, b: f32| Color::linear_rgb(r * pulse, g * pulse, b * pulse);
            let c = match view.mark {
                Mark::None => Color::NONE,
                Mark::Active => lin(1.0, 0.75, 0.25),
                Mark::Target => lin(1.0, 0.1, 0.06),
                Mark::Friend => lin(0.2, 1.0, 0.3),
                Mark::Selected => Color::linear_rgb(0.5, 0.45, 0.35),
            };
            if m.base_color != c {
                m.base_color = c;
            }
        }
    }
}

/// Built scenery models stop casting shadows (a pillar's shadow would slice the stage in two).
fn unshadow(
    mut commands: Commands,
    parts: Query<(Entity, &crate::wow::ModelPart), Added<crate::wow::ModelPart>>,
    quiet: Query<(), With<NoShadows>>,
) {
    for (e, p) in &parts {
        if quiet.get(p.owner).is_ok() {
            commands.entity(e).insert(bevy::light::NotShadowCaster);
        }
    }
}

/// A darkening at the screen's edge, built once as a radial gradient.
pub fn vignette_image() -> Image {
    let n = 256u32;
    let mut px = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let dx = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let dy = (y as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let d = (dx * dx * 0.8 + dy * dy * 1.1).sqrt();
            let t = ((d - 0.55) / 0.6).clamp(0.0, 1.0);
            let a = (t * t * (3.0 - 2.0 * t) * 235.0) as u8;
            px.extend_from_slice(&[0, 0, 0, a]);
        }
    }
    Image::new(
        bevy::render::render_resource::Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        px,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

pub fn spawn_vignette(commands: &mut Commands, images: &mut Assets<Image>) {
    let img = images.add(vignette_image());
    commands.spawn((
        ImageNode::new(img),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        GlobalZIndex(-1),
        Pickable::IGNORE,
        Vignette,
        Scoped(Screen::Dungeon),
    ));
}

/// How a region's halls are dressed.
struct Theme {
    floor: &'static str,
    wall: &'static str,
    trim: &'static str,
    pillar: &'static str,
    ceiling: &'static str,
    tint: Color,
    /// Wall furnishings between pillars: (model, height, height above the floor).
    props: &'static [(&'static str, f32, f32)],
    clutter: &'static str,
    sconce: Color,
    fog: Color,
}

fn theme(region: Region) -> Theme {
    match region {
        Region::Crypts => Theme {
            floor: "Dungeons\\Textures\\floor\\JACRYPTFLOOR01",
            wall: "Dungeons\\Textures\\brick\\JACRYPTBRICK01",
            trim: "Dungeons\\Textures\\trim\\JACRYPTBRICK02",
            pillar: "Dungeons\\Textures\\roof\\JACRYPTPILLAR",
            ceiling: "Dungeons\\Textures\\roof\\JACRYPTCEILING",
            tint: Color::srgb(0.72, 0.68, 0.66),
            props: &[
                ("World\\Lordaeron\\Scholomance\\PassiveDoodads\\Candles\\Scholme_Candelabra.m2", 2.2, -0.2),
                ("World\\Generic\\PassiveDoodads\\Skeletons\\ChainedSkeleton01.m2", 1.9, 1.2),
                ("World\\Lordaeron\\Scholomance\\PassiveDoodads\\Tapestries\\Scholme_tapestrySkull.m2", 3.0, 1.8),
            ],
            clutter: "World\\Azeroth\\TheBlastedlands\\PassiveDoodads\\Bones\\BlastedLandsBonePile02.m2",
            sconce: Color::srgb(1.0, 0.62, 0.3),
            fog: Color::srgb(0.01, 0.008, 0.012),
        },
        Region::Warrens => Theme {
            floor: "Dungeons\\Textures\\Deadmine\\STU_MIDLEVELFLOOR_01",
            wall: "Dungeons\\Textures\\Deadmine\\STU_LUMBERWALL_01",
            trim: "Dungeons\\Textures\\Deadmine\\STU_DEADMINELOG",
            pillar: "Dungeons\\Textures\\Deadmine\\STU_MINEBEAM_01",
            ceiling: "Dungeons\\Textures\\Deadmine\\STU_CAVECEILING_02",
            tint: Color::srgb(0.75, 0.68, 0.6),
            props: &[
                ("World\\Dungeon\\Goldmine\\PassiveDoodads\\CaveLanterns\\CaveMineLantern01.m2", 1.0, 2.2),
                // Not the Elwynn mine cart: it is authored tipped over with its spill, and hangs
                // tilted across the stage at this scale.
                ("World\\Generic\\PassiveDoodads\\Barrel\\Barrel01.m2", 1.1, 0.0),
                ("World\\Dungeon\\Goldmine\\PassiveDoodads\\CaveMineKobolds\\CaveMineKobold02.m2", 1.4, 0.0),
            ],
            clutter: "World\\Azeroth\\Karazahn\\PassiveDoodads\\Rubble\\KarazahnRockRubble01.m2",
            sconce: Color::srgb(1.0, 0.7, 0.35),
            fog: Color::srgb(0.02, 0.012, 0.006),
        },
        Region::Grotto => Theme {
            floor: "Dungeons\\Textures\\BlackFathom\\BM_BF_INTFLOOR01",
            wall: "Dungeons\\Textures\\BlackFathom\\BM_BF_STONEWALL01",
            trim: "Dungeons\\Textures\\BlackFathom\\BM_BF_INTTRIMS02",
            pillar: "Dungeons\\Textures\\BlackFathom\\BM_BF_COLUMNS01",
            ceiling: "Dungeons\\Textures\\BlackFathom\\BM_BF_CAVECEILING01",
            tint: Color::srgb(0.6, 0.72, 0.72),
            props: &[
                ("World\\Azeroth\\Duskwood\\PassiveDoodads\\Detail\\DuskwoodSeaWeed01\\DuskwoodSeaWeed01.m2", 1.6, 0.0),
                ("World\\Azeroth\\Westfall\\PassiveDoodads\\SunkenAnchor\\SunkenAnchor.m2", 2.0, 0.0),
                ("World\\Azeroth\\Stranglethorn\\PassiveDoodads\\SerpentStatue02\\SerpentStatue02.m2", 2.4, 0.0),
            ],
            clutter: "World\\Azeroth\\Elwynn\\PassiveDoodads\\Detail\\ElwynnSeaWeed01\\ElwynnSeaWeed01.m2",
            sconce: Color::srgb(0.35, 0.85, 0.8),
            fog: Color::srgb(0.005, 0.015, 0.02),
        },
    }
}
