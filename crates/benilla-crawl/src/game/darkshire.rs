//! Darkshire off the player's own map: the Azeroth tiles around the town hall, read once, their
//! ground baked to plain textures, and every building and doodad where the map places it. The
//! scene sits relative to the town hall, so the hamlet's numbers stay small.

use std::collections::HashMap;

use benilla_assets::coords::{placement_rotation, wow_to_bevy};
use benilla_formats::{ChunkMesh, Doodad, WmoInstance};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::wow::{ModelSource, WowData, WowModel};

/// The Darkshire town hall's placement, WoW world coords: the scene's origin.
pub const HALL_WOW: [f32; 3] = [-10554.281, -1159.9063, 29.070576];
/// Map kept around the hall, in yards: terrain to the far edge, buildings and doodads nearer.
const TERRAIN_RADIUS: f32 = 300.0;
const DOODAD_RADIUS: f32 = 220.0;
/// Ground texels per chunk (33⅓ yd) near the hall and beyond `NEAR_CHUNKS`.
const NEAR_TEXELS: usize = 256;
const FAR_TEXELS: usize = 64;
const NEAR_CHUNKS: f32 = 140.0;
/// Each ground layer repeats this many times across a chunk, as in the reference's terrain.
const LAYER_TILES: usize = 8;
/// How much an MCSH-shadowed texel keeps of its colour.
const SHADOW_KEEP: f32 = 0.62;

/// Bevy-space position of the hall's placement; the scene subtracts it.
pub fn origin() -> Vec3 {
    wow_to_bevy(HALL_WOW)
}

/// A WoW world position as a scene position.
pub fn scene(p: [f32; 3]) -> Vec3 {
    wow_to_bevy(p) - origin()
}

/// The town's map data, read from the install the first time the hamlet opens.
#[derive(Resource)]
pub struct DarkshireMap {
    chunks: Vec<ChunkMesh>,
    doodads: Vec<Doodad>,
    pub wmos: Vec<WmoInstance>,
}

fn flat_dist(p: [f32; 3]) -> f32 {
    ((p[0] - HALL_WOW[0]).powi(2) + (p[1] - HALL_WOW[1]).powi(2)).sqrt()
}

impl DarkshireMap {
    pub fn load(data: &WowData) -> Option<DarkshireMap> {
        let tiles = {
            let mut chain = data.chain.lock().ok()?;
            benilla_formats::load_tiles_around(&mut chain, "Azeroth", HALL_WOW[0], HALL_WOW[1], 1)
                .ok()?
        };
        let mut chunks = Vec::new();
        let mut doodads = Vec::new();
        let mut wmos: Vec<WmoInstance> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (_, t) in tiles {
            for c in t.chunks {
                let Some(p) = c.positions.get(c.positions.len() / 2) else {
                    continue;
                };
                if flat_dist(*p) < TERRAIN_RADIUS {
                    chunks.push(c);
                }
            }
            for d in t.doodads {
                if flat_dist(d.position) < DOODAD_RADIUS && seen.insert(d.unique_id) {
                    doodads.push(d);
                }
            }
            for w in t.wmos {
                if flat_dist(w.position) < DOODAD_RADIUS + 60.0 && seen.insert(w.unique_id) {
                    wmos.push(w);
                }
            }
        }
        info!(
            "darkshire: {} chunks, {} doodads, {} buildings",
            chunks.len(),
            doodads.len(),
            wmos.len()
        );
        Some(DarkshireMap {
            chunks,
            doodads,
            wmos,
        })
    }

    /// The ground's height under a scene position, from the map's own triangles.
    pub fn ground_at(&self, at: Vec3) -> Option<f32> {
        let wow = benilla_assets::coords::bevy_to_wow(at + origin());
        benilla_formats::terrain_height_at(&self.chunks, wow).map(|z| z - HALL_WOW[2])
    }
}

