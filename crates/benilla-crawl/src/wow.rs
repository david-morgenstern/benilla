//! The bridge to the player's install: the DBC catalogs, and models assembled from benilla's M2
//! assets with stock Bevy skinning and `StandardMaterial`, so the crawler draws WoW creatures,
//! armoured characters and props without the client's world pipeline.

use std::collections::HashMap;
use std::sync::Mutex;

use bevy::animation::{AnimatedBy, AnimationPlayer};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use benilla_assets::{
    bone_target_id, m2_url, skin_url, texture_url, wmo_url, M2Model, ModelAnimations, WmoModel,
};
use benilla_formats::{
    AnimDataCatalog, BlpTexels, Chain, CharSections, CharSkinSlot, CharacterGeosets, ChrRaces,
    CreatureCatalog, EquipGeosets, ItemDisplayCatalog, ModelBlend,
};

/// Everything read from the install once at boot.
#[derive(Resource)]
pub struct WowData {
    pub chain: Mutex<Chain>,
    pub creatures: CreatureCatalog,
    pub sections: CharSections,
    pub geosets: CharacterGeosets,
    pub items: ItemDisplayCatalog,
    pub races: ChrRaces,
    pub anim_data: AnimDataCatalog,
}

impl WowData {
    pub fn load(data_dir: &std::path::Path) -> anyhow::Result<WowData> {
        let mut chain = benilla_formats::open_chain(data_dir)?;
        let creatures = benilla_formats::load_creature_catalog(&mut chain)?;
        let sections = CharSections::load(&mut chain)?;
        let geosets = CharacterGeosets::load(&mut chain)?;
        let items = benilla_formats::load_item_display_catalog(&mut chain)?;
        let races = benilla_formats::load_chr_races(&mut chain)?;
        let anim_data = benilla_formats::load_anim_data_catalog(&mut chain)?;
        Ok(WowData {
            chain: Mutex::new(chain),
            creatures,
            sections,
            geosets,
            items,
            races,
            anim_data,
        })
    }

    /// Decode one BLP from the chain to a plain RGBA image, for the UI.
    pub fn rgba_image(&self, path: &str) -> Option<Image> {
        let mut chain = self.chain.lock().ok()?;
        let name = if path.to_ascii_lowercase().ends_with(".blp") {
            path.to_owned()
        } else {
            format!("{path}.blp")
        };
        let bytes = chain.read_file(&name).ok()?;
        let (w, h, rgba) = benilla_formats::blp_to_rgba(&bytes).ok()?;
        Some(Image::new(
            Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        ))
    }
}

/// The `Character\<folder>` of a playable race.
pub fn race_folder(race: u8) -> Option<&'static str> {
    Some(match race {
        1 => "Human",
        2 => "Orc",
        3 => "Dwarf",
        4 => "NightElf",
        5 => "Scourge",
        6 => "Tauren",
        7 => "Gnome",
        8 => "Troll",
        _ => return None,
    })
}

/// A character's customisation and worn gear, by `ItemDisplayInfo` id (0 = none).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CharLook {
    pub race: u8,
    pub sex: u8,
    pub skin: u8,
    pub face: u8,
    pub hair_style: u8,
    pub hair_color: u8,
    pub facial_hair: u8,
    /// Shirt, chest, belt, pants, boots, wrist, gloves, tabard.
    pub body: [u32; 8],
    pub helm: u32,
    pub shoulders: u32,
    pub main_hand: u32,
    pub off_hand: u32,
    /// The off hand holds a shield (attaches to the forearm) rather than a weapon.
    pub shield: bool,
}

#[derive(Clone, Debug)]
pub enum ModelSource {
    Creature(u32),
    Character(Box<CharLook>),
    /// A doodad or item M2 by path, with an optional texture for its object slot.
    Prop(String, Option<String>),
    /// A WMO building by path, with its first doodad set and fixture lights.
    Wmo(String),
}

/// Ask for a model on this entity; it is built under it once the M2 loads.
#[derive(Component, Clone, Debug)]
pub struct WowModel {
    pub source: ModelSource,
    /// Scale the model so its bounds stand this tall (yards); `None` keeps the authored size.
    pub height: Option<f32>,
}

