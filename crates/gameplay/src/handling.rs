use ow_math::{Quat, Vec3};

/// Vehicle drivetrain layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrivetrainType {
    RearWheelDrive,
    FrontWheelDrive,
    AllWheelDrive,
}

/// Comprehensive vehicle handling configuration matching RAGE handling.dat parameters.
#[derive(Debug, Clone)]
pub struct HandlingConfig {
    /// Total vehicle curb mass in kilograms.
    pub mass: f32,
    /// Aerodynamic air resistance drag coefficient.
    pub drag_coefficient: f32,
    /// Aerodynamic downforce multiplier at speed.
    pub downforce_mult: f32,
    /// Drivetrain configuration (RWD, FWD, AWD).
    pub drivetrain: DrivetrainType,
    /// Torque bias sent to front wheels: 0.0 (100% rear) to 1.0 (100% front).
    pub drive_bias_front: f32,
    /// Number of forward gear ratios.
    pub gear_count: usize,
    /// Gear transmission ratios [Reverse, 1st, 2nd, 3rd, 4th, 5th, 6th].
    pub gear_ratios: [f32; 7],
    /// Final drive axle differential ratio.
    pub final_drive_ratio: f32,
    /// Peak engine drive force in Newtons.
    pub drive_force: f32,
    /// Peak hydraulic service brake force in Newtons.
    pub brake_force: f32,
    /// Rear axle mechanical handbrake lock force.
    pub handbrake_force: f32,
    /// Maximum front wheel steering lock angle in radians.
    pub steering_lock: f32,
    /// Suspension spring rate stiffness in N/m.
    pub suspension_stiffness: f32,
    /// Suspension bump / compression shock damping.
    pub suspension_comp_damping: f32,
    /// Suspension rebound shock damping.
    pub suspension_rebound_damping: f32,
    /// Maximum vertical suspension suspension travel in meters.
    pub suspension_travel: f32,
    /// Uncompressed suspension spring rest length in meters.
    pub suspension_rest_length: f32,
    /// Anti-roll bar torsional spring rate preventing excessive body roll.
    pub anti_roll_bar_stiffness: f32,
    /// Lateral tire cornering grip friction coefficient.
    pub tire_grip_lateral: f32,
    /// Longitudinal tire drive and braking traction coefficient.
    pub tire_grip_longitudinal: f32,
}

impl Default for HandlingConfig {
    fn default() -> Self {
        Self {
            mass: 1550.0,
            drag_coefficient: 0.32,
            downforce_mult: 1.5,
            drivetrain: DrivetrainType::AllWheelDrive,
            drive_bias_front: 0.4, // 40% front, 60% rear (RAGE sports AWD bias)
            gear_count: 6,
            gear_ratios: [-3.2, 3.8, 2.4, 1.7, 1.3, 1.0, 0.8],
            final_drive_ratio: 3.42,
            drive_force: 5200.0,
            brake_force: 8500.0,
            handbrake_force: 14000.0,
            steering_lock: 0.61, // ~ 35 degrees
            suspension_stiffness: 38000.0,
            suspension_comp_damping: 3400.0,
            suspension_rebound_damping: 4200.0,
            suspension_travel: 0.22,
            suspension_rest_length: 0.45,
            anti_roll_bar_stiffness: 9500.0,
            tire_grip_lateral: 1.30,
            tire_grip_longitudinal: 1.35,
        }
    }
}

/// Identifiers for each of the 4 independent wheel corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelCorner {
    FrontLeft = 0,
    FrontRight = 1,
    RearLeft = 2,
    RearRight = 3,
}

/// State of an individual wheel including raycast suspension and tire slip dynamics.
#[derive(Debug, Clone)]
pub struct WheelState {
    pub corner: WheelCorner,
    pub relative_mount_point: Vec3,
    pub is_steering: bool,
    pub is_driven: bool,
    pub wheel_radius: f32,
    pub steer_angle: f32,
    pub current_suspension_length: f32,
    pub suspension_velocity: f32,
    pub is_grounded: bool,
    pub contact_normal: Vec3,
    pub contact_point: Vec3,
    pub angular_velocity: f32,
    pub slip_ratio: f32,
    pub slip_angle: f32,
    pub skid_intensity: f32,
}

