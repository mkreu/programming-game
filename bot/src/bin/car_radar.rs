#![no_std]
#![no_main]

use core::{f32::consts::PI, fmt::Write};

use botracers_bot_sdk::{
    driving::{CarControls, CarState, TrackRadar},
    log, SLOT2, SLOT3, SLOT5,
};

#[unsafe(export_name = "main")]
fn main() -> ! {
    writeln!(log(), "Car Radar OS starting up...").ok();

    let car_state = CarState::bind(SLOT2);
    let mut car_controls = CarControls::bind(SLOT3);
    let radar = TrackRadar::bind(SLOT5);

    let max_steer = PI / 6.0;
    let mut fallback_turn_dir = 1.0f32;

    loop {
        let speed = car_state.speed();
        let rays = radar.distances();

        // Ray order is right->left from current host implementation.
        let r0 = sanitize_distance(rays[0]);
        let r1 = sanitize_distance(rays[1]);
        let r2 = sanitize_distance(rays[2]);
        let c = sanitize_distance(rays[3]);
        let l0 = sanitize_distance(rays[4]);
        let l1 = sanitize_distance(rays[5]);
        let l2 = sanitize_distance(rays[6]);

        let right_clear = (r0 + r1 + r2) / 3.0;
        let left_clear = (l0 + l1 + l2) / 3.0;
        let side_balance = (right_clear - left_clear) * 0.035;

        // If front is tight, prioritize turning toward the side with more space.
        let front_urgency = ((20.0 - c) / 20.0).clamp(0.0, 1.0);
        let turn_bias = if right_clear > left_clear { 1.0 } else { -1.0 };

        let mut desired_steer = side_balance + turn_bias * front_urgency * 0.9;

        // If all rays are invalid/far, sweep gently to find the track again.
        let no_signal = rays.iter().all(|d| d.is_nan());
        if no_signal {
            desired_steer = fallback_turn_dir * 0.4;
            fallback_turn_dir = -fallback_turn_dir;
        }

        desired_steer = desired_steer.clamp(-max_steer, max_steer);

        // Smooth steering changes to reduce oscillation.
        let current_steer = car_controls.steering();
        let steer_blend = 0.18;
        car_controls
            .set_steering(current_steer * (1.0 - steer_blend) + desired_steer * steer_blend);

        // Speed policy from front clearance and current speed.
        let accel: f32 = 1.0;
        let mut brake: f32 = 0.0;
        
        if c < 20.0 && speed > 40.0/3.6 {
            brake = 1.0 //* 1.0f32.min((speed-10.0).max(0.0) / 5.0);
        } 

        // Additional high-speed caution when forward space is limited.
        /*if speed > 20.0 && c < 18.0 {
            accel = accel.min(0.03);
            brake = brake.max(0.25);
        }*/

        car_controls.set_accelerator(accel);
        car_controls.set_brake(brake);
    }
}

fn sanitize_distance(distance: f32) -> f32 {
    if distance.is_nan() {
        80.0
    } else {
        distance
    }
}
