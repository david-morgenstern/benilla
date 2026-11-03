//! The hero portrait: a second copy of the hero stands far off-stage under its own light, and a
//! camera renders their face into an image the interface shows, in place of a painted portrait.

use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

use super::looks::hero_look;
use super::Run;
use crate::rules::data::anim;
use crate::wow::{ModelSource, PlayAnim, WowData, WowModel};

/// Where the portrait sitter stands, beyond the stage camera's far plane.
const SITTER: Vec3 = Vec3::new(600.0, 0.0, 0.0);
const SITTER_HEIGHT: f32 = 1.9;

#[derive(Component)]
struct PortraitCam;

#[derive(Component)]
struct PortraitLight(Vec3);

#[derive(Resource)]
pub struct Portrait {
    pub image: Handle<Image>,
    /// The hero the screens want shown.
    pub want: Option<u32>,
    shown: Option<u32>,
    model: Option<Entity>,
}

pub struct PortraitPlugin;

impl Plugin for PortraitPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, (sit, frame).chain());
    }
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let image = images.add(Image::new_target_texture(
        256,
        256,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let head = SITTER + Vec3::Y * SITTER_HEIGHT * 0.86;
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.05, 0.035, 0.03)),
            ..default()
        },
        RenderTarget::Image(image.clone().into()),
        Projection::from(PerspectiveProjection {
            fov: 26f32.to_radians(),
            ..default()
        }),
        Transform::from_translation(head + Vec3::new(0.25, 0.05, 1.25))
            .looking_at(head - Vec3::Y * 0.05, Vec3::Y),
        PortraitCam,
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.78, 0.55),
            intensity: 60_000.0,
            range: 6.0,
            ..default()
        },
        Transform::from_translation(head + Vec3::new(0.9, 0.6, 1.4)),
        PortraitLight(Vec3::new(0.9, 0.6, 1.4)),
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(0.4, 0.45, 0.8),
            intensity: 15_000.0,
            range: 6.0,
            ..default()
        },
        Transform::from_translation(head + Vec3::new(-1.0, 0.2, 0.6)),
        PortraitLight(Vec3::new(-1.0, 0.2, 0.6)),
    ));
    commands.insert_resource(Portrait {
        image,
        want: None,
        shown: None,
        model: None,
    });
}

fn sit(
    mut commands: Commands,
    mut p: ResMut<Portrait>,
    run: Option<Res<Run>>,
    game: Option<Res<super::Game>>,
    data: Res<WowData>,
) {
    if p.want == p.shown {
        return;
    }
    if let Some(e) = p.model.take() {
        commands.entity(e).try_despawn();
    }
    p.shown = p.want;
    let Some(id) = p.want else { return };
    let hero = run
        .as_ref()
        .and_then(|r| r.0.party.iter().find(|h| h.id == id).cloned())
        .or_else(|| {
            game.as_ref()
                .and_then(|g| g.0.roster.iter().find(|h| h.id == id).cloned())
        });
    let Some(hero) = hero else { return };
    let look = hero_look(&data, &hero);
    let e = commands
        .spawn((
            Transform::from_translation(SITTER)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI + 0.35)),
            Visibility::default(),
            WowModel {
                source: ModelSource::Character(Box::new(look)),
                height: Some(SITTER_HEIGHT),
            },
            PlayAnim::looped(anim::STAND),
        ))
        .id();
    p.model = Some(e);
}

/// Keep the camera on the sitter's head bone as it breathes and shifts.
fn frame(
    p: Res<Portrait>,
    rigs: Query<&crate::wow::Rig>,
    joints: Query<&GlobalTransform>,
    mut cam: Query<&mut Transform, (With<PortraitCam>, Without<PortraitLight>)>,
    mut lights: Query<(&mut Transform, &PortraitLight), Without<PortraitCam>>,
) {
    let Some(e) = p.model else { return };
    let Ok(rig) = rigs.get(e) else { return };
    let Some(head) = rig
        .head
        .and_then(|h| joints.get(h).ok())
        .map(GlobalTransform::translation)
    else {
        return;
    };
    let target = head + Vec3::Y * 0.02;
    if let Ok(mut t) = cam.single_mut() {
        let want = Transform::from_translation(target + Vec3::new(0.22, 0.04, 0.95))
            .looking_at(target, Vec3::Y);
        t.translation = t.translation.lerp(want.translation, 0.25);
        t.rotation = t.rotation.slerp(want.rotation, 0.25);
    }
    for (mut t, l) in &mut lights {
        t.translation = target + l.0;
    }
}
