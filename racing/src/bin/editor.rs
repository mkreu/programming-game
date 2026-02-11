use std::path::PathBuf;

use bevy::{
    color::palettes::css,
    input::mouse::MouseWheel,
    prelude::*,
    window::PrimaryWindow,
};

use racing::track::{self, TrackSpline};
use racing::track_format::TrackFile;

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let initial_path = std::env::args().nth(1).map(PathBuf::from);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Track Editor".into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(EditorState::new(initial_path))
        .add_systems(Startup, editor_setup)
        .add_systems(
            Update,
            (
                handle_camera,
                handle_mouse_input,
                handle_keyboard,
                rebuild_visuals.run_if(resource_changed::<RebuildFlag>),
                update_ui_text,
                draw_editor_gizmos,
            ),
        )
        .init_resource::<RebuildFlag>()
        .run();
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct EditorState {
    track_file: TrackFile,
    file_path: Option<PathBuf>,
    selected_point: Option<usize>,
    dragging: bool,
    /// Last cursor position in world coords while dragging (for deltas).
    drag_prev_world: Option<Vec2>,
    /// Undo stack: snapshots of control_points *before* a modification.
    undo_stack: Vec<Vec<[f32; 2]>>,
    /// Redo stack: snapshots popped from undo.
    redo_stack: Vec<Vec<[f32; 2]>>,
    /// Ruler start in world coords (Shift+LMB).
    ruler_start: Option<Vec2>,
    ruler_end: Option<Vec2>,
    /// Camera panning state.
    pan_origin: Option<Vec2>,
    pan_camera_origin: Option<Vec3>,
    /// Show curvature heatmap.
    show_curvature: bool,
    /// Show point index labels.
    show_labels: bool,
    /// Show help overlay.
    show_help: bool,
    /// Dirty flag — unsaved changes.
    dirty: bool,
}

impl EditorState {
    fn new(path: Option<PathBuf>) -> Self {
        let (track_file, file_path) = if let Some(ref p) = path {
            match TrackFile::load(p) {
                Ok(tf) => (tf, Some(p.clone())),
                Err(e) => {
                    eprintln!("Warning: {e}. Starting with empty track.");
                    (TrackFile::new_empty("Untitled"), None)
                }
            }
        } else {
            (TrackFile::new_empty("Untitled"), None)
        };

        Self {
            track_file,
            file_path,
            selected_point: None,
            dragging: false,
            drag_prev_world: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            ruler_start: None,
            ruler_end: None,
            pan_origin: None,
            pan_camera_origin: None,
            show_curvature: false,
            show_labels: true,
            show_help: true,
            dirty: false,
        }
    }

    fn push_undo(&mut self) {
        self.undo_stack.push(self.track_file.control_points.clone());
        self.redo_stack.clear();
        self.dirty = true;
    }

    fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.track_file.control_points.clone());
            self.track_file.control_points = prev;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.track_file.control_points.clone());
            self.track_file.control_points = next;
            self.dirty = true;
            true
        } else {
            false
        }
    }
}

/// Marker resource: when mutated, triggers a visual rebuild.
#[derive(Resource, Default)]
struct RebuildFlag(u32);

// ---------------------------------------------------------------------------
// Marker components
// ---------------------------------------------------------------------------

#[derive(Component)]
struct TrackVisual;

#[derive(Component)]
struct ControlPointVisual;

#[derive(Component)]
struct PointLabel;

#[derive(Component)]
struct UiOverlay;

#[derive(Component)]
struct HelpOverlay;

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

