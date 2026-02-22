use bevy::prelude::*;

use crate::main_game::{
    CarLabel, DebugGizmos, DriverType, FollowCar, RaceManager, SimState, SpawnCarRequest,
    WebApiCommand, WebPortalState,
};

pub struct RaceUiPlugin;

impl Plugin for RaceUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui)
            .add_systems(
                Update,
                (
                    handle_web_buttons,
                    update_web_status_dialog,
                    update_artifact_list_ui,
                    handle_artifact_spawn_button,
                    handle_artifact_delete_button,
                    handle_artifact_visibility_button,
                    update_car_list_ui,
                    handle_remove_car_button,
                    handle_toggle_gizmos_button,
                ),
            )
            .add_systems(
                Update,
                (
                    handle_follow_car_button,
                    handle_start_button,
                    handle_reset_button,
                    update_console_output,
                    update_start_button_text,
                ),
            );
    }
}

#[derive(Component)]
struct UiRoot;
#[derive(Component)]
struct StatusDialogText;
#[derive(Component)]
struct ArtifactListContainer;
#[derive(Component)]
struct ArtifactListRow(#[allow(dead_code)] i64);
#[derive(Component)]
struct RefreshArtifactsButton;
#[derive(Component)]
struct UploadArtifactButton;
#[derive(Component)]
struct SpawnArtifactButton(i64);
#[derive(Component)]
struct DeleteArtifactButton(i64);
#[derive(Component)]
struct ToggleArtifactVisibilityButton(i64, bool);
#[derive(Component)]
struct StartButton;
#[derive(Component)]
struct ResetButton;
#[derive(Component)]
struct CarListContainer;
#[derive(Component)]
struct RemoveCarButton(Entity);
#[derive(Component)]
struct ToggleGizmosButton(Entity);
#[derive(Component)]
struct FollowCarButton(Entity);
#[derive(Component)]
struct CarListRow(#[allow(dead_code)] Entity);
#[derive(Component)]
struct ConsoleTextContainer;
#[derive(Component)]
struct ConsoleText;

const PANEL_BG: Color = Color::srgba(0.08, 0.08, 0.12, 0.92);
const BTN_BG: Color = Color::srgb(0.25, 0.25, 0.35);
const START_BG: Color = Color::srgb(0.15, 0.55, 0.2);
const RESET_BG: Color = Color::srgb(0.55, 0.15, 0.15);
const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);
const LABEL_COLOR: Color = Color::srgb(0.7, 0.7, 0.7);

fn px(val: f32) -> Val {
    Val::Px(val)
}

fn text_font(size: f32) -> TextFont {
    TextFont {
        font_size: size,
        ..default()
    }
}

fn button_style() -> Node {
    Node {
        padding: UiRect::axes(px(10.0), px(4.0)),
        margin: UiRect::all(px(2.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

fn setup_ui(mut commands: Commands) {
    commands
        .spawn((
            UiRoot,
            Node {
                position_type: PositionType::Absolute,
                right: px(0.0),
                top: px(0.0),
                bottom: px(0.0),
                width: px(380.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(10.0)),
                row_gap: px(8.0),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            Pickable::default(),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("Race Control"),
                text_font(22.0),
                TextColor(TEXT_COLOR),
            ));

            panel.spawn((
                Text::new("Server Status"),
                text_font(16.0),
                TextColor(LABEL_COLOR),
            ));

            panel
                .spawn((
                    Node {
                        padding: UiRect::all(px(6.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                ))
                .with_children(|dialog| {
                    dialog.spawn((
                        Text::new("[status] Waiting for server response..."),
                        StatusDialogText,
                        text_font(12.0),
                        TextColor(TEXT_COLOR),
                    ));
                });

            panel.spawn((
                Text::new("Artifacts"),
                text_font(16.0),
                TextColor(LABEL_COLOR),
            ));

            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(6.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Button,
                        RefreshArtifactsButton,
                        button_style(),
                        BackgroundColor(BTN_BG),
                    ))
                    .with_children(|btn| {
                        btn.spawn((Text::new("Refresh"), text_font(14.0), TextColor(TEXT_COLOR)));
                    });

                    row.spawn((
                        Button,
                        UploadArtifactButton,
                        button_style(),
                        BackgroundColor(BTN_BG),
                    ))
                    .with_children(|btn| {
                        btn.spawn((Text::new("Upload"), text_font(14.0), TextColor(TEXT_COLOR)));
                    });
                });

            panel
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(4.0),
                        overflow: Overflow::scroll_y(),
                        max_height: px(220.0),
                        ..default()
                    },
                    ArtifactListContainer,
                ))
                .with_children(|_| {});

            panel.spawn((Text::new("Race"), text_font(16.0), TextColor(LABEL_COLOR)));

            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(6.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Button,
                        StartButton,
                        button_style(),
                        BackgroundColor(START_BG),
                    ))
                    .with_children(|btn| {
                        btn.spawn((Text::new("Start"), text_font(14.0), TextColor(TEXT_COLOR)));
                    });

                    row.spawn((
                        Button,
                        ResetButton,
                        button_style(),
                        BackgroundColor(RESET_BG),
                    ))
                    .with_children(|btn| {
                        btn.spawn((Text::new("Reset"), text_font(14.0), TextColor(TEXT_COLOR)));
                    });
                });

            panel.spawn((Text::new("Cars"), text_font(16.0), TextColor(LABEL_COLOR)));

            panel
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(4.0),
                        overflow: Overflow::scroll_y(),
                        max_height: px(220.0),
                        ..default()
                    },
                    CarListContainer,
                ))
                .with_children(|_| {});

            panel.spawn((
                Text::new("Console"),
                text_font(16.0),
                TextColor(LABEL_COLOR),
            ));

            panel.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    overflow: Overflow::scroll_y(),
                    padding: UiRect::all(px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                ConsoleTextContainer,
            ));
        });
}