/// A ground texture, box-filtered down to `size`² texels for one repeat.
struct Swatch {
    size: usize,
    rgba: Vec<u8>,
}

fn swatch(full: &Image, size: usize) -> Option<Swatch> {
    let (w, h) = (full.width() as usize, full.height() as usize);
    let src = full.data.as_ref()?;
    if w == 0 || h == 0 || src.len() < w * h * 4 {
        return None;
    }
    let mut rgba = vec![0u8; size * size * 4];
    for y in 0..size {
        let (y0, y1) = (y * h / size, ((y + 1) * h / size).max(y * h / size + 1));
        for x in 0..size {
            let (x0, x1) = (x * w / size, ((x + 1) * w / size).max(x * w / size + 1));
            let mut acc = [0u32; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = (sy * w + sx) * 4;
                    for k in 0..4 {
                        acc[k] += u32::from(src[i + k]);
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            let o = (y * size + x) * 4;
            for k in 0..4 {
                rgba[o + k] = (acc[k] / n) as u8;
            }
        }
    }
    Some(Swatch { size, rgba })
}

/// Bilinear sample of a 64×64 map channel at chunk uv.
fn sample64(map: &[u8], stride: usize, channel: usize, u: f32, v: f32) -> f32 {
    let fx = (u * 64.0 - 0.5).clamp(0.0, 63.0);
    let fy = (v * 64.0 - 0.5).clamp(0.0, 63.0);
    let (x0, y0) = (fx as usize, fy as usize);
    let (x1, y1) = ((x0 + 1).min(63), (y0 + 1).min(63));
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let at = |x: usize, y: usize| f32::from(map[(y * 64 + x) * stride + channel]) / 255.0;
    let top = at(x0, y0) * (1.0 - tx) + at(x1, y0) * tx;
    let bot = at(x0, y1) * (1.0 - tx) + at(x1, y1) * tx;
    top * (1.0 - ty) + bot * ty
}

/// One chunk's ground as an image: its layers blended through its alpha map, its baked shadow
/// multiplied in.
fn bake_chunk(
    chunk: &ChunkMesh,
    texels: usize,
    swatches: &mut HashMap<(String, usize), Option<Swatch>>,
    data: &WowData,
) -> Image {
    let rep = (texels / LAYER_TILES).max(1);
    let layers: Vec<Option<&Swatch>> = {
        for path in &chunk.layer_textures {
            swatches
                .entry((path.clone(), rep))
                .or_insert_with(|| data.rgba_image(path).and_then(|i| swatch(&i, rep)));
        }
        chunk
            .layer_textures
            .iter()
            .map(|p| swatches.get(&(p.clone(), rep)).and_then(|s| s.as_ref()))
            .collect()
    };
    let mut out = vec![0u8; texels * texels * 4];
    for y in 0..texels {
        let v = (y as f32 + 0.5) / texels as f32;
        for x in 0..texels {
            let u = (x as f32 + 0.5) / texels as f32;
            let mut col = [0.3f32, 0.3, 0.25];
            for (li, layer) in layers.iter().enumerate() {
                let Some(s) = layer else { continue };
                let i = ((y % s.size) * s.size + (x % s.size)) * 4;
                let c = [
                    f32::from(s.rgba[i]),
                    f32::from(s.rgba[i + 1]),
                    f32::from(s.rgba[i + 2]),
                ];
                let a = if li == 0 {
                    1.0
                } else {
                    chunk
                        .alpha_map
                        .as_ref()
                        .map_or(0.0, |m| sample64(m, 4, li - 1, u, v))
                };
                for k in 0..3 {
                    col[k] = col[k] * (1.0 - a) + c[k] / 255.0 * a;
                }
            }
            if let Some(sh) = &chunk.shadow {
                let s = sample64(sh, 1, 0, u, v);
                let keep = 1.0 - s * (1.0 - SHADOW_KEEP);
                for c in &mut col {
                    *c *= keep;
                }
            }
            let o = (y * texels + x) * 4;
            for k in 0..3 {
                out[o + k] = (col[k].clamp(0.0, 1.0) * 255.0) as u8;
            }
            out[o + 3] = 255;
        }
    }
    Image::new(
        Extent3d {
            width: texels as u32,
            height: texels as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        out,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn chunk_mesh(chunk: &ChunkMesh) -> Option<Mesh> {
    if chunk.indices.len() < 3 {
        return None;
    }
    let positions: Vec<[f32; 3]> = chunk
        .positions
        .iter()
        .map(|p| scene(*p).to_array())
        .collect();
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, chunk.uvs.clone());
    m.insert_indices(Indices::U32(chunk.indices.clone()));
    if chunk.normals.len() == chunk.positions.len() {
        let n: Vec<[f32; 3]> = chunk
            .normals
            .iter()
            .map(|n| wow_to_bevy(*n).to_array())
            .collect();
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, n);
    } else {
        m.compute_normals();
    }
    Some(m)
}

/// Lamps among the doodads, by model name: each gets a warm light at its head.
fn is_lamp(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.contains("lamppost") || m.contains("streetlamp") || m.contains("duskwoodlamp")
}

/// Spawn the ground, the doodads and the buildings; each model entity is returned with its
/// lowercased path and scene position, for the hamlet to hang signs on. Trees within `clear` yards
/// of `eye` are left out, so none stands in front of the view. Everything carries `scoped`.
#[allow(clippy::too_many_arguments)]
pub fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    data: &WowData,
    map: &DarkshireMap,
    scoped: impl Component + Clone,
    lamp: impl Fn(&mut Commands, Vec3),
    eye: Vec3,
    clear: f32,
) -> Vec<(Entity, String, Vec3)> {
    let started = std::time::Instant::now();
    let mut swatches = HashMap::new();
    for chunk in &map.chunks {
        let Some(mesh) = chunk_mesh(chunk) else {
            continue;
        };
        let mid = chunk.positions[chunk.positions.len() / 2];
        let texels = if flat_dist(mid) < NEAR_CHUNKS {
            NEAR_TEXELS
        } else {
            FAR_TEXELS
        };
        let image = images.add(bake_chunk(chunk, texels, &mut swatches, data));
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(mats.add(StandardMaterial {
                base_color_texture: Some(image),
                perceptual_roughness: 1.0,
                reflectance: 0.04,
                ..default()
            })),
            Transform::default(),
            scoped.clone(),
        ));
    }
    info!("darkshire: ground baked in {:?}", started.elapsed());

    let mut placed = Vec::new();
    let mut lamps = 0;
    for d in &map.doodads {
        let at = scene(d.position);
        let model = d.model.to_ascii_lowercase();
        if model.contains("tree") && at.xz().distance(eye.xz()) < clear {
            continue;
        }
        let e = commands.spawn((
            Transform {
                translation: at,
                rotation: placement_rotation(d.rotation),
                scale: Vec3::splat(d.scale),
            },
            Visibility::default(),
            WowModel {
                source: ModelSource::Prop(d.model.clone(), None),
                height: None,
            },
            scoped.clone(),
        ));
        placed.push((e.id(), model, at));
        if is_lamp(&d.model) && lamps < 24 {
            lamps += 1;
            lamp(commands, at + Vec3::Y * 3.6 * d.scale);
        }
    }

    for w in &map.wmos {
        let at = scene(w.position);
        let e = commands
            .spawn((
                Transform {
                    translation: at,
                    rotation: placement_rotation(w.rotation),
                    scale: Vec3::ONE,
                },
                Visibility::default(),
                WowModel {
                    source: ModelSource::WmoPlaced(w.model.clone(), w.doodad_set),
                    height: None,
                },
                scoped.clone(),
            ))
            .id();
        placed.push((e, w.model.to_ascii_lowercase(), at));
    }
    placed
}