fn editor_setup(
    mut commands: Commands,
    editor: Res<EditorState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut rebuild: ResMut<RebuildFlag>,
) {
    // Camera
    let mut cam = commands.spawn(Camera2d);
    cam.insert(Transform::from_xyz(0.0, 0.0, 999.0));

    // Set initial zoom to see the whole track
    // We'll handle the projection after spawn via a separate query — instead,
    // we just insert an Orthographic projection with a sensible default.
    // (Bevy 0.17 auto-creates Projection for Camera2d; we adjust in handle_camera startup)

    // UI overlay — track info (top-left)
    commands.spawn((
        Text::new("Track Info"),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        UiOverlay,
    ));

    // Help overlay (bottom-left)
    commands.spawn((
        Text::new(HELP_TEXT),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        HelpOverlay,
    ));

    // Spawn initial track visuals
    spawn_track_visuals(
        &mut commands,
        &mut meshes,
        &mut materials,
        &editor.track_file,
        editor.show_curvature,
    );

    // Spawn control point visuals
    spawn_point_visuals(
        &mut commands,
        &mut meshes,
        &mut materials,
        &editor.track_file,
        editor.selected_point,
    );

    // Trigger initial rebuild counter
    rebuild.0 = 0;
}

const HELP_TEXT: &str = "\
Controls:
  LMB          Select / drag control point
  Right-drag   Pan camera
  Scroll       Zoom
  A            Add point at cursor
  Del/Bksp     Delete selected point
  Ctrl+Z       Undo
  Ctrl+Y       Redo
  Ctrl+S       Save
  Ctrl+O       Open
  Ctrl+N       New track
  Shift+drag   Ruler measurement
  [ / ]        Decrease / increase track width
  - / =        Scale track down / up
  C            Toggle curvature heatmap
  L            Toggle point labels
  H            Toggle this help";

// ---------------------------------------------------------------------------
// Spawn helpers
// ---------------------------------------------------------------------------

fn spawn_track_visuals(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    track_file: &TrackFile,
    show_curvature: bool,
) {
    let pts = track_file.control_points_vec2();
    if pts.len() < 4 {
        // Need at least 4 points for a cubic B-spline
        return;
    }
    let spline = track::build_spline(&pts);
    let tw = track_file.metadata.track_width;
    let kw = track_file.metadata.kerb_width;

    // Ground
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(2000.0, 2000.0))),
        MeshMaterial2d(materials.add(Color::srgb(0.2, 0.6, 0.2))),
        Transform::from_xyz(0.0, 0.0, -1.0),
        TrackVisual,
    ));

    // Track surface
    let track_mesh = if show_curvature {
        create_curvature_track_mesh(&spline, tw, 1000)
    } else {
        track::create_track_mesh(&spline, tw, 1000)
    };
    commands.spawn((
        Mesh2d(meshes.add(track_mesh)),
        MeshMaterial2d(if show_curvature {
            materials.add(ColorMaterial::default())
        } else {
            materials.add(Color::srgb(0.3, 0.3, 0.3))
        }),
        Transform::from_xyz(0.0, 0.0, 0.0),
        TrackVisual,
    ));

    // Kerbs
    let (inner_kerb, outer_kerb) = track::create_kerb_meshes(&spline, tw, kw, 1000);
    commands.spawn((
        Mesh2d(meshes.add(inner_kerb)),
        MeshMaterial2d(materials.add(ColorMaterial::default())),
        Transform::from_xyz(0.0, 0.0, 0.1),
        TrackVisual,
    ));
    commands.spawn((
        Mesh2d(meshes.add(outer_kerb)),
        MeshMaterial2d(materials.add(ColorMaterial::default())),
        Transform::from_xyz(0.0, 0.0, 0.1),
        TrackVisual,
    ));

    // Store spline resource
    commands.insert_resource(TrackSpline { spline });
}