fn handle_web_buttons(
    refresh_query: Query<&Interaction, (Changed<Interaction>, With<RefreshArtifactsButton>)>,
    upload_query: Query<&Interaction, (Changed<Interaction>, With<UploadArtifactButton>)>,
    mut web_commands: MessageWriter<WebApiCommand>,
) {
    for interaction in &refresh_query {
        if *interaction == Interaction::Pressed {
            web_commands.write(WebApiCommand::LoadArtifacts);
        }
    }

    for interaction in &upload_query {
        if *interaction == Interaction::Pressed {
            web_commands.write(WebApiCommand::UploadArtifact);
        }
    }
}

fn update_web_status_dialog(
    web_state: Res<WebPortalState>,
    mut text_query: Query<&mut Text, With<StatusDialogText>>,
) {
    if !web_state.is_changed() {
        return;
    }

    let status = web_state
        .status_message
        .as_deref()
        .unwrap_or("[status] idle");

    for mut text in &mut text_query {
        text.0 = status.to_string();
    }
}

fn update_artifact_list_ui(
    web_state: Res<WebPortalState>,
    mut commands: Commands,
    container_query: Query<Entity, With<ArtifactListContainer>>,
    existing_rows: Query<Entity, With<ArtifactListRow>>,
) {
    if !web_state.is_changed() {
        return;
    }

    let Ok(container) = container_query.single() else {
        return;
    };

    for row_entity in &existing_rows {
        commands.entity(row_entity).despawn();
    }

    for artifact in &web_state.artifacts {
        let artifact_id = artifact.id;
        let visibility = if artifact.is_public {
            "public"
        } else {
            "private"
        };
        let label = format!(
            "{} [#{}] by {} ({})",
            artifact.name, artifact.id, artifact.owner_username, visibility
        );

        commands.entity(container).with_children(|list| {
            list.spawn((
                ArtifactListRow(artifact_id),
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(4.0),
                    padding: UiRect::axes(px(4.0), px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.15, 0.15, 0.2, 0.8)),
            ))
            .with_children(|row| {
                row.spawn((
                    Text::new(label),
                    text_font(13.0),
                    TextColor(TEXT_COLOR),
                    Node {
                        flex_grow: 1.0,
                        ..default()
                    },
                ));

                row.spawn((
                    Button,
                    SpawnArtifactButton(artifact_id),
                    Node {
                        padding: UiRect::axes(px(6.0), px(2.0)),
                        ..default()
                    },
                    BackgroundColor(BTN_BG),
                ))
                .with_children(|btn| {
                    btn.spawn((Text::new("Spawn"), text_font(12.0), TextColor(TEXT_COLOR)));
                });

                if artifact.owned_by_me {
                    row.spawn((
                        Button,
                        ToggleArtifactVisibilityButton(artifact_id, !artifact.is_public),
                        Node {
                            padding: UiRect::axes(px(6.0), px(2.0)),
                            ..default()
                        },
                        BackgroundColor(BTN_BG),
                    ))
                    .with_children(|btn| {
                        let text = if artifact.is_public {
                            "Make Private"
                        } else {
                            "Make Public"
                        };
                        btn.spawn((Text::new(text), text_font(12.0), TextColor(TEXT_COLOR)));
                    });

                    row.spawn((
                        Button,
                        DeleteArtifactButton(artifact_id),
                        Node {
                            padding: UiRect::axes(px(6.0), px(2.0)),
                            ..default()
                        },
                        BackgroundColor(RESET_BG),
                    ))
                    .with_children(|btn| {
                        btn.spawn((Text::new("Delete"), text_font(12.0), TextColor(TEXT_COLOR)));
                    });
                }
            });
        });
    }
}