impl WheelState {
    pub fn new(corner: WheelCorner, mount: Vec3, is_steering: bool, is_driven: bool) -> Self {
        Self {
            corner,
            relative_mount_point: mount,
            is_steering,
            is_driven,
            wheel_radius: 0.34,
            steer_angle: 0.0,
            current_suspension_length: 0.45,
            suspension_velocity: 0.0,
            is_grounded: false,
            contact_normal: Vec3::UP,
            contact_point: Vec3::ZERO,
            angular_velocity: 0.0,
            slip_ratio: 0.0,
            slip_angle: 0.0,
            skid_intensity: 0.0,
        }
    }
}

/// 4-Wheel raycast vehicle physics simulation with RAGE transmission, suspension, and tire grip.
#[derive(Debug, Clone)]
pub struct FourWheelVehicle {
    pub handling: HandlingConfig,
    pub position: Vec3,
    pub rotation: Quat,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3, // Pitch, Yaw, Roll rates
    pub wheels: [WheelState; 4],
    pub engine_rpm: f32,
    pub current_gear: i32, // -1 = Reverse, 0 = Neutral, 1..6 = Forward
    pub throttle: f32,
    pub brake: f32,
    pub handbrake: bool,
    pub steer_input: f32,
    pub is_engine_running: bool,
}

impl FourWheelVehicle {
    pub fn new(handling: HandlingConfig) -> Self {
        let half_track = 0.85; // 1.70m vehicle width
        let half_wheelbase = 1.35; // 2.70m wheelbase
        let mount_height = 0.35;

        let is_fwd = handling.drivetrain == DrivetrainType::FrontWheelDrive || handling.drivetrain == DrivetrainType::AllWheelDrive;
        let is_rwd = handling.drivetrain == DrivetrainType::RearWheelDrive || handling.drivetrain == DrivetrainType::AllWheelDrive;

        let wheels = [
            WheelState::new(WheelCorner::FrontLeft, Vec3::new(-half_track, mount_height, half_wheelbase), true, is_fwd),
            WheelState::new(WheelCorner::FrontRight, Vec3::new(half_track, mount_height, half_wheelbase), true, is_fwd),
            WheelState::new(WheelCorner::RearLeft, Vec3::new(-half_track, mount_height, -half_wheelbase), false, is_rwd),
            WheelState::new(WheelCorner::RearRight, Vec3::new(half_track, mount_height, -half_wheelbase), false, is_rwd),
        ];

        Self {
            handling,
            position: Vec3::new(0.0, 0.5, 0.0),
            rotation: Quat::IDENTITY,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            wheels,
            engine_rpm: 800.0, // Idle RPM
            current_gear: 1,
            throttle: 0.0,
            brake: 0.0,
            handbrake: false,
            steer_input: 0.0,
            is_engine_running: true,
        }
    }

    pub fn set_inputs(&mut self, throttle: f32, brake: f32, handbrake: bool, steer: f32) {
        self.throttle = throttle.clamp(0.0, 1.0);
        self.brake = brake.clamp(0.0, 1.0);
        self.handbrake = handbrake;
        self.steer_input = steer.clamp(-1.0, 1.0);
    }

    pub fn forward_vector(&self) -> Vec3 {
        self.rotation.rotate_vector(Vec3::FORWARD)
    }

    pub fn right_vector(&self) -> Vec3 {
        self.rotation.rotate_vector(Vec3::RIGHT)
    }

    pub fn up_vector(&self) -> Vec3 {
        self.rotation.rotate_vector(Vec3::UP)
    }

    pub fn get_speed_kmh(&self) -> f32 {
        self.linear_velocity.dot(&self.forward_vector()) * 3.6
    }

