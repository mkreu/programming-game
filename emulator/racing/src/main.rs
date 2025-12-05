use bevy::{
    diagnostic::FrameTimeDiagnosticsPlugin,
    input::mouse::MouseWheel,
    math::{vec2, cubic_splines::{CubicCardinalSpline, CubicCurve, CyclicCubicGenerator}},
    prelude::*,
};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            FrameTimeDiagnosticsPlugin::default(),
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, (drive_car, update_camera))
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn(Camera2d);

    // Green ground plane
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(800.0, 800.0))),
        MeshMaterial2d(materials.add(Color::srgb(0.2, 0.6, 0.2))),
        Transform::from_xyz(0.0, 0.0, -1.0),
    ));

    // Create a simple oval racetrack using control points
    let control_points = vec![
        vec2(300.0, 0.0),
        vec2(300.0, 200.0),
        vec2(0.0, 300.0),
        vec2(-300.0, 200.0),
        vec2(-300.0, 0.0),
        vec2(-300.0, -200.0),
        vec2(0.0, -300.0),
        vec2(300.0, -200.0),
    ];

    let spline = CubicCardinalSpline::new(0.5, control_points).to_curve_cyclic().expect("Failed to create cyclic curve");
    
    // Generate track mesh
    let track_mesh = create_track_mesh(&spline, 80.0, 100);
    
    commands.spawn((
        Mesh2d(meshes.add(track_mesh)),
        MeshMaterial2d(materials.add(Color::srgb(0.3, 0.3, 0.3))),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
    
    // Generate kerbs with vertex colors
    let (inner_kerb, outer_kerb) = create_kerb_meshes(&spline, 80.0, 100);
    
    commands.spawn((
        Mesh2d(meshes.add(inner_kerb)),
        MeshMaterial2d(materials.add(ColorMaterial::default())),
        Transform::from_xyz(0.0, 0.0, 0.1),
    ));
    
    commands.spawn((
        Mesh2d(meshes.add(outer_kerb)),
        MeshMaterial2d(materials.add(ColorMaterial::default())),
        Transform::from_xyz(0.0, 0.0, 0.1),
    ));
    
    // Spawn the player car with wheels as children
    commands.spawn((
        Sprite::from_image(asset_server.load("blue_car_without_wheels.png")),
        Transform::from_xyz(300.0, 0.0, 1.0).with_scale(Vec3::splat(0.1)),
        Car {
            velocity: Vec2::ZERO,
            angle: 0.0,
            speed: 0.0,
        },
    )).with_children(|parent| {
        // Front left wheel
        parent.spawn((
            Sprite::from_image(asset_server.load("wheel.png")),
            Transform::from_xyz(-38.0, 72.0, 0.1),
            Wheel,
        ));
        // Front right wheel
        parent.spawn((
            Sprite::from_image(asset_server.load("wheel.png")),
            Transform::from_xyz(38.0, 72.0, 0.1),
            Wheel,
        ));
    });
}

fn create_track_mesh(spline: &CubicCurve<Vec2>, track_width: f32, segments: usize) -> Mesh {
    let domain = spline.domain();
    let t_max = domain.end();
    
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    
    // Generate vertices along the spline
    for i in 0..segments {
        let t1 = (i as f32 / segments as f32) * t_max;
        let t2 = (((i + 1) % segments) as f32 / segments as f32) * t_max;
        
        let p1 = spline.position(t1);
        let p2 = spline.position(t2);
        
        // Calculate perpendicular direction for track width
        let tangent = (p2 - p1).normalize();
        let normal = vec2(-tangent.y, tangent.x);
        
        // Inner and outer edge vertices
        let inner = p1 - normal * track_width * 0.5;
        let outer = p1 + normal * track_width * 0.5;
        
        positions.push([inner.x, inner.y, 0.0]);
        positions.push([outer.x, outer.y, 0.0]);
    }
    
    // Generate triangle indices
    for i in 0..segments {
        let base = (i * 2) as u32;
        let next_base = ((i + 1) % segments * 2) as u32;
        
        // Two triangles per segment
        indices.push(base);
        indices.push(next_base);
        indices.push(base + 1);
        
        indices.push(base + 1);
        indices.push(next_base);
        indices.push(next_base + 1);
    }
    
    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(bevy::mesh::Indices::U32(indices));
    mesh
}

fn create_kerb_meshes(spline: &CubicCurve<Vec2>, track_width: f32, segments: usize) -> (Mesh, Mesh) {
    let domain = spline.domain();
    let t_max = domain.end();
    let kerb_width = 8.0;
    let kerb_stripe_length = 1; // Number of segments per stripe color
    
    let mut inner_positions = Vec::new();
    let mut inner_colors = Vec::new();
    let mut inner_indices = Vec::new();
    
    let mut outer_positions = Vec::new();
    let mut outer_colors = Vec::new();
    let mut outer_indices = Vec::new();
    
    // Generate vertices - 4 vertices per segment (not shared with adjacent segments)
    for i in 0..segments {
        let t = (i as f32 / segments as f32) * t_max;
        let t_next = (((i + 1) % segments) as f32 / segments as f32) * t_max;
        
        let p = spline.position(t);
        let p_next = spline.position(t_next);
        
        // Calculate normals at both the start and end of this segment
        let i_prev = if i == 0 { segments - 1 } else { i - 1 };
        let t_prev = (i_prev as f32 / segments as f32) * t_max;
        let p_prev = spline.position(t_prev);
        let t_after = (((i + 2) % segments) as f32 / segments as f32) * t_max;
        let p_after = spline.position(t_after);
        
        // Normal at start: perpendicular to direction from prev to current
        let tangent_start = (p_next - p_prev).normalize();
        let normal_start = vec2(-tangent_start.y, tangent_start.x);
        
        // Normal at end: perpendicular to direction from current to after
        let tangent_end = (p_after - p).normalize();
        let normal_end = vec2(-tangent_end.y, tangent_end.x);
        
        // Determine color for this segment
        let is_red = (i / kerb_stripe_length) % 2 == 0;
        let color = if is_red {
            [0.9, 0.1, 0.1, 1.0]
        } else {
            [0.95, 0.95, 0.95, 1.0]
        };
        
        // Inner kerb - use appropriate normal at each end
        let inner_edge_start = p - normal_start * track_width * 0.5;
        let inner_outer_start = p - normal_start * (track_width * 0.5 - kerb_width);
        let inner_edge_end = p_next - normal_end * track_width * 0.5;
        let inner_outer_end = p_next - normal_end * (track_width * 0.5 - kerb_width);
        
        let base_idx = inner_positions.len() as u32;
        inner_positions.push([inner_edge_start.x, inner_edge_start.y, 0.0]);
        inner_positions.push([inner_outer_start.x, inner_outer_start.y, 0.0]);
        inner_positions.push([inner_edge_end.x, inner_edge_end.y, 0.0]);
        inner_positions.push([inner_outer_end.x, inner_outer_end.y, 0.0]);
        
        // All 4 vertices get the same color for sharp transition
        inner_colors.push(color);
        inner_colors.push(color);
        inner_colors.push(color);
        inner_colors.push(color);
        
        // Two triangles for this segment
        inner_indices.push(base_idx);
        inner_indices.push(base_idx + 2);
        inner_indices.push(base_idx + 1);
        
        inner_indices.push(base_idx + 1);
        inner_indices.push(base_idx + 2);
        inner_indices.push(base_idx + 3);
        
        // Outer kerb - use appropriate normal at each end
        let outer_inner_start = p + normal_start * (track_width * 0.5 - kerb_width);
        let outer_edge_start = p + normal_start * track_width * 0.5;
        let outer_inner_end = p_next + normal_end * (track_width * 0.5 - kerb_width);
        let outer_edge_end = p_next + normal_end * track_width * 0.5;
        
        let base_idx = outer_positions.len() as u32;
        outer_positions.push([outer_inner_start.x, outer_inner_start.y, 0.0]);
        outer_positions.push([outer_edge_start.x, outer_edge_start.y, 0.0]);
        outer_positions.push([outer_inner_end.x, outer_inner_end.y, 0.0]);
        outer_positions.push([outer_edge_end.x, outer_edge_end.y, 0.0]);
        
        outer_colors.push(color);
        outer_colors.push(color);
        outer_colors.push(color);
        outer_colors.push(color);
        
        outer_indices.push(base_idx);
        outer_indices.push(base_idx + 2);
        outer_indices.push(base_idx + 1);
        
        outer_indices.push(base_idx + 1);
        outer_indices.push(base_idx + 2);
        outer_indices.push(base_idx + 3);
    }
    
    let mut inner_mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    inner_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, inner_positions);
    inner_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, inner_colors);
    inner_mesh.insert_indices(bevy::mesh::Indices::U32(inner_indices));
    
    let mut outer_mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    outer_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, outer_positions);
    outer_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, outer_colors);
    outer_mesh.insert_indices(bevy::mesh::Indices::U32(outer_indices));
    
    (inner_mesh, outer_mesh)
}

