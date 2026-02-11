#![no_std]
#![no_main]

use core::{f32::consts::PI, fmt::Write, panic::PanicInfo};

use bot::{
    driving::{CarControls, CarState},
    log,
};

#[panic_handler]
fn panic(_panic: &PanicInfo<'_>) -> ! {
    writeln!(log(), "{}", _panic).ok(); // Do not panic in panic
    loop {}
}

#[unsafe(export_name = "main")]
fn main() -> ! {
    let car_state = CarState::bind(bot::SLOT2);
    let mut car_controls = CarControls::bind(bot::SLOT3);
    loop {
        let target_pos = car_state.target();
        let car_pos = car_state.position();
        let car_forward = car_state.forward();
        // Calculate steering to target
        let to_target = (target_pos - car_pos).normalize();
        let angle_to_target = car_forward.angle_to(to_target);

        // Smooth proportional steering with lower gain
        // Negate because physics uses -car.steer
        let max_steer = PI / 6.0;
        let desired_steer = (-angle_to_target * 0.8).clamp(-max_steer, max_steer);
        let steer_blend = 0.1; // How quickly to change steering (lower = smoother)
        car_controls.set_steering(car_controls.steering() * (1.0 - steer_blend) + desired_steer * steer_blend);

        car_controls.set_accelerator(0.1); // Default to full throttle
    }
}