/// The animation a built model should play: `then` follows a one-shot `id`.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct PlayAnim {
    pub id: u16,
    pub looping: bool,
    pub then: Option<u16>,
}

impl PlayAnim {
    pub fn looped(id: u16) -> Self {
        PlayAnim {
            id,
            looping: true,
            then: None,
        }
    }

    pub fn once_then(id: u16, then: u16) -> Self {
        PlayAnim {
            id,
            looping: false,
            then: Some(then),
        }
    }

    /// A one-shot that holds its last frame (a death).
    pub fn hold(id: u16) -> Self {
        PlayAnim {
            id,
            looping: false,
            then: None,
        }
    }
}

/// Set on a model entity once its parts are spawned.
#[derive(Component)]
pub struct Rig {
    /// The child that carries the scale, the `AnimationPlayer` and the joints.
    pub root: Entity,
    pub joints: Vec<Entity>,
    pub anims: Option<ModelAnimations>,
    /// The model's standing height after scaling, yards.
    pub height: f32,
    /// The head key bone's joint, for framing a portrait.
    pub head: Option<Entity>,
    playing: Option<(u16, bool)>,
}

#[derive(Component)]
struct Pending {
    model: Handle<M2Model>,
    /// Attached item models waiting on their own loads: (handle, attachment id, texture).
    attachments: Vec<(Handle<M2Model>, u16, Option<String>)>,
}

/// Marks a mesh part of a built model, for tinting (selection, death fade).
#[derive(Component)]
pub struct ModelPart {
    pub owner: Entity,
    pub material: Handle<StandardMaterial>,
}

pub struct WowPlugin;

impl Plugin for WowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BodyCache>().add_systems(
            Update,
            (start_loads, build_models, build_wmos, drive_anims, reskin).chain(),
        );
    }
}

/// Frames a new skinned part waits before its skin is registered again.
const RESKIN_FRAMES: u8 = 3;

/// A skinned part whose skin is registered again once its joints have settled.
///
/// Bevy writes a skin's joint matrices when the mesh first becomes visible, falling back to the
/// identity for a joint it cannot resolve yet, and afterwards rewrites a joint only when its
/// `GlobalTransform` changes. A joint that never moves keeps the identity, and its vertices drop
/// to the world origin while the rest follow the model: a beam stretched across the stage.
/// Touching the `SkinnedMesh` makes Bevy drop the skin and build it afresh from settled joints.
#[derive(Component)]
struct Reskin(u8);

fn reskin(
    mut commands: Commands,
    mut q: Query<(Entity, &mut Reskin, &mut SkinnedMesh, &mut Mesh3d)>,
) {
    for (e, mut r, mut skin, mut mesh) in &mut q {
        if r.0 > 0 {
            r.0 -= 1;
            continue;
        }
        skin.set_changed();
        mesh.set_changed();
        commands.entity(e).remove::<Reskin>();
    }
}

/// Composited body skins, one per look.
#[derive(Resource, Default)]
struct BodyCache(HashMap<CharLook, Option<Handle<Image>>>);

fn model_path(data: &WowData, source: &ModelSource) -> Option<String> {
    match source {
        ModelSource::Creature(display) => data.creatures.model(*display).map(|m| m.model_path),
        ModelSource::Character(look) => {
            let race = race_folder(look.race)?;
            let sex = if look.sex == 1 { "Female" } else { "Male" };
            Some(format!("Character\\{race}\\{sex}\\{race}{sex}.m2"))
        }
        ModelSource::Prop(p, _) | ModelSource::Wmo(p) => Some(p.clone()),
    }
}

#[derive(Component)]
struct PendingWmo(Handle<WmoModel>);

#[allow(clippy::type_complexity)]
fn start_loads(
    mut commands: Commands,
    data: Option<Res<WowData>>,
    asset_server: Res<AssetServer>,
    new: Query<(Entity, &WowModel), (Without<Pending>, Without<PendingWmo>, Without<Rig>)>,
) {
    let Some(data) = data else {
        return;
    };
    for (e, req) in &new {
        let Some(path) = model_path(&data, &req.source) else {
            warn!("crawl: no model for {:?}", req.source);
            commands.entity(e).remove::<WowModel>();
            continue;
        };
        if let ModelSource::Wmo(p) = &req.source {
            commands
                .entity(e)
                .insert(PendingWmo(asset_server.load(wmo_url(p))));
            continue;
        }
        let mut attachments = Vec::new();
        if let ModelSource::Character(look) = &req.source {
            attachments = item_attachments(&data, look, &asset_server);
        }
        commands.entity(e).insert(Pending {
            model: asset_server.load(m2_url(&path)),
            attachments,
        });
    }
}