fn spawn_point_visuals(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    track_file: &TrackFile,
    selected: Option<usize>,
) {
    let point_radius = 1.2;
    let normal_color = Color::srgba(1.0, 1.0, 0.0, 0.9);
    let selected_color = Color::srgba(0.0, 1.0, 1.0, 1.0);
    let circle_mesh = meshes.add(Circle::new(point_radius));
    let normal_mat = materials.add(normal_color);
    let selected_mat = materials.add(selected_color);

    for (i, &[x, y]) in track_file.control_points.iter().enumerate() {
        let is_sel = selected == Some(i);
        commands.spawn((
            Mesh2d(circle_mesh.clone()),
            MeshMaterial2d(if is_sel {
                selected_mat.clone()
            } else {
                normal_mat.clone()
            }),
            Transform::from_xyz(x, y, 2.0),
            ControlPointVisual,
        ));

        // Index label
        commands.spawn((
            Text2d::new(format!("{}", i)),
            TextFont {
                font_size: 60.0,
                ..default()
            },
            TextColor(if is_sel {
                Color::srgb(0.0, 1.0, 1.0)
            } else {
                Color::srgba(1.0, 1.0, 1.0, 0.7)
            }),
            Transform::from_xyz(x + 1.5, y + 1.5, 3.0).with_scale(Vec3::splat(0.04)),
            PointLabel,
            ControlPointVisual,
        ));
    }
}

// ---------------------------------------------------------------------------
// Curvature track mesh (vertex-colored)
// ---------------------------------------------------------------------------

fn create_curvature_track_mesh(
    spline: &CubicCurve<Vec2>,
    track_width: f32,
    segments: usize,
) -> Mesh {
    let domain = spline.domain();
    let t_max = domain.end();

    // First pass: compute curvature at each sample
    let mut curvatures = Vec::with_capacity(segments);
    for i in 0..segments {
        let t = (i as f32 / segments as f32) * t_max;
        let dt = t_max / segments as f32;
        let t_prev = if t < dt { t + t_max - dt } else { t - dt };
        let t_next = (t + dt) % t_max;

        let p_prev = spline.position(t_prev);
        let p_curr = spline.position(t);
        let p_next = spline.position(t_next);

        let v1 = (p_curr - p_prev).normalize_or_zero();
        let v2 = (p_next - p_curr).normalize_or_zero();
        let curvature = v1.angle_to(v2).abs();
        curvatures.push(curvature);
    }

    let max_curv = curvatures.iter().cloned().fold(0.01_f32, f32::max);

    let mut positions = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices = Vec::new();

    for i in 0..segments {
        let t1 = (i as f32 / segments as f32) * t_max;
        let t2 = (((i + 1) % segments) as f32 / segments as f32) * t_max;
        let p1 = spline.position(t1);
        let p2 = spline.position(t2);
        let tangent = (p2 - p1).normalize_or_zero();
        let normal = Vec2::new(-tangent.y, tangent.x);

        let inner = p1 - normal * track_width * 0.5;
        let outer = p1 + normal * track_width * 0.5;
        positions.push([inner.x, inner.y, 0.0]);
        positions.push([outer.x, outer.y, 0.0]);

        // Map curvature to color: green (low) → yellow (mid) → red (high)
        let t_val = (curvatures[i] / max_curv).clamp(0.0, 1.0);
        let (r, g, b) = if t_val < 0.5 {
            let s = t_val * 2.0;
            (s, 1.0, 0.0)
        } else {
            let s = (t_val - 0.5) * 2.0;
            (1.0, 1.0 - s, 0.0)
        };
        colors.push([r, g, b, 1.0]);
        colors.push([r, g, b, 1.0]);
    }

    for i in 0..segments {
        let base = (i * 2) as u32;
        let next_base = ((i + 1) % segments * 2) as u32;
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
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(bevy::mesh::Indices::U32(indices));
    mesh
}

// ---------------------------------------------------------------------------
// Camera: pan (right-drag) + zoom (scroll)
// ---------------------------------------------------------------------------

fn handle_camera(
    mut camera_q: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    mut scroll_events: MessageReader<MouseWheel>,
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut editor: ResMut<EditorState>,
) {
    let Ok((mut cam_tf, mut projection)) = camera_q.single_mut() else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };

    // --- Zoom ---
    if let Projection::Orthographic(ref mut ortho) = *projection {
        // Set a good initial zoom on the very first frame
        if ortho.scale == 1.0 {
            ortho.scale = 0.5;
        }

        for event in scroll_events.read() {
            let zoom_delta = match event.unit {
                bevy::input::mouse::MouseScrollUnit::Line => event.y * 0.1,
                bevy::input::mouse::MouseScrollUnit::Pixel => event.y * 0.001,
            };
            ortho.scale *= 1.0 - zoom_delta;
            ortho.scale = ortho.scale.clamp(0.001, 50.0);
        }
    }

    // --- Pan (right-click drag) ---
    if let Some(cursor) = window.cursor_position() {
        if buttons.just_pressed(MouseButton::Right) {
            editor.pan_origin = Some(cursor);
            editor.pan_camera_origin = Some(cam_tf.translation);
        }
    }

    if buttons.pressed(MouseButton::Right) {
        if let (Some(origin), Some(cam_origin)) = (editor.pan_origin, editor.pan_camera_origin) {
            if let Some(cursor) = window.cursor_position() {
                let scale = if let Projection::Orthographic(ref ortho) = *projection {
                    ortho.scale
                } else {
                    1.0
                };
                let delta = cursor - origin;
                cam_tf.translation.x = cam_origin.x - delta.x * scale;
                cam_tf.translation.y = cam_origin.y + delta.y * scale; // flip Y
            }
        }
    }

    if buttons.just_released(MouseButton::Right) {
        editor.pan_origin = None;
        editor.pan_camera_origin = None;
    }
}