    /// Advances the 4-wheel physics simulation by delta time.
    pub fn update(&mut self, dt: f32, ground_y: f32) {
        if dt <= 0.0 {
            return;
        }

        let _forward = self.forward_vector();
        let _right = self.right_vector();
        let up = self.up_vector();

        // 1. Steering input with speed-sensitive lock reduction (RAGE steering curve)
        let speed_kmh = self.get_speed_kmh().abs();
        let speed_ratio = (speed_kmh / 120.0).min(0.7); // Reduces max lock at high speeds
        let effective_lock = self.handling.steering_lock * (1.0 - speed_ratio);
        let target_steer = self.steer_input * effective_lock;

        for w in &mut self.wheels {
            if w.is_steering {
                w.steer_angle = target_steer;
            } else {
                w.steer_angle = 0.0;
            }
        }

        // 2. Wheel Raycast Suspension & Contact Evaluation
        let mut total_suspension_force = Vec3::ZERO;
        let mut total_tire_force = Vec3::ZERO;
        let mut total_torque = Vec3::ZERO;

        let max_ray_len = self.handling.suspension_rest_length + 0.34; // Spring rest + tire radius

        for w in &mut self.wheels {
            let world_mount = self.position + self.rotation.rotate_vector(w.relative_mount_point);
            let ray_origin = world_mount;
            let wheel_bottom = ray_origin.y - max_ray_len;

            if wheel_bottom <= ground_y {
                w.is_grounded = true;
                w.contact_point = Vec3::new(ray_origin.x, ground_y, ray_origin.z);
                w.contact_normal = Vec3::UP;

                let hit_distance = (ray_origin.y - ground_y).max(0.0);
                let current_length = hit_distance - w.wheel_radius;
                let compression = (self.handling.suspension_rest_length - current_length)
                    .clamp(0.0, self.handling.suspension_travel);

                let comp_velocity = (w.current_suspension_length - current_length) / dt;
                w.suspension_velocity = comp_velocity;
                w.current_suspension_length = current_length;

                // Hooke's Law + Damper shock force
                let spring_force = compression * self.handling.suspension_stiffness;
                let damper_force = if comp_velocity >= 0.0 {
                    comp_velocity * self.handling.suspension_comp_damping
                } else {
                    comp_velocity * self.handling.suspension_rebound_damping
                };

                let normal_load = (spring_force + damper_force).max(0.0);
                let susp_force_vec = up * normal_load;
                total_suspension_force = total_suspension_force + susp_force_vec;

                // Moment arm around center of mass
                let arm = world_mount - self.position;
                total_torque = total_torque + arm.cross(susp_force_vec);

                // 3. Wheel Heading and Local Velocity
                let wheel_rot = Quat::from_axis_angle(up, w.steer_angle);
                let wheel_forward = (wheel_rot * self.rotation).rotate_vector(Vec3::FORWARD);
                let wheel_right = (wheel_rot * self.rotation).rotate_vector(Vec3::RIGHT);

                // Linear velocity at the wheel contact patch
                let wheel_velocity = self.linear_velocity + self.angular_velocity.cross(arm);
                let forward_speed = wheel_velocity.dot(&wheel_forward);
                let lateral_speed = wheel_velocity.dot(&wheel_right);

                // Tire slip calculations
                w.slip_angle = if forward_speed.abs() > 0.5 {
                    (-lateral_speed / forward_speed.abs()).atan()
                } else {
                    0.0
                };

                // Longitudinal traction (Engine Drive or Brakes)
                let mut long_force_mag = 0.0;
                if self.is_engine_running && w.is_driven && self.throttle > 0.0 {
                    let drive_fraction = if w.is_steering {
                        self.handling.drive_bias_front
                    } else {
                        1.0 - self.handling.drive_bias_front
                    };
                    long_force_mag += self.handling.drive_force * self.throttle * drive_fraction * 0.5;
                }

                if self.brake > 0.0 {
                    let brake_sign = if forward_speed > 0.0 { -1.0 } else { 1.0 };
                    long_force_mag += self.handling.brake_force * self.brake * 0.25 * brake_sign;
                }

                if self.handbrake && !w.is_steering {
                    let brake_sign = if forward_speed > 0.0 { -1.0 } else { 1.0 };
                    long_force_mag += self.handling.handbrake_force * 0.5 * brake_sign;
                }

                // Lateral cornering friction (Pacejka approximation)
                let lateral_friction = if self.handbrake && !w.is_steering {
                    // Handbrake causes rear tire lateral slip break (drift initiated)
                    self.handling.tire_grip_lateral * 0.35
                } else {
                    self.handling.tire_grip_lateral
                };

                let lateral_force_mag = -lateral_speed * normal_load * lateral_friction * 0.08;
                let lateral_force_clamped = lateral_force_mag.clamp(-normal_load * 1.5, normal_load * 1.5);

                let tire_force_vec = wheel_forward * long_force_mag + wheel_right * lateral_force_clamped;
                total_tire_force = total_tire_force + tire_force_vec;
                total_torque = total_torque + arm.cross(tire_force_vec);

                // Skid / smoke intensity
                let total_slip = (w.slip_angle.abs() * 2.0 + (lateral_speed.abs() / 15.0)).min(1.0);
                w.skid_intensity = if self.handbrake && !w.is_steering {
                    1.0
                } else if total_slip > 0.35 {
                    total_slip
                } else {
                    0.0
                };
            } else {
                w.is_grounded = false;
                w.current_suspension_length = self.handling.suspension_rest_length;
                w.skid_intensity = 0.0;
            }
        }

        // 4. Aerodynamic Drag and Downforce
        let current_speed = self.linear_velocity.length();
        let drag_force = -self.linear_velocity * (0.5 * 1.225 * self.handling.drag_coefficient * current_speed);
        let downforce = -up * (self.handling.downforce_mult * current_speed * current_speed * 0.5);

        // 5. Gravity
        let gravity_force = Vec3::new(0.0, -9.81 * self.handling.mass, 0.0);

        // Net Forces and Acceleration
        let net_force = total_suspension_force + total_tire_force + drag_force + downforce + gravity_force;
        let linear_accel = net_force / self.handling.mass;

        self.linear_velocity = self.linear_velocity + linear_accel * dt;
        self.position = self.position + self.linear_velocity * dt;

        // Ground clamping prevention from falling through floor
        if self.position.y < ground_y + 0.34 {
            self.position.y = ground_y + 0.34;
            if self.linear_velocity.y < 0.0 {
                self.linear_velocity.y = 0.0;
            }
        }

        // 6. Rotational Dynamics (Yaw, Pitch, Roll)
        let inertia = self.handling.mass * 2.5; // Simplified box inertia
        let angular_accel = total_torque / inertia;
        self.angular_velocity = self.angular_velocity + angular_accel * dt;

        // Rotational damping
        self.angular_velocity = self.angular_velocity * (1.0 - (4.0 * dt).min(0.95));

        let angle_delta = self.angular_velocity.length() * dt;
        if angle_delta > 1e-5 {
            let rot_axis = self.angular_velocity.normalize();
            let delta_q = Quat::from_axis_angle(rot_axis, angle_delta);
            self.rotation = (delta_q * self.rotation).normalize();
        }

        // 7. Engine RPM & Automatic Transmission Shifting
        self.update_transmission(dt);
    }