/// The held and worn item models a look hangs on its attachment points.
fn item_attachments(
    data: &WowData,
    look: &CharLook,
    asset_server: &AssetServer,
) -> Vec<(Handle<M2Model>, u16, Option<String>)> {
    let mut out = Vec::new();
    let mut add = |display: u32, dir: &str, col: usize, attach: u16, helm: bool| {
        let Some(d) = data.items.get(display) else {
            return;
        };
        let Some(model) = d.model[col].clone() else {
            return;
        };
        let model = if helm {
            let Some(prefix) = data.races.client_prefix(u32::from(look.race)) else {
                return;
            };
            let letter = if look.sex == 1 { 'F' } else { 'M' };
            let stem = model.strip_suffix(".m2").unwrap_or(&model).to_owned();
            format!("{stem}_{prefix}{letter}.m2")
        } else {
            model
        };
        let dir = format!("Item\\ObjectComponents\\{dir}");
        let tex = d.model_texture[col].clone().map(|t| format!("{dir}\\{t}"));
        out.push((
            asset_server.load(m2_url(&format!("{dir}\\{model}"))),
            attach,
            tex,
        ));
    };
    if look.shoulders != 0 {
        add(look.shoulders, "Shoulder", 0, 6, false);
        add(look.shoulders, "Shoulder", 1, 5, false);
    }
    if look.helm != 0 {
        add(look.helm, "Head", 0, 11, true);
    }
    if look.main_hand != 0 {
        add(look.main_hand, "Weapon", 0, 1, false);
    }
    if look.off_hand != 0 {
        if look.shield {
            add(look.off_hand, "Shield", 0, 0, false);
        } else {
            add(look.off_hand, "Weapon", 0, 2, false);
        }
    }
    out
}

/// The skinned Bevy mesh for one submesh: benilla's geometry with Bevy's own joint attributes.
fn skinned_mesh(sub: &benilla_formats::RenderSubmesh, skinned: bool) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let pos: Vec<[f32; 3]> = sub
        .positions
        .iter()
        .map(|p| benilla_assets::coords::wow_to_bevy(*p).to_array())
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    if sub.normals.len() == sub.positions.len() {
        let n: Vec<[f32; 3]> = sub
            .normals
            .iter()
            .map(|p| {
                benilla_assets::coords::wow_to_bevy(*p)
                    .normalize_or_zero()
                    .to_array()
            })
            .collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, n);
    }
    if sub.uvs.len() == sub.positions.len() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, sub.uvs.clone());
    }
    if skinned && sub.joints.len() == sub.positions.len() {
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_JOINT_INDEX,
            VertexAttributeValues::Uint16x4(sub.joints.clone()),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, sub.weights.clone());
    }
    mesh.insert_indices(Indices::U32(sub.indices.clone()));
    if sub.normals.len() != sub.positions.len() {
        mesh.compute_normals();
    }
    mesh
}

fn material_for(
    blend: ModelBlend,
    additive: bool,
    two_sided: bool,
    emissive: bool,
    texture: Option<Handle<Image>>,
) -> Option<StandardMaterial> {
    let alpha_mode = match blend {
        ModelBlend::Opaque => AlphaMode::Opaque,
        ModelBlend::AlphaTest => AlphaMode::Mask(0.5),
        ModelBlend::Blend if additive => AlphaMode::Add,
        ModelBlend::Blend => AlphaMode::Blend,
        // The armour sheen and its kin multiply over the base; dropped.
        ModelBlend::Mod | ModelBlend::Mod2x => return None,
    };
    Some(StandardMaterial {
        base_color_texture: texture,
        alpha_mode,
        double_sided: two_sided,
        cull_mode: if two_sided {
            None
        } else {
            Some(bevy::render::render_resource::Face::Back)
        },
        unlit: emissive || additive,
        perceptual_roughness: 0.92,
        reflectance: 0.08,
        ..default()
    })
}