// ---------------------------------------------------------------------------
// Cursor → world helper
// ---------------------------------------------------------------------------

fn cursor_to_world(
    window: &Window,
    camera: &Camera,
    cam_gt: &GlobalTransform,
) -> Option<Vec2> {
    let cursor = window.cursor_position()?;
    camera.viewport_to_world_2d(cam_gt, cursor).ok()
}

// ---------------------------------------------------------------------------
// Mouse input: select / drag / ruler
// ---------------------------------------------------------------------------

fn handle_mouse_input(
    mut editor: ResMut<EditorState>,
    buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut rebuild: ResMut<RebuildFlag>,
) {
    let Ok(window) = windows.single() else { return };
    let Ok((camera, cam_gt)) = camera_q.single() else { return };
    let Some(world_pos) = cursor_to_world(window, camera, cam_gt) else { return };

    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    // --- Ruler (Shift + LMB) ---
    if shift {
        if buttons.just_pressed(MouseButton::Left) {
            editor.ruler_start = Some(world_pos);
            editor.ruler_end = Some(world_pos);
        }
        if buttons.pressed(MouseButton::Left) {
            editor.ruler_end = Some(world_pos);
        }
        if buttons.just_released(MouseButton::Left) {
            // keep ruler visible until next click
        }
        return; // don't process selection while shift is held
    }

    // --- Select / drag ---
    if buttons.just_pressed(MouseButton::Left) {
        // Clear ruler
        editor.ruler_start = None;
        editor.ruler_end = None;

        // Find nearest control point
        let threshold = 3.0; // world units
        let mut best_idx: Option<usize> = None;
        let mut best_dist = f32::MAX;
        for (i, &[x, y]) in editor.track_file.control_points.iter().enumerate() {
            let d = world_pos.distance(Vec2::new(x, y));
            if d < best_dist && d < threshold {
                best_dist = d;
                best_idx = Some(i);
            }
        }
        editor.selected_point = best_idx;
        if best_idx.is_some() {
            editor.dragging = true;
            editor.drag_prev_world = Some(world_pos);
            editor.push_undo(); // snapshot before drag
        }
        rebuild.0 += 1; // update visuals for selection change
    }

    if buttons.pressed(MouseButton::Left) && editor.dragging {
        if let Some(idx) = editor.selected_point {
            editor.track_file.control_points[idx] = [world_pos.x, world_pos.y];
            editor.drag_prev_world = Some(world_pos);
            editor.dirty = true;
            rebuild.0 += 1;
        }
    }

    if buttons.just_released(MouseButton::Left) {
        editor.dragging = false;
        editor.drag_prev_world = None;
    }
}