    /// Simulates automatic transmission gear shifts and engine RPM tachometer.
    fn update_transmission(&mut self, _dt: f32) {
        let forward_speed = self.get_speed_kmh();

        if forward_speed < -1.0 {
            self.current_gear = -1; // Reverse
            let gear_ratio = self.handling.gear_ratios[0].abs();
            self.engine_rpm = (800.0 + forward_speed.abs() * gear_ratio * self.handling.final_drive_ratio * 40.0).clamp(800.0, 7500.0);
            return;
        }

        if self.current_gear <= 0 {
            self.current_gear = 1;
        }

        let gear_idx = self.current_gear as usize;
        let gear_ratio = self.handling.gear_ratios[gear_idx];
        let theoretical_rpm = 800.0 + forward_speed.max(0.0) * gear_ratio * self.handling.final_drive_ratio * 25.0;

        self.engine_rpm = theoretical_rpm.clamp(800.0, 7500.0);

        // Automatic shift up at 6200 RPM
        if self.engine_rpm > 6200.0 && self.current_gear < self.handling.gear_count as i32 {
            self.current_gear += 1;
        }
        // Automatic shift down below 2200 RPM
        else if self.engine_rpm < 2200.0 && self.current_gear > 1 {
            self.current_gear -= 1;
        }
    }
}