/// A composited body skin as a Bevy image.
fn body_image(data: &WowData, look: &CharLook) -> Option<Image> {
    let displays: Vec<Option<&benilla_formats::ItemDisplay>> =
        look.body.iter().map(|id| data.items.get(*id)).collect();
    let equipment: [Option<&benilla_formats::ItemDisplay>; 8] =
        std::array::from_fn(|i| displays[i]);
    let mut chain = data.chain.lock().ok()?;
    let atlas = data
        .sections
        .composite_body(
            &mut chain,
            look.race,
            look.sex,
            look.skin,
            look.face,
            look.facial_hair,
            look.hair_style,
            look.hair_color,
            equipment,
            None,
            false,
        )
        .map_err(|e| warn!("crawl: body composite failed: {e:#}"))
        .ok()??;
    let format = match atlas.texels {
        BlpTexels::Rgba8Unorm => TextureFormat::Rgba8UnormSrgb,
        BlpTexels::Bc1 => TextureFormat::Bc1RgbaUnormSrgb,
        BlpTexels::Bc2 => TextureFormat::Bc2RgbaUnormSrgb,
        BlpTexels::Bc3 => TextureFormat::Bc3RgbaUnormSrgb,
    };
    let data0 = atlas.mips.first()?.clone();
    Some(Image::new(
        Extent3d {
            width: atlas.width,
            height: atlas.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data0,
        format,
        RenderAssetUsages::default(),
    ))
}

fn equip_geosets(data: &WowData, look: &CharLook) -> EquipGeosets {
    let mut eq = EquipGeosets::default();
    for (i, id) in look.body.iter().enumerate() {
        if let Some(d) = data.items.get(*id) {
            eq.bodyslots[i] = Some(d.geoset_groups);
        }
    }
    if let Some(d) = data.items.get(look.helm) {
        eq.helm_vis = Some(d.helmet_vis);
    }
    let displays: Vec<Option<&benilla_formats::ItemDisplay>> =
        look.body.iter().map(|id| data.items.get(*id)).collect();
    let equipment: [Option<&benilla_formats::ItemDisplay>; 8] =
        std::array::from_fn(|i| displays[i]);
    eq.forearm_dressed = benilla_formats::forearm_dressed(&equipment);
    eq
}

#[allow(clippy::type_complexity)]
fn build_models(
    mut commands: Commands,
    data: Option<Res<WowData>>,
    asset_server: Res<AssetServer>,
    m2s: Res<Assets<M2Model>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut bodies: ResMut<BodyCache>,
    pending: Query<(Entity, &WowModel, &Pending)>,
) {
    let Some(data) = data else {
        return;
    };
    for (e, req, p) in &pending {
        let Some(m) = m2s.get(&p.model) else {
            if let Some(bevy::asset::LoadState::Failed(err)) = asset_server.get_load_state(&p.model)
            {
                warn!("crawl: model failed to load for {:?}: {err}", req.source);
                commands.entity(e).remove::<(Pending, WowModel)>();
            }
            continue;
        };
        // Hold off until every attachment has resolved one way or the other.
        let attachments_ready = p.attachments.iter().all(|(h, _, _)| {
            m2s.get(h).is_some()
                || matches!(
                    asset_server.get_load_state(h),
                    Some(bevy::asset::LoadState::Failed(_))
                )
        });
        if !attachments_ready {
            continue;
        }

        // Per-source texture resolution.
        let mut skins: [Option<Handle<Image>>; 3] = [None, None, None];
        let mut body: Option<Handle<Image>> = None;
        let mut hair: Option<Handle<Image>> = None;
        let mut visible: Option<benilla_formats::VisibleGeosets> = None;
        let mut object_tex: Option<Handle<Image>> = None;
        match &req.source {
            ModelSource::Creature(display) => {
                if let Some(cm) = data.creatures.model(*display) {
                    let dir = cm
                        .model_path
                        .rsplit_once('\\')
                        .map_or(String::new(), |(d, _)| d.to_owned());
                    for (i, t) in cm.textures.iter().enumerate() {
                        if let Some(name) = t {
                            skins[i] = Some(asset_server.load(skin_url(&dir, name)));
                        }
                    }
                }
            }
            ModelSource::Character(look) => {
                body = bodies
                    .0
                    .entry((**look).clone())
                    .or_insert_with(|| body_image(&data, look).map(|i| images.add(i)))
                    .clone();
                if let Some(path) = data.sections.hair_mesh_texture(
                    look.race,
                    look.sex,
                    look.hair_style,
                    look.hair_color,
                ) {
                    hair = Some(asset_server.load(texture_url(path, (true, true))));
                }
                let eq = equip_geosets(&data, look);
                visible = Some(data.geosets.visible_geosets(
                    look.race,
                    look.sex,
                    look.hair_style,
                    look.facial_hair,
                    &eq,
                ));
            }
            ModelSource::Wmo(_) => {}
            ModelSource::Prop(_, tex) => {
                object_tex = tex
                    .as_ref()
                    .map(|t| asset_server.load(texture_url(t, (true, true))));
            }
        }

        // Scale from the authored bounds.
        let authored_h = m
            .bounds
            .as_ref()
            .map(|b| (b.bbox_max[2] - b.bbox_min[2]).abs())
            .filter(|h| *h > 0.05)
            .unwrap_or(2.0);
        // A prop is sized by its height, but no longer than `PROP_ASPECT` heights: a low, long
        // doodad (a coffin, rubble) sized by its height alone grows into a beam across the stage.
        const PROP_ASPECT: f32 = 1.6;
        let authored_h = match (&req.source, m.bounds.as_ref()) {
            (ModelSource::Prop(..), Some(b)) => {
                let w = (b.bbox_max[0] - b.bbox_min[0])
                    .abs()
                    .max((b.bbox_max[1] - b.bbox_min[1]).abs());
                authored_h.max(w / PROP_ASPECT)
            }
            _ => authored_h,
        };
        let scale = req.height.map_or(1.0, |h| h / authored_h);

        let root = commands
            .spawn((
                Transform::from_scale(Vec3::splat(scale)),
                Visibility::default(),
            ))
            .id();
        commands.entity(e).add_child(root);
        let joints = spawn_joints(&mut commands, root, &m.skeleton);
        let anims = m.animations.clone();
        if let Some(a) = &anims {
            commands.entity(root).insert((
                AnimationPlayer::default(),
                AnimationGraphHandle(a.graph.clone()),
            ));
            for (i, j) in joints.iter().enumerate() {
                commands
                    .entity(*j)
                    .insert((bone_target_id(i as u16), AnimatedBy(root)));
            }
        }
        let skinned = !joints.is_empty();
        for sub in &m.submeshes {
            if sub.billboard.is_some() {
                continue;
            }
            if let Some(v) = &visible {
                if !v.contains(&sub.geoset_id) {
                    continue;
                }
            }
            let texture = match (sub.skin_slot, sub.char_slot) {
                (Some(slot), _) => skins.get(usize::from(slot)).cloned().flatten(),
                (_, Some(CharSkinSlot::Body)) => body.clone(),
                (_, Some(CharSkinSlot::Hair)) => hair.clone(),
                (_, Some(CharSkinSlot::Object)) => object_tex.clone(),
                (_, Some(CharSkinSlot::SkinExtra)) => body.clone(),
                _ => sub.texture.clone(),
            };
            let Some(mat) = material_for(
                sub.blend,
                sub.additive,
                sub.two_sided,
                sub.emissive,
                texture,
            ) else {
                continue;
            };
            let mat = materials.add(mat);
            let mesh = meshes.add(skinned_mesh(&sub.geometry, skinned));
            let mut part = commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(mat.clone()),
                Transform::default(),
                ModelPart {
                    owner: e,
                    material: mat,
                },
                bevy::camera::visibility::NoFrustumCulling,
            ));
            if skinned {
                part.insert((
                    SkinnedMesh {
                        inverse_bindposes: m.inverse_bindposes.clone(),
                        joints: joints.clone(),
                    },
                    Reskin(RESKIN_FRAMES),
                ));
            }
            let part = part.id();
            commands.entity(root).add_child(part);
        }
        // Items on their attachment points.
        for (handle, attach_id, tex) in &p.attachments {
            let Some(item) = m2s.get(handle) else {
                continue;
            };
            let Some(att) = m.attachments.iter().find(|a| a.id == *attach_id) else {
                continue;
            };
            let Some(&bone) = joints.get(usize::from(att.bone)) else {
                continue;
            };
            let holder = commands
                .spawn((
                    Transform::from_translation(att.offset),
                    Visibility::default(),
                ))
                .id();
            commands.entity(bone).add_child(holder);
            let item_tex: Option<Handle<Image>> = tex
                .as_ref()
                .map(|t| asset_server.load(texture_url(t, (true, true))));
            let item_joints = spawn_joints(&mut commands, holder, &item.skeleton);
            let item_skinned = !item_joints.is_empty();
            for sub in &item.submeshes {
                if sub.billboard.is_some() {
                    continue;
                }
                let texture = match sub.char_slot {
                    Some(CharSkinSlot::Object) => item_tex.clone(),
                    _ => sub.texture.clone().or_else(|| item_tex.clone()),
                };
                let Some(mat) = material_for(
                    sub.blend,
                    sub.additive,
                    sub.two_sided,
                    sub.emissive,
                    texture,
                ) else {
                    continue;
                };
                let mat = materials.add(mat);
                let mesh = meshes.add(skinned_mesh(&sub.geometry, item_skinned));
                let mut part = commands.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(mat.clone()),
                    Transform::default(),
                    ModelPart {
                        owner: e,
                        material: mat,
                    },
                    bevy::camera::visibility::NoFrustumCulling,
                ));
                if item_skinned {
                    part.insert((
                        SkinnedMesh {
                            inverse_bindposes: item.inverse_bindposes.clone(),
                            joints: item_joints.clone(),
                        },
                        Reskin(RESKIN_FRAMES),
                    ));
                }
                let part = part.id();
                commands.entity(holder).add_child(part);
            }
        }
        debug!("crawl: built model on {e:?}");
        let head = m
            .skeleton
            .head_bone
            .and_then(|b| joints.get(usize::from(b)).copied());
        commands.entity(e).remove::<Pending>().insert(Rig {
            root,
            head,
            joints,
            anims,
            height: authored_h * scale,
            playing: None,
        });
    }
}