#[derive(Component)]
struct Car {
    velocity: Vec2,
    angle: f32,
    speed: f32,
}

#[derive(Component)]
struct Wheel;

fn drive_car(
    mut car_query: Query<(&mut Transform, &mut Car, &Children)>,
    mut wheel_query: Query<&mut Transform, (With<Wheel>, Without<Car>)>,
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    for (mut transform, mut car, children) in &mut car_query {
        let dt = time.delta_secs();
        
        // Car physics parameters
        let acceleration = 150.0;
        let braking = 200.0;
        let max_speed = 500.0;
        let reverse_speed = 100.0;
        let turn_speed = 3.0;
        let drag: f32 = 0.99;
        
        // Forward/Backward
        if keyboard.pressed(KeyCode::KeyW) {
            car.speed += acceleration * dt;
            car.speed = car.speed.min(max_speed);
        }
        if keyboard.pressed(KeyCode::KeyS) {
            car.speed -= braking * dt;
            car.speed = car.speed.max(-reverse_speed);
        }
        
        // Apply drag
        car.speed *= drag.powf(dt * 60.0_f32);
        
        // Stop if very slow
        if car.speed.abs() < 0.1 {
            car.speed = 0.0;
        }
        
        // Steering (only when moving)
        let mut wheel_angle = 0.0;
        if car.speed.abs() > 0.1 {
            if keyboard.pressed(KeyCode::KeyA) {
                car.angle -= turn_speed * dt * (car.speed / max_speed);
                wheel_angle = -0.5; // Left turn
            }
            if keyboard.pressed(KeyCode::KeyD) {
                car.angle += turn_speed * dt * (car.speed / max_speed);
                wheel_angle = 0.5; // Right turn
            }
        }
        
        // Update velocity based on angle and speed
        car.velocity = Vec2::new(car.angle.sin(), car.angle.cos()) * car.speed;
        
        // Update position
        transform.translation.x += car.velocity.x * dt;
        transform.translation.y += car.velocity.y * dt;
        
        // Update rotation
        transform.rotation = Quat::from_rotation_z(-car.angle);
        
        // Update wheel rotation
        for child in children.iter() {
            if let Ok(mut wheel_transform) = wheel_query.get_mut(child) {
                wheel_transform.rotation = Quat::from_rotation_z(-wheel_angle);
            }
        }
    }
}

fn update_camera(
    car_query: Query<&Transform, With<Car>>,
    mut camera_query: Query<(&mut Transform, &mut Projection), (With<Camera2d>, Without<Car>)>,
    mut scroll_events: MessageReader<MouseWheel>,
) {
    let Ok(car_transform) = car_query.single() else {
        return;
    };
    let Ok((mut camera_transform, mut projection)) = camera_query.single_mut() else {
        return;
    };
    
    // Handle zoom with mouse wheel
    if let Projection::Orthographic(ref mut ortho) = *projection {
        for event in scroll_events.read() {
            let zoom_delta = match event.unit {
                bevy::input::mouse::MouseScrollUnit::Line => event.y * 0.1,
                bevy::input::mouse::MouseScrollUnit::Pixel => event.y * 0.001,
            };
            
            ortho.scale *= 1.0 - zoom_delta;
            ortho.scale = ortho.scale.clamp(0.1, 5.0);
        }
    }
    
    // Follow the car
    camera_transform.translation.x = car_transform.translation.x;
    camera_transform.translation.y = car_transform.translation.y;
}