// ---------------------------------------------------------------------------
// Keyboard: add/delete, undo/redo, file ops, toggles
// ---------------------------------------------------------------------------

fn handle_keyboard(
    mut editor: ResMut<EditorState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut rebuild: ResMut<RebuildFlag>,
) {
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    // --- Undo / Redo ---
    if ctrl && keyboard.just_pressed(KeyCode::KeyZ) {
        if shift {
            if editor.redo() { rebuild.0 += 1; }
        } else {
            if editor.undo() { rebuild.0 += 1; }
        }
        return;
    }
    if ctrl && keyboard.just_pressed(KeyCode::KeyY) {
        if editor.redo() { rebuild.0 += 1; }
        return;
    }

    // --- Save (Ctrl+S) ---
    if ctrl && keyboard.just_pressed(KeyCode::KeyS) {
        let path = if let Some(ref p) = editor.file_path {
            Some(p.clone())
        } else {
            rfd::FileDialog::new()
                .add_filter("Track files", &["toml"])
                .set_file_name("track.toml")
                .save_file()
        };
        if let Some(path) = path {
            match editor.track_file.save(&path) {
                Ok(()) => {
                    info!("Saved to {}", path.display());
                    editor.file_path = Some(path);
                    editor.dirty = false;
                }
                Err(e) => error!("Save failed: {e}"),
            }
        }
        return;
    }

    // --- Open (Ctrl+O) ---
    if ctrl && keyboard.just_pressed(KeyCode::KeyO) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Track files", &["toml"])
            .pick_file()
        {
            match TrackFile::load(&path) {
                Ok(tf) => {
                    editor.track_file = tf;
                    editor.file_path = Some(path);
                    editor.selected_point = None;
                    editor.undo_stack.clear();
                    editor.redo_stack.clear();
                    editor.dirty = false;
                    rebuild.0 += 1;
                    info!("Opened track");
                }
                Err(e) => error!("Open failed: {e}"),
            }
        }
        return;
    }

    // --- New (Ctrl+N) ---
    if ctrl && keyboard.just_pressed(KeyCode::KeyN) {
        editor.track_file = TrackFile::new_empty("Untitled");
        editor.file_path = None;
        editor.selected_point = None;
        editor.undo_stack.clear();
        editor.redo_stack.clear();
        editor.dirty = false;
        rebuild.0 += 1;
        return;
    }

    // --- Add point (A) at cursor ---
    if keyboard.just_pressed(KeyCode::KeyA) {
        let Ok(window) = windows.single() else { return };
        let Ok((camera, cam_gt)) = camera_q.single() else { return };
        let Some(world_pos) = cursor_to_world(window, camera, cam_gt) else { return };

        editor.push_undo();
        let insert_idx = find_insert_index(world_pos, &editor.track_file.control_points);
        editor
            .track_file
            .control_points
            .insert(insert_idx, [world_pos.x, world_pos.y]);
        editor.selected_point = Some(insert_idx);
        rebuild.0 += 1;
        return;
    }

    // --- Delete selected point (Delete / Backspace) ---
    if keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace) {
        if let Some(idx) = editor.selected_point {
            if editor.track_file.control_points.len() > 1 {
                editor.push_undo();
                editor.track_file.control_points.remove(idx);
                // Adjust selection
                if idx >= editor.track_file.control_points.len() {
                    editor.selected_point = Some(editor.track_file.control_points.len() - 1);
                }
                rebuild.0 += 1;
            }
        }
        return;
    }

    // --- Track width adjustment ([ / ]) ---
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        editor.push_undo();
        editor.track_file.metadata.track_width = (editor.track_file.metadata.track_width - 0.5).max(1.0);
        rebuild.0 += 1;
        return;
    }
    if keyboard.just_pressed(KeyCode::BracketRight) {
        editor.push_undo();
        editor.track_file.metadata.track_width += 0.5;
        rebuild.0 += 1;
        return;
    }

    // --- Scale all points (- / =) ---
    if keyboard.just_pressed(KeyCode::Minus) && !ctrl {
        scale_track(&mut editor, 1.0 / 1.05);
        rebuild.0 += 1;
        return;
    }
    if keyboard.just_pressed(KeyCode::Equal) && !ctrl {
        scale_track(&mut editor, 1.05);
        rebuild.0 += 1;
        return;
    }

    // --- Toggle curvature (C) ---
    if keyboard.just_pressed(KeyCode::KeyC) {
        editor.show_curvature = !editor.show_curvature;
        rebuild.0 += 1;
    }

    // --- Toggle labels (L) ---
    if keyboard.just_pressed(KeyCode::KeyL) {
        editor.show_labels = !editor.show_labels;
        rebuild.0 += 1;
    }

    // --- Toggle help (H) ---
    if keyboard.just_pressed(KeyCode::KeyH) {
        editor.show_help = !editor.show_help;
    }
}

