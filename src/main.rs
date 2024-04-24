//! A simple 3D scene with light shining over a cube sitting on a plane.

use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use std::f32::consts::PI;

// length of each road segment
static LEN_SEG: f32 = 1.8;
// how fast the road moves forward, m/s
static ROAD_VEL: f32 = 1.0;
static PLAYER_VEL: f32 = 1.0;
static PLAYER_GUN_PERIOD: f32 = 0.5;
static BULLET_VEL: f32 = 50.0;
static ENEMY_SIZE: f32 = 0.3;
static ENEMY_VEL: f32 = 0.5;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: WindowResolution::new(1080. / 2., 1920. / 2.),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(WorldInspectorPlugin::new())
        .add_systems(Startup, setup)
        .add_systems(Update, move_road)
        .add_systems(Update, move_player)
        .add_systems(Update, shoot)
        .add_systems(Update, move_bullet)
        .add_systems(Update, move_enemies)
        .run();
}

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct Road;

fn move_road(time: Res<Time>, mut transform: Query<&mut Transform, With<Road>>) {
    for mut t in &mut transform {
        let secs = time.delta_seconds();
        t.translation.z -= secs * ROAD_VEL;

        let div = t.translation.z % LEN_SEG;
        if t.translation.z.abs() > 1. && div < 0.01 {
            t.translation.z = div + 10. * LEN_SEG;
        }
    }
}

/// returns desired x,z velocity based on what keys are pressed
fn decode_move(input: &ButtonInput<KeyCode>, elapsed: f32) -> (f32, f32) {
    let mut x = 0.0;
    let mut z = 0.0;
    if input.pressed(KeyCode::ArrowLeft) {
        x = -PLAYER_VEL;
    }
    if input.pressed(KeyCode::ArrowRight) {
        x = PLAYER_VEL;
    }
    if input.pressed(KeyCode::ArrowUp) {
        z = -PLAYER_VEL;
    }
    if input.pressed(KeyCode::ArrowDown) {
        z = PLAYER_VEL;
    }

    (x * elapsed, z * elapsed)
}

#[derive(Component, Default)]
pub struct Gun {
    handles: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
    last_fired: f32,
}

#[derive(Component)]
pub struct Bullet;

#[derive(Component)]
pub struct Enemy;

fn move_player(
    time: Res<Time>,
    mut transform: Query<&mut Transform, With<Player>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    let x_width = 3.6;
    let mut transform = transform.single_mut();
    let secs = time.delta_seconds();
    let tl = &mut transform.translation;
    let (dx, dz) = decode_move(&keyboard_input, secs);
    tl.z += dz;
    tl.x += dx;
    // keep player in bounds
    tl.x = tl.x.clamp(-x_width / 2.0, x_width / 2.0);
    tl.z = tl.z.clamp(0.0, 5.0);
}

fn move_enemies(
    time: Res<Time>,
    mut transforms: Query<&mut Transform, With<Enemy>>,
    player_pos: Query<&Transform, With<Player>>,
) {
    for transform in &mut transforms {
        let mut t = transform.translation;
        t.z -= ENEMY_VEL * time.delta_seconds();
        if let Ok(player_pos) = player_pos.get_single() {
            t.z = t.z.min(player_pos.translation.z);
        } else {
            println!("no player????");
        }
    }
}

fn shoot(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    mut gun_pos: Query<(&GlobalTransform, &mut Gun)>,
) {
    for (pos, mut gun) in &mut gun_pos {
        let now = time.elapsed_seconds();

        if now - gun.last_fired > PLAYER_GUN_PERIOD {
            gun.last_fired = now
        } else {
            continue;
        }
        // no time for monad mental gymnastics, I have games to build
        let (mesh, material) = if gun.handles.is_some() {
            let (mesh, material) = gun.handles.clone().unwrap();
            (mesh, material)
        } else {
            let mesh = meshes.add(Sphere::new(0.05));
            let material = materials.add(StandardMaterial {
                base_color: Color::YELLOW,
                unlit: true,
                ..default()
            });
            gun.handles = Some((mesh.clone(), material.clone()));
            (mesh, material)
        };

        commands
            .spawn(PbrBundle {
                transform: Transform::default().with_translation(pos.translation()),
                mesh,
                material,
                ..default()
            })
            .insert(Bullet);
    }
}

fn move_bullet(
    mut commands: Commands,
    time: Res<Time>,
    mut bullet_pos: Query<(&mut Transform, Entity), With<Bullet>>,
    enemies: Query<(&GlobalTransform, Entity), With<Enemy>>,
) {
    // we have move_bullet and move_player, why not combine into Velocity component or smth?
    // I think it's not actually a great idea: they move in different ways and I don't want to
    // import a heavyweight plugin system with a superset of features.
    // eg: bullets need hit detection, player needs to respond to controls, road needs to loop back
    // so their similarities mostly end at x += v * delta_t, and this logic alone does not warrant
    // breaking out

    for (mut p, e) in &mut bullet_pos {
        if p.translation.z.abs() > 100. {
            commands.entity(e).despawn()
        }
        let new_z = p.translation.z - time.delta_seconds() * BULLET_VEL;
        let mut was_kil = false;
        for (e_p, e_e) in &enemies {
            if p.translation.z >= e_p.translation().z && new_z <= e_p.translation().z {
                // crosses over
                if (p.translation.x - e_p.translation().x).abs() <= ENEMY_SIZE / 2.0 {
                    println!("GOTTEM");
                    was_kil = true;
                    commands.entity(e_e).despawn();
                    break;
                }
            }
        }
        if was_kil {
            // collision consumes the bullet
            commands.entity(e).despawn()
        } else {
            p.translation.z = new_z;
        }
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // road
    let road_mesh = asset_server.load("meshes.gltf#Mesh0/Primitive0");
    let road_mat = asset_server.load("meshes.gltf#Material0");
    commands
        .spawn(SpatialBundle {
            transform: Transform::default().with_translation(Vec3::new(0.0, -0.2, 10. * LEN_SEG)),
            ..default()
        })
        .insert(Road {})
        .with_children(|parent| {
            for i in 0..64 {
                parent.spawn(PbrBundle {
                    mesh: road_mesh.clone(),
                    material: road_mat.clone(),
                    transform: Transform::default()
                        //.with_scale(Vec3::new(0.1, 0.1, 0.1))
                        .with_rotation(Quat::from_rotation_y(PI / 2.0))
                        .with_translation(Vec3::new(0.0, 0.0, -i as f32 * LEN_SEG)),
                    ..default()
                });
            }
        });

    // player
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Cuboid::new(0.1, 0.1, 0.1)),
            material: materials.add(Color::rgb_u8(124, 144, 255)),
            transform: Transform::from_xyz(0.0, 0.1 / 2.0, 0.0),
            ..default()
        })
        .insert(Player {})
        .insert(Gun::default());

    // enemies
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Cuboid::new(ENEMY_SIZE, ENEMY_SIZE, ENEMY_SIZE)),
            material: materials.add(Color::RED),
            transform: Transform::from_xyz(0.0, ENEMY_SIZE / 2.0, -5.0),
            ..default()
        })
        .insert(Enemy {});

    // light
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });

    // camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 2.0, 6.0).looking_at(Vec3::new(0., 0.5, 0.), Vec3::Y),
        ..default()
    });
}
