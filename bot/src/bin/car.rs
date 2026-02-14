#![no_std]
#![no_main]

use core::{f32::consts::PI, fmt::Write, panic::PanicInfo};

use bot::{
    driving::{CarControls, CarState, SplineQuery},
    log,
};

#[panic_handler]
fn panic(_panic: &PanicInfo<'_>) -> ! {
    writeln!(log(), "{}", _panic).ok(); // Do not panic in panic
    loop {}
}

#[unsafe(export_name = "main")]
fn main() -> ! {
    writeln!(log(), "Car OS starting up...").ok();

    let car_state = CarState::bind(bot::SLOT2);
    let mut car_controls = CarControls::bind(bot::SLOT3);
    let mut spline = SplineQuery::bind(bot::SLOT4);

    // Read t_max once at startup
    spline.query(0.0);
    let t_max = spline.t_max();

    // Track our position along the spline
    let mut target_t = 0.0;

    loop {
        let car_pos = car_state.position();
        let car_forward = car_state.forward();
        let car_speed = car_state.speed();

        // Search a small window around current target_t to find where we actually are
        let mut best_t = target_t;
        let mut best_score = f32::MAX;

        let window_samples = 50;
        let window_size = t_max * 0.1; // Search +/- 10% of track
        for i in 0..window_samples {
            let offset = (i as f32 / window_samples as f32) * window_size - window_size * 0.5;
            let test_t = (target_t + offset + t_max) % t_max;
            let test_pos = spline.query(test_t);
            let dist = car_pos.distance(test_pos);

            // Prefer points ahead (positive offset) over points behind
            let forward_bias = if offset > 0.0 { 0.0 } else { 2.0 };
            let score = dist + forward_bias;

            if score < best_score {
                best_score = score;
                best_t = test_t;
            }
        }

        // Dynamic lookahead based on speed
        let base_lookahead = 2.0;
        let speed_factor = (car_speed * 0.5).max(1.0);
        let lookahead_distance = base_lookahead * speed_factor;

        let mut current_t = best_t;
        let mut traveled = 0.0;

        // Walk along the spline until we've traveled lookahead_distance
        while traveled < lookahead_distance {
            let step = t_max / 2000.0; // Smaller steps for smoother distance calculation
            let next_t = (current_t + step) % t_max;
            let p1 = spline.query(current_t);
            let p2 = spline.query(next_t);
            traveled += p1.distance(p2);
            current_t = next_t;
        }

        target_t = current_t;

        let target_pos = spline.query(target_t);

        // Calculate steering to target
        let to_target = (target_pos - car_pos).normalize();
        let angle_to_target = car_forward.angle_to(to_target);

        // Smooth proportional steering with lower gain
        // Negate because physics uses -car.steer
        let max_steer = PI / 6.0;
        let desired_steer = (-angle_to_target * 0.8).clamp(-max_steer, max_steer);
        let steer_blend = 0.1; // How quickly to change steering (lower = smoother)
        car_controls.set_steering(
            car_controls.steering() * (1.0 - steer_blend) + desired_steer * steer_blend,
        );

        // Straight or gentle curve - full throttle
        car_controls.set_accelerator(1.0 * 0.1);
        car_controls.set_brake(0.0);
    }
}