/// Scale all control points around their centroid by the given factor.
fn scale_track(editor: &mut ResMut<EditorState>, factor: f32) {
    if editor.track_file.control_points.is_empty() {
        return;
    }
    editor.push_undo();
    let pts = &editor.track_file.control_points;
    let n = pts.len() as f32;
    let cx: f32 = pts.iter().map(|p| p[0]).sum::<f32>() / n;
    let cy: f32 = pts.iter().map(|p| p[1]).sum::<f32>() / n;
    for p in &mut editor.track_file.control_points {
        p[0] = cx + (p[0] - cx) * factor;
        p[1] = cy + (p[1] - cy) * factor;
    }
}

/// Find the best index to insert a new control point near `click`.
fn find_insert_index(click: Vec2, points: &[[f32; 2]]) -> usize {
    if points.is_empty() {
        return 0;
    }
    if points.len() == 1 {
        return 1;
    }
    let n = points.len();
    let mut best_idx = 0;
    let mut best_dist = f32::MAX;
    for i in 0..n {
        let a = Vec2::new(points[i][0], points[i][1]);
        let b = Vec2::new(points[(i + 1) % n][0], points[(i + 1) % n][1]);
        let d = point_to_segment_dist(click, a, b);
        if d < best_dist {
            best_dist = d;
            best_idx = (i + 1) % (n + 1);
            if best_idx == 0 {
                best_idx = n;
            }
        }
    }
    best_idx
}

fn point_to_segment_dist(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let ap = p - a;
    let len_sq = ab.length_squared();
    if len_sq < 1e-8 {
        return ap.length();
    }
    let t = (ap.dot(ab) / len_sq).clamp(0.0, 1.0);
    (a + ab * t - p).length()
}

// ---------------------------------------------------------------------------
// Rebuild visuals when track data changes
// ---------------------------------------------------------------------------

fn rebuild_visuals(
    mut commands: Commands,
    old_track: Query<Entity, With<TrackVisual>>,
    old_points: Query<Entity, With<ControlPointVisual>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    editor: Res<EditorState>,
) {
    // Despawn old entities
    for entity in &old_track {
        commands.entity(entity).despawn();
    }
    for entity in &old_points {
        commands.entity(entity).despawn();
    }

    // Rebuild
    spawn_track_visuals(
        &mut commands,
        &mut meshes,
        &mut materials,
        &editor.track_file,
        editor.show_curvature,
    );
    spawn_point_visuals(
        &mut commands,
        &mut meshes,
        &mut materials,
        &editor.track_file,
        editor.selected_point,
    );
}

// ---------------------------------------------------------------------------
// UI text overlay
// ---------------------------------------------------------------------------

