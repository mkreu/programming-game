use bevy::prelude::*;

use crate::main_game::{
    ArtifactFetchPipeline, CarLabel, DebugGizmos, DriverType, FollowCar, RaceManager, SimState,
    SpawnCarRequest, WebApiCommand, WebPortalState,
};

pub struct RaceUiPlugin;

impl Plugin for RaceUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiTextInputState>()
            .add_systems(Startup, setup_ui)
            .add_systems(
                Update,
                (
                    handle_focus_clicks,
                    handle_text_input,
                    update_car_list_ui,
                    handle_add_car_button,
                    handle_driver_selector,
                    handle_web_buttons,
                    update_web_ui_text,
                    update_pipeline_status,
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
struct CarListContainer;
#[derive(Component)]
struct ConsoleTextContainer;
#[derive(Component)]
struct AddCarButton;
#[derive(Component)]
struct StartButton;
#[derive(Component)]
struct ResetButton;
#[derive(Component)]
struct DriverSelectorButton;
#[derive(Component)]
struct DriverSelectorText;
#[derive(Component)]
struct PipelineStatusText;
#[derive(Component)]
struct WebStatusText;
#[derive(Component)]
struct ArtifactsText;
#[derive(Component)]
struct UsernameFieldText;
#[derive(Component)]
struct PasswordFieldText;
#[derive(Component)]
struct UsernameFieldButton;
#[derive(Component)]
struct PasswordFieldButton;
#[derive(Component)]
struct LoginButton;
#[derive(Component)]
struct LoadArtifactsButton;
#[derive(Component)]
struct UploadArtifactButton;
#[derive(Component)]
struct RemoveCarButton(Entity);
#[derive(Component)]
struct ToggleGizmosButton(Entity);
#[derive(Component)]
struct FollowCarButton(Entity);
#[derive(Component)]
struct CarListRow(#[allow(dead_code)] Entity);
#[derive(Component)]
struct ConsoleText;

#[derive(Resource, Default)]
struct UiTextInputState {
    focused: Option<FocusedField>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusedField {
    Username,
    Password,
}

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
                width: px(340.0),
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

            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(6.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Text::new("Driver:"),
                        text_font(14.0),
                        TextColor(LABEL_COLOR),
                    ));
                    row.spawn((
                        Button,
                        DriverSelectorButton,
                        button_style(),
                        BackgroundColor(BTN_BG),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("<No artifacts>"),
                            DriverSelectorText,
                            text_font(14.0),
                            TextColor(TEXT_COLOR),
                        ));
                    });
                });

            panel.spawn((
                Text::new("Artifact load: idle"),
                PipelineStatusText,
                text_font(12.0),
                TextColor(LABEL_COLOR),
            ));

            panel.spawn((
                Text::new("RaceHub"),
                text_font(16.0),
                TextColor(LABEL_COLOR),
            ));

            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4.0),
                    ..default()
                })
                .with_children(|web| {
                    web.spawn((
                        Button,
                        UsernameFieldButton,
                        button_style(),
                        BackgroundColor(BTN_BG),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Username: <click to edit>"),
                            UsernameFieldText,
                            text_font(13.0),
                            TextColor(TEXT_COLOR),
                        ));
                    });

                    web.spawn((
                        Button,
                        PasswordFieldButton,
                        button_style(),
                        BackgroundColor(BTN_BG),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Password: <click to edit>"),
                            PasswordFieldText,
                            text_font(13.0),
                            TextColor(TEXT_COLOR),
                        ));
                    });

                    web.spawn((Button, LoginButton, button_style(), BackgroundColor(BTN_BG)))
                        .with_children(|btn| {
                            btn.spawn((Text::new("Login"), text_font(14.0), TextColor(TEXT_COLOR)));
                        });

                    web.spawn((
                        Button,
                        LoadArtifactsButton,
                        button_style(),
                        BackgroundColor(BTN_BG),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Refresh Artifacts"),
                            text_font(14.0),
                            TextColor(TEXT_COLOR),
                        ));
                    });

                    web.spawn((
                        Button,
                        UploadArtifactButton,
                        button_style(),
                        BackgroundColor(BTN_BG),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Upload Artifact"),
                            text_font(14.0),
                            TextColor(TEXT_COLOR),
                        ));
                    });

                    web.spawn((
                        Text::new("Web status: idle"),
                        WebStatusText,
                        text_font(12.0),
                        TextColor(LABEL_COLOR),
                    ));
                    web.spawn((
                        Text::new("Artifacts: -"),
                        ArtifactsText,
                        text_font(12.0),
                        TextColor(LABEL_COLOR),
                    ));
                });

            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(6.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Button,
                        AddCarButton,
                        button_style(),
                        BackgroundColor(BTN_BG),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("+ Add Car"),
                            text_font(14.0),
                            TextColor(TEXT_COLOR),
                        ));
                    });
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