fn spawn_joints(
    commands: &mut Commands,
    root: Entity,
    skeleton: &benilla_assets::ModelSkeleton,
) -> Vec<Entity> {
    let joints: Vec<Entity> = skeleton
        .joints
        .iter()
        .map(|j| {
            commands
                .spawn((
                    Transform::from_translation(j.local_translation),
                    Visibility::default(),
                ))
                .id()
        })
        .collect();
    for (i, j) in skeleton.joints.iter().enumerate() {
        let parent = usize::try_from(j.parent)
            .ok()
            .and_then(|p| joints.get(p).copied())
            .unwrap_or(root);
        commands.entity(parent).add_child(joints[i]);
    }
    joints
}

/// Resolve a requested animation id to one the model has, through the AnimationData fallbacks.
fn find_clip<'a>(
    anims: &'a ModelAnimations,
    data: &AnimDataCatalog,
    id: u16,
) -> Option<&'a benilla_assets::AnimClip> {
    let mut cur = id;
    for _ in 0..8 {
        let resolved = anims.resolve(cur, data).id;
        if let Some(c) = anims.find(resolved) {
            return Some(c);
        }
        if let Some(c) = anims.find(cur) {
            return Some(c);
        }
        match data.fallback(cur) {
            Some(f) if f != cur => cur = f,
            _ => break,
        }
    }
    anims.find(0)
}