fn update_ui_text(
    editor: Res<EditorState>,
    track_spline: Option<Res<TrackSpline>>,
    mut ui_text: Query<&mut Text, (With<UiOverlay>, Without<HelpOverlay>)>,
    mut help_text: Query<(&mut Visibility, &mut Text), (With<HelpOverlay>, Without<UiOverlay>)>,
) {
    // Track info
    if let Ok(mut text) = ui_text.single_mut() {
        let name = &editor.track_file.metadata.name;
        let n_pts = editor.track_file.control_points.len();
        let dirty_marker = if editor.dirty { " *" } else { "" };
        let file_str = editor
            .file_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(unsaved)".into());

        let length_str = if let Some(ref ts) = track_spline {
            format!("{:.1} m", track::spline_length(&ts.spline, 2000))
        } else {
            "N/A".into()
        };

        let selected_str = editor
            .selected_point
            .map(|i| format!("  |  Selected: #{i}"))
            .unwrap_or_default();

        let tw = editor.track_file.metadata.track_width;

        let curvature_str = if editor.show_curvature {
            "  |  Curvature: ON"
        } else {
            ""
        };

        **text = format!(
            "{name}{dirty_marker}  |  {file_str}\n\
             Points: {n_pts}  |  Width: {tw:.1}  |  Length: {length_str}{selected_str}{curvature_str}"
        );
    }

    // Help visibility
    if let Ok((mut vis, _text)) = help_text.single_mut() {
        *vis = if editor.show_help {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

// ---------------------------------------------------------------------------
// Gizmos: ruler, control polygon, highlight
// ---------------------------------------------------------------------------

fn draw_editor_gizmos(
    editor: Res<EditorState>,
    mut gizmos: Gizmos,
    track_spline: Option<Res<TrackSpline>>,
) {
    let pts = &editor.track_file.control_points;

    // Draw control polygon (lines between consecutive control points)
    if pts.len() >= 2 {
        let n = pts.len();
        for i in 0..n {
            let a = Vec2::new(pts[i][0], pts[i][1]);
            let b = Vec2::new(pts[(i + 1) % n][0], pts[(i + 1) % n][1]);
            gizmos.line_2d(a, b, Color::srgba(1.0, 1.0, 1.0, 0.15));
        }
    }

    // Highlight selected point with a larger ring
    if let Some(idx) = editor.selected_point {
        if idx < pts.len() {
            let p = Vec2::new(pts[idx][0], pts[idx][1]);
            gizmos.circle_2d(p, 2.0, css::AQUA);
        }
    }

    // Draw the spline itself as a gizmo line (thin, on top of meshes for clarity)
    if let Some(ref ts) = track_spline {
        let domain = ts.spline.domain();
        let t_max = domain.end();
        let n_samples = 500;
        let mut prev = ts.spline.position(0.0);
        for i in 1..=n_samples {
            let t = (i as f32 / n_samples as f32) * t_max;
            let p = ts.spline.position(t);
            gizmos.line_2d(prev, p, Color::srgba(1.0, 0.5, 0.0, 0.4));
            prev = p;
        }
    }

    // Ruler
    if let (Some(start), Some(end)) = (editor.ruler_start, editor.ruler_end) {
        gizmos.line_2d(start, end, css::WHITE);
        // Small crosses at endpoints
        let cross_sz = 1.0;
        gizmos.line_2d(
            start + Vec2::new(-cross_sz, -cross_sz),
            start + Vec2::new(cross_sz, cross_sz),
            css::WHITE,
        );
        gizmos.line_2d(
            start + Vec2::new(-cross_sz, cross_sz),
            start + Vec2::new(cross_sz, -cross_sz),
            css::WHITE,
        );
        gizmos.line_2d(
            end + Vec2::new(-cross_sz, -cross_sz),
            end + Vec2::new(cross_sz, cross_sz),
            css::WHITE,
        );
        gizmos.line_2d(
            end + Vec2::new(-cross_sz, cross_sz),
            end + Vec2::new(cross_sz, -cross_sz),
            css::WHITE,
        );
    }
}
