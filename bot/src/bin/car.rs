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

        // Calculate curvature ahead to determine braking
        let curvature_lookahead = 15.0; // Look further ahead for braking
        let mut curve_t = best_t;
        let mut curve_traveled = 0.0;

        // Sample points ahead to measure curvature
        let mut max_curvature: f32 = 0.0;

        while curve_traveled < curvature_lookahead {
            let step = t_max / 2000.0;
            let next_t = (curve_t + step) % t_max;
            let prev_t = if curve_t < step {
                t_max + curve_t - step
            } else {
                curve_t - step
            };

            // Calculate curvature using three points
            let p_prev = spline.query(prev_t);
            let p_curr = spline.query(curve_t);
            let p_next = spline.query(next_t);

            let v1 = (p_curr - p_prev).normalize();
            let v2 = (p_next - p_curr).normalize();

            // Angle change indicates curvature
            let angle_change = v1.angle_to(v2).abs();
            max_curvature = max_curvature.max(angle_change);

            curve_traveled += p_curr.distance(p_next);
            curve_t = next_t;
        }

        let target_pos = spline.query(target_t);

        // Calculate steering to target
        let to_target = (target_pos - car_pos).normalize();
        let angle_to_target = car_forward.angle_to(to_target);

        // Smooth proportional steering with lower gain
        // Negate because physics uses -car.steer
        let max_steer = PI / 6.0;
        let desired_steer = (-angle_to_target * 0.8).clamp(-max_steer, max_steer);
        let steer_blend = 0.1; // How quickly to change steering (lower = smoother)
        car_controls.set_steering(car_controls.steering() * (1.0 - steer_blend) + desired_steer * steer_blend);

        // Determine acceleration/braking based on curvature ahead
        let curvature_threshold_brake = 0.05; // Start braking at tight turns
        let curvature_threshold_caution = 0.02; // Reduce throttle at moderate turns

        if max_curvature > curvature_threshold_brake {
            // Sharp turn ahead - brake
            car_controls.set_accelerator(0.0);
            car_controls.set_brake(((max_curvature - curvature_threshold_brake) * 10.0).min(1.0));
        } else if max_curvature > curvature_threshold_caution {
            // Moderate turn - reduce throttle
            let throttle_reduction = (max_curvature - curvature_threshold_caution)
                / (curvature_threshold_brake - curvature_threshold_caution);
            car_controls.set_accelerator((1.0 - throttle_reduction * 0.7).max(0.3) * 0.1);
            car_controls.set_brake(0.0);
        } else {
            // Straight or gentle curve - full throttle
            car_controls.set_accelerator(1.0 * 0.1);
            car_controls.set_brake(0.0);
        }
    }
}