fn handle_driver_selector(
    query: Query<&Interaction, (Changed<Interaction>, With<DriverSelectorButton>)>,
    mut text_query: Query<&mut Text, With<DriverSelectorText>>,
    mut manager: ResMut<RaceManager>,
    web_state: Res<WebPortalState>,
) {
    let options = available_driver_options(&web_state);
    if options.is_empty() {
        for mut text in &mut text_query {
            text.0 = "<No artifacts>".to_string();
        }
        manager.selected_driver = None;
        return;
    }

    for interaction in &query {
        if *interaction == Interaction::Pressed {
            let current_index = options
                .iter()
                .position(|driver| Some(driver) == manager.selected_driver.as_ref())
                .unwrap_or(0);
            let next_index = (current_index + 1) % options.len();
            manager.selected_driver = Some(options[next_index].clone());

            for mut text in &mut text_query {
                text.0 = manager
                    .selected_driver
                    .as_ref()
                    .map(|d| d.label())
                    .unwrap_or_else(|| "<Invalid>".to_string());
            }
        }
    }
}

fn available_driver_options(web_state: &WebPortalState) -> Vec<DriverType> {
    web_state
        .artifacts
        .iter()
        .map(|artifact| DriverType::RemoteArtifact { id: artifact.id })
        .collect()
}

fn handle_focus_clicks(
    username_query: Query<&Interaction, (Changed<Interaction>, With<UsernameFieldButton>)>,
    password_query: Query<&Interaction, (Changed<Interaction>, With<PasswordFieldButton>)>,
    mut input_state: ResMut<UiTextInputState>,
) {
    for interaction in &username_query {
        if *interaction == Interaction::Pressed {
            input_state.focused = Some(FocusedField::Username);
        }
    }
    for interaction in &password_query {
        if *interaction == Interaction::Pressed {
            input_state.focused = Some(FocusedField::Password);
        }
    }
}

fn handle_text_input(
    mut keyboard_events: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut input_state: ResMut<UiTextInputState>,
    mut web_state: ResMut<WebPortalState>,
) {
    let Some(focused) = input_state.focused else {
        return;
    };

    for event in keyboard_events.read() {
        if event.state != bevy::input::ButtonState::Pressed {
            continue;
        }

        match event.logical_key {
            bevy::input::keyboard::Key::Escape => {
                input_state.focused = None;
            }
            bevy::input::keyboard::Key::Backspace => match focused {
                FocusedField::Username => {
                    web_state.username_input.pop();
                }
                FocusedField::Password => {
                    web_state.password_input.pop();
                }
            },
            bevy::input::keyboard::Key::Enter => {
                input_state.focused = None;
            }
            _ => {
                if let Some(text) = &event.text {
                    for ch in text.chars() {
                        if ch.is_control() {
                            continue;
                        }
                        match focused {
                            FocusedField::Username => web_state.username_input.push(ch),
                            FocusedField::Password => web_state.password_input.push(ch),
                        }
                    }
                }
            }
        }
    }
}