fn handle_artifact_spawn_button(
    query: Query<(&Interaction, &SpawnArtifactButton), Changed<Interaction>>,
    mut spawn_events: MessageWriter<SpawnCarRequest>,
    state: Res<State<SimState>>,
) {
    if *state.get() != SimState::PreRace {
        return;
    }

    for (interaction, spawn_btn) in &query {
        if *interaction == Interaction::Pressed {
            spawn_events.write(SpawnCarRequest {
                driver: DriverType::RemoteArtifact { id: spawn_btn.0 },
            });
        }
    }
}

fn handle_artifact_delete_button(
    query: Query<(&Interaction, &DeleteArtifactButton), Changed<Interaction>>,
    mut web_commands: MessageWriter<WebApiCommand>,
    state: Res<State<SimState>>,
) {
    if *state.get() != SimState::PreRace {
        return;
    }

    for (interaction, delete_btn) in &query {
        if *interaction == Interaction::Pressed {
            web_commands.write(WebApiCommand::DeleteArtifact { id: delete_btn.0 });
        }
    }
}

fn handle_artifact_visibility_button(
    query: Query<(&Interaction, &ToggleArtifactVisibilityButton), Changed<Interaction>>,
    mut web_commands: MessageWriter<WebApiCommand>,
    state: Res<State<SimState>>,
) {
    if *state.get() != SimState::PreRace {
        return;
    }

    for (interaction, toggle_btn) in &query {
        if *interaction == Interaction::Pressed {
            web_commands.write(WebApiCommand::SetArtifactVisibility {
                id: toggle_btn.0,
                is_public: toggle_btn.1,
            });
        }
    }
}

fn handle_start_button(
    query: Query<&Interaction, (Changed<Interaction>, With<StartButton>)>,
    current_state: Res<State<SimState>>,
    mut next_state: ResMut<NextState<SimState>>,
) {
    for interaction in &query {
        if *interaction == Interaction::Pressed {
            match current_state.get() {
                SimState::PreRace => {
                    next_state.set(SimState::Racing);
                }
                SimState::Racing => {
                    next_state.set(SimState::Paused);
                }
                SimState::Paused => {
                    next_state.set(SimState::Racing);
                }
            }
        }
    }
}

fn update_start_button_text(
    state: Res<State<SimState>>,
    start_btn_query: Query<&Children, With<StartButton>>,
    mut text_query: Query<&mut Text>,
) {
    if !state.is_changed() {
        return;
    }
    for children in &start_btn_query {
        for child in children.iter() {
            if let Ok(mut text) = text_query.get_mut(child) {
                text.0 = match state.get() {
                    SimState::PreRace => "Start".into(),
                    SimState::Racing => "Pause".into(),
                    SimState::Paused => "Resume".into(),
                };
            }
        }
    }
}

fn handle_reset_button(
    query: Query<&Interaction, (Changed<Interaction>, With<ResetButton>)>,
    mut next_state: ResMut<NextState<SimState>>,
    mut manager: ResMut<RaceManager>,
    car_query: Query<Entity, With<CarLabel>>,
    mut commands: Commands,
) {
    for interaction in &query {
        if *interaction == Interaction::Pressed {
            for entity in &car_query {
                commands.entity(entity).despawn();
            }
            manager.cars.clear();
            manager.next_car_id = 1;
            next_state.set(SimState::PreRace);
        }
    }
}

fn handle_remove_car_button(
    query: Query<(&Interaction, &RemoveCarButton), Changed<Interaction>>,
    mut manager: ResMut<RaceManager>,
    mut commands: Commands,
    state: Res<State<SimState>>,
) {
    if *state.get() != SimState::PreRace {
        return;
    }
    for (interaction, remove_btn) in &query {
        if *interaction == Interaction::Pressed {
            let entity = remove_btn.0;
            commands.entity(entity).despawn();
            manager.cars.retain(|c| c.entity != entity);
        }
    }
}

fn handle_toggle_gizmos_button(
    query: Query<(&Interaction, &ToggleGizmosButton), Changed<Interaction>>,
    mut commands: Commands,
    gizmo_query: Query<(), With<DebugGizmos>>,
) {
    for (interaction, toggle_btn) in &query {
        if *interaction == Interaction::Pressed {
            let entity = toggle_btn.0;
            if gizmo_query.get(entity).is_ok() {
                commands.entity(entity).remove::<DebugGizmos>();
            } else {
                commands.entity(entity).insert(DebugGizmos);
            }
        }
    }
}