fn drive_anims(
    data: Option<Res<WowData>>,
    mut models: Query<(&mut Rig, &mut PlayAnim)>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Some(data) = data else {
        return;
    };
    for (mut rig, mut want) in &mut models {
        let Some(anims) = rig.anims.as_ref() else {
            continue;
        };
        let Ok(mut player) = players.get_mut(rig.root) else {
            continue;
        };
        // A finished one-shot rolls on to its follow-up.
        if let (Some((_, false)), Some(then)) = (rig.playing, want.then) {
            if player.all_finished() {
                *want = PlayAnim::looped(then);
            }
        }
        let key = (want.id, want.looping);
        if rig.playing == Some(key) && !want.is_changed() {
            continue;
        }
        let Some(clip) = find_clip(anims, &data.anim_data, want.id) else {
            continue;
        };
        let node = clip.node;
        player.stop_all();
        let active = player.play(node);
        if want.looping {
            active.repeat();
        }
        rig.playing = Some(key);
    }
}

/// WoW's model-space quaternion in Bevy's basis; the axis swap is a proper rotation, so the
/// vector part maps like a position.
fn wow_quat(q: [f32; 4]) -> Quat {
    let v = benilla_assets::coords::wow_to_bevy([q[0], q[1], q[2]]);
    Quat::from_xyzw(v.x, v.y, v.z, q[3]).normalize()
}