fn handle_web_buttons(
    login_query: Query<&Interaction, (Changed<Interaction>, With<LoginButton>)>,
    load_artifacts_query: Query<&Interaction, (Changed<Interaction>, With<LoadArtifactsButton>)>,
    upload_artifact_query: Query<&Interaction, (Changed<Interaction>, With<UploadArtifactButton>)>,
    mut web_commands: MessageWriter<WebApiCommand>,
) {
    for interaction in &login_query {
        if *interaction == Interaction::Pressed {
            web_commands.write(WebApiCommand::Login);
        }
    }
    for interaction in &load_artifacts_query {
        if *interaction == Interaction::Pressed {
            web_commands.write(WebApiCommand::LoadArtifacts);
        }
    }
    for interaction in &upload_artifact_query {
        if *interaction == Interaction::Pressed {
            web_commands.write(WebApiCommand::UploadArtifact);
        }
    }
}

fn update_web_ui_text(
    web_state: Res<WebPortalState>,
    input_state: Res<UiTextInputState>,
    mut texts: ParamSet<(
        Query<&mut Text, With<UsernameFieldText>>,
        Query<&mut Text, With<PasswordFieldText>>,
        Query<&mut Text, With<WebStatusText>>,
        Query<&mut Text, With<ArtifactsText>>,
    )>,
) {
    if !web_state.is_changed() && !input_state.is_changed() {
        return;
    }

    let username_prefix = if input_state.focused == Some(FocusedField::Username) {
        "Username*: "
    } else {
        "Username: "
    };
    let password_prefix = if input_state.focused == Some(FocusedField::Password) {
        "Password*: "
    } else {
        "Password: "
    };

    for mut text in &mut texts.p0() {
        let value = if web_state.username_input.is_empty() {
            "<click to edit>".to_string()
        } else {
            web_state.username_input.clone()
        };
        text.0 = format!("{username_prefix}{value}");
    }
    for mut text in &mut texts.p1() {
        let masked = if web_state.password_input.is_empty() {
            "<click to edit>".to_string()
        } else {
            "*".repeat(web_state.password_input.chars().count())
        };
        text.0 = format!("{password_prefix}{masked}");
    }
    for mut text in &mut texts.p2() {
        let user = web_state.logged_in_user.as_deref().unwrap_or("anonymous");
        let status = web_state.status_message.as_deref().unwrap_or("idle");
        let auth = match web_state.auth_required {
            Some(true) => "auth=required",
            Some(false) => "auth=disabled",
            None => "auth=?",
        };
        text.0 = format!("Web status ({user}, {auth}): {status}");
    }

    let artifact_summary = if web_state.artifacts.is_empty() {
        "-".to_string()
    } else {
        web_state
            .artifacts
            .iter()
            .take(4)
            .map(|a| format!("{}(#{} )", a.name, a.id))
            .collect::<Vec<_>>()
            .join(", ")
    };
    for mut text in &mut texts.p3() {
        text.0 = format!(
            "Artifacts ({}): {}",
            web_state.artifacts.len(),
            artifact_summary
        );
    }
}

fn handle_add_car_button(
    query: Query<&Interaction, (Changed<Interaction>, With<AddCarButton>)>,
    mut spawn_events: MessageWriter<SpawnCarRequest>,
    manager: Res<RaceManager>,
    state: Res<State<SimState>>,
) {
    if *state.get() != SimState::PreRace {
        return;
    }
    for interaction in &query {
        if *interaction == Interaction::Pressed {
            if let Some(driver) = &manager.selected_driver {
                spawn_events.write(SpawnCarRequest {
                    driver: driver.clone(),
                });
            }
        }
    }
}

fn update_pipeline_status(
    pipeline: Res<ArtifactFetchPipeline>,
    mut text_query: Query<&mut Text, With<PipelineStatusText>>,
) {
    if !pipeline.is_changed() {
        return;
    }

    let message = pipeline
        .status_message
        .clone()
        .unwrap_or_else(|| "Artifact load: idle".to_string());

    for mut text in &mut text_query {
        text.0 = message.clone();
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
                    btn.spawn((Text::new("👁"), text_font(12.0), TextColor(TEXT_COLOR)));
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
                    btn.spawn((Text::new("🔧"), text_font(12.0), TextColor(TEXT_COLOR)));
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
                    btn.spawn((Text::new("✕"), text_font(12.0), TextColor(TEXT_COLOR)));
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
                Text::new(format!("── {} ──", entry.name)),
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