fn handle_follow_car_button(
    query: Query<(&Interaction, &FollowCarButton), Changed<Interaction>>,
    mut follow: ResMut<FollowCar>,
) {
    for (interaction, follow_btn) in &query {
        if *interaction == Interaction::Pressed {
            if follow.target == Some(follow_btn.0) {
                follow.target = None;
            } else {
                follow.target = Some(follow_btn.0);
            }
        }
    }
}

fn update_car_list_ui(
    manager: Res<RaceManager>,
    mut commands: Commands,
    container_query: Query<Entity, With<CarListContainer>>,
    existing_rows: Query<(Entity, &CarListRow)>,
    gizmo_query: Query<(), With<DebugGizmos>>,
    follow: Res<FollowCar>,
) {
    if !manager.is_changed() && !follow.is_changed() {
        return;
    }

    let Ok(container) = container_query.single() else {
        return;
    };

    for (row_entity, _) in &existing_rows {
        commands.entity(row_entity).despawn();
    }

    for entry in &manager.cars {
        let entity = entry.entity;
        let has_gizmos = gizmo_query.get(entity).is_ok();
        let is_followed = follow.target == Some(entity);
        let driver_label = entry.driver.label();

        commands.entity(container).with_children(|list| {
            list.spawn((
                CarListRow(entity),
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(4.0),
                    padding: UiRect::axes(px(4.0), px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.15, 0.15, 0.2, 0.8)),
            ))
            .with_children(|row| {
                row.spawn((
                    Text::new(format!("{} [{}]", entry.name, driver_label)),
                    text_font(13.0),
                    TextColor(TEXT_COLOR),
                    Node {
                        flex_grow: 1.0,
                        ..default()
                    },
                ));

                let follow_bg = if is_followed {
                    Color::srgb(0.2, 0.5, 0.7)
                } else {
                    BTN_BG
                };
                row.spawn((
                    Button,
                    FollowCarButton(entity),
                    Node {
                        padding: UiRect::axes(px(6.0), px(2.0)),
                        ..default()
                    },
                    BackgroundColor(follow_bg),
                ))
                .with_children(|btn| {
                    btn.spawn((Text::new("Follow"), text_font(12.0), TextColor(TEXT_COLOR)));
                });

                let gizmo_bg = if has_gizmos {
                    Color::srgb(0.2, 0.6, 0.3)
                } else {
                    BTN_BG
                };
                row.spawn((
                    Button,
                    ToggleGizmosButton(entity),
                    Node {
                        padding: UiRect::axes(px(6.0), px(2.0)),
                        ..default()
                    },
                    BackgroundColor(gizmo_bg),
                ))
                .with_children(|btn| {
                    btn.spawn((Text::new("Gizmos"), text_font(12.0), TextColor(TEXT_COLOR)));
                });

                row.spawn((
                    Button,
                    RemoveCarButton(entity),
                    Node {
                        padding: UiRect::axes(px(6.0), px(2.0)),
                        ..default()
                    },
                    BackgroundColor(RESET_BG),
                ))
                .with_children(|btn| {
                    btn.spawn((Text::new("Remove"), text_font(12.0), TextColor(TEXT_COLOR)));
                });
            });
        });
    }
}

fn update_console_output(
    mut manager: ResMut<RaceManager>,
    mut cpu_query: Query<(&CarLabel, &mut emulator::cpu::LogDevice)>,
    container_query: Query<Entity, With<ConsoleTextContainer>>,
    mut commands: Commands,
    existing_texts: Query<Entity, (With<Text>, With<ConsoleText>)>,
) {
    let mut any_new = false;
    for (label, mut log_dev) in &mut cpu_query {
        let output = log_dev.drain_output();
        if !output.is_empty() {
            if let Some(entry) = manager.cars.iter_mut().find(|c| c.name == label.name) {
                entry.console_output.push_str(&output);
                if entry.console_output.len() > 8192 {
                    let start = entry.console_output.len() - 4096;
                    let trimmed = entry.console_output[start..].to_string();
                    entry.console_output = trimmed;
                }
                any_new = true;
            }
        }
    }

    if !any_new {
        return;
    }

    let Ok(container) = container_query.single() else {
        return;
    };

    for entity in &existing_texts {
        commands.entity(entity).despawn();
    }

    commands.entity(container).with_children(|console| {
        for entry in &manager.cars {
            if entry.console_output.is_empty() {
                continue;
            }
            console.spawn((
                Text::new(format!("-- {} --", entry.name)),
                text_font(12.0),
                TextColor(Color::srgb(0.5, 0.8, 1.0)),
                ConsoleText,
            ));
            let lines: Vec<&str> = entry.console_output.lines().collect();
            let start = lines.len().saturating_sub(40);
            let display = lines[start..].join("\n");
            console.spawn((
                Text::new(display),
                text_font(11.0),
                TextColor(Color::srgb(0.75, 0.75, 0.75)),
                ConsoleText,
            ));
        }
    });
}