fn build_wmos(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    wmos: Res<Assets<WmoModel>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    pending: Query<(Entity, &WowModel, &PendingWmo)>,
) {
    for (e, req, p) in &pending {
        let Some(w) = wmos.get(&p.0) else {
            if let Some(bevy::asset::LoadState::Failed(err)) = asset_server.get_load_state(&p.0) {
                warn!("crawl: building failed to load for {:?}: {err}", req.source);
                commands.entity(e).remove::<(PendingWmo, WowModel)>();
            }
            continue;
        };
        // Centre the building on its footprint, its floor on the ground.
        let offset = w.collision_bounds.map_or(Vec3::ZERO, |(lo, hi)| {
            let (a, b) = (
                benilla_assets::coords::wow_to_bevy(lo),
                benilla_assets::coords::wow_to_bevy(hi),
            );
            let (mn, mx) = (a.min(b), a.max(b));
            Vec3::new(-(mn.x + mx.x) / 2.0, -mn.y, -(mn.z + mx.z) / 2.0)
        });
        let root = commands
            .spawn((Transform::from_translation(offset), Visibility::default()))
            .id();
        commands.entity(e).add_child(root);
        for sub in &w.submeshes {
            let Some(mat) = material_for(
                sub.blend,
                sub.additive,
                sub.two_sided,
                sub.emissive,
                sub.texture.clone(),
            ) else {
                continue;
            };
            let mut mesh = skinned_mesh(&sub.geometry, false);
            // Interiors carry their baked light in the vertex colours.
            let g = &sub.geometry;
            if sub.interior && g.vertex_colors.len() == g.positions.len() {
                let cols: Vec<[f32; 4]> = g
                    .vertex_colors
                    .iter()
                    .map(|c| [c[0] * 1.6, c[1] * 1.6, c[2] * 1.6, 1.0])
                    .collect();
                mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, cols);
            }
            let mat = materials.add(mat);
            let part = commands
                .spawn((
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(mat.clone()),
                    Transform::default(),
                    ModelPart {
                        owner: e,
                        material: mat,
                    },
                ))
                .id();
            commands.entity(root).add_child(part);
        }
        // Set 0, the doodads every placement shows.
        if let Some(set) = w.doodad_sets.first() {
            let start = set.start as usize;
            for d in w.doodads.iter().skip(start).take(set.count as usize) {
                if d.model.is_empty() {
                    continue;
                }
                let child = commands
                    .spawn((
                        Transform {
                            translation: benilla_assets::coords::wow_to_bevy(d.position),
                            rotation: wow_quat(d.orientation),
                            scale: Vec3::splat(d.scale),
                        },
                        Visibility::default(),
                        WowModel {
                            source: ModelSource::Prop(d.model.clone(), None),
                            height: None,
                        },
                    ))
                    .id();
                commands.entity(root).add_child(child);
            }
        }
        // A handful of its fixture lights, warm through the windows.
        for l in w.lights.iter().filter(|l| l.is_omni()).take(4) {
            let light = commands
                .spawn((
                    PointLight {
                        color: Color::srgb(l.color[0], l.color[1], l.color[2]),
                        intensity: 40_000.0 * l.intensity.clamp(0.2, 2.0),
                        range: l.attenuation_end.clamp(4.0, 14.0),
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(benilla_assets::coords::wow_to_bevy(l.position)),
                ))
                .id();
            commands.entity(root).add_child(light);
        }
        let height = w
            .collision_bounds
            .map_or(8.0, |(lo, hi)| (hi[2] - lo[2]).abs());
        commands.entity(e).remove::<PendingWmo>().insert(Rig {
            root,
            head: None,
            joints: Vec::new(),
            anims: None,
            height,
            playing: None,
        });
    }
}
