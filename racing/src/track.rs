use bevy::prelude::*;

/// The computed cubic spline for the track centre line.
#[derive(Resource)]
pub struct TrackSpline {
    pub spline: CubicCurve<Vec2>,
}

/// Build a closed cubic B-spline from control points.
pub fn build_spline(control_points: &[Vec2]) -> CubicCurve<Vec2> {
    CubicBSpline::new(control_points.to_vec())
        .to_curve_cyclic()
        .expect("Failed to create cyclic curve")
}

/// Compute the arc-length of a closed spline by sampling.
pub fn spline_length(spline: &CubicCurve<Vec2>, samples: usize) -> f32 {
    let domain = spline.domain();
    let t_max = domain.end();
    let mut length = 0.0f32;
    let mut prev = spline.position(0.0);
    for i in 1..=samples {
        let t = (i as f32 / samples as f32) * t_max;
        let p = spline.position(t);
        length += prev.distance(p);
        prev = p;
    }
    length
}

/// Get the first control point from a track file (useful for spawn position).
pub fn first_point_from_file(track_file: &crate::track_format::TrackFile) -> Vec2 {
    let pts = track_file.control_points_vec2();
    pts[0]
}

pub fn create_track_mesh(spline: &CubicCurve<Vec2>, track_width: f32, segments: usize) -> Mesh {
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

pub fn create_kerb_meshes(
    spline: &CubicCurve<Vec2>,
    track_width: f32,
    kerb_width: f32,
    segments: usize,
) -> (Mesh, Mesh) {
    let domain = spline.domain();
    let t_max = domain.end();
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
