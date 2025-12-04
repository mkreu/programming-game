use bevy::{
    diagnostic::FrameTimeDiagnosticsPlugin,
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
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    // Green ground plane
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(2000.0, 2000.0))),
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