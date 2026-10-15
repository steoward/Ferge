// Gameplay systems: character, vehicle, weapon, ai, quest, world, handling, script
pub mod handling;
pub mod script;

pub mod character {
    use ow_ecs::component::{Health, Name, Position, Velocity};
    use ow_math::Vec3;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum CharacterState {
        Idle,
        Walking,
        Running,
        Sprinting,
        Crouching,
        Jumping,
        Falling,
        Swimming,
        Climbing,
        Ragdoll,
        Dead,
    }

    #[derive(Debug, Clone)]
    pub struct CharacterController {
        pub entity_id: u64,
        pub state: CharacterState,
        pub move_direction: Vec3,
        pub look_direction: Vec3,
        pub speed: f32,
        pub max_speed: f32,
        pub acceleration: f32,
        pub deceleration: f32,
        pub jump_force: f32,
        pub gravity: f32,
        pub is_grounded: bool,
        pub can_move: bool,
        pub health: Health,
        pub name: Name,
        pub position: Position,
        pub velocity: Velocity,
    }

    impl CharacterController {
        pub fn new(entity_id: u64, name: impl Into<String>) -> Self {
            Self {
                entity_id,
                state: CharacterState::Idle,
                move_direction: Vec3::ZERO,
                look_direction: Vec3::FORWARD,
                speed: 0.0,
                max_speed: 5.0,
                acceleration: 10.0,
                deceleration: 15.0,
                jump_force: 8.0,
                gravity: -9.81,
                is_grounded: true,
                can_move: true,
                health: Health::new(100.0),
                name: Name(name.into()),
                position: Position(Vec3::ZERO),
                velocity: Velocity(Vec3::ZERO),
            }
        }

        pub fn update(&mut self, dt: f32, input_move: Vec3, input_jump: bool) {
            if !self.can_move || !self.health.is_alive() {
                return;
            }

            self.move_direction = input_move.normalize();

            let target_speed = if input_move.length() > 0.0 {
                self.max_speed
            } else {
                0.0
            };

            let accel = if target_speed > self.speed {
                self.acceleration
            } else {
                self.deceleration
            };

            self.speed += (target_speed - self.speed) * accel * dt;

            let horizontal_velocity = self.move_direction * self.speed;
            let vertical_velocity = Vec3::new(0.0, self.velocity.0.y, 0.0);

            self.velocity = Velocity(horizontal_velocity + vertical_velocity);

            let new_pos = Position(self.position.0 + self.velocity.0 * dt);
            self.position = new_pos;

            if self.is_grounded && input_jump {
                self.velocity = Velocity(self.velocity.0 + Vec3::new(0.0, self.jump_force, 0.0));
                self.is_grounded = false;
                self.state = CharacterState::Jumping;
            }

            if !self.is_grounded {
                let mut v = self.velocity.0;
                v.y += self.gravity * dt;
                self.velocity = Velocity(v);
            }

            if self.is_grounded && self.speed > 0.1 {
                self.state = if self.speed > self.max_speed * 0.7 {
                    CharacterState::Running
                } else {
                    CharacterState::Walking
                };
            } else if self.is_grounded && self.speed <= 0.1 {
                self.state = CharacterState::Idle;
            }
        }

        pub fn take_damage(&mut self, damage: f32) {
            self.health.take_damage(damage);
            if !self.health.is_alive() {
                self.state = CharacterState::Dead;
                self.can_move = false;
            }
        }

        pub fn heal(&mut self, amount: f32) {
            self.health.heal(amount);
            if self.health.is_alive() && self.state == CharacterState::Dead {
                self.state = CharacterState::Idle;
                self.can_move = true;
            }
        }

        pub fn is_alive(&self) -> bool {
            self.health.is_alive()
        }
    }
}

pub mod vehicle {
    use ow_math::Vec3;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum VehicleState {
        Parked,
        Idle,
        Driving,
        Reversed,
        Damaged,
        Destroyed,
    }

    #[derive(Debug, Clone)]
    pub struct VehicleController {
        pub entity_id: u64,
        pub state: VehicleState,
        pub speed: f32,
        pub max_speed: f32,
        pub acceleration: f32,
        pub brake_force: f32,
        pub linear_damping: f32,
        pub steering: f32,
        pub max_steering: f32,
        pub position: Vec3,
        pub rotation: ow_math::Quat,
        pub velocity: Vec3,
        pub angular_velocity: f32,
        pub mass: f32,
        pub engine_force: f32,
        pub brake_torque: f32,
        pub wheel_friction: f32,
        pub suspension_stiffness: f32,
        pub suspension_damping: f32,
        pub suspension_rest_length: f32,
        pub is_engine_running: bool,
        pub is_braking: bool,
        pub steer_input: f32,
        pub throttle_input: f32,
        pub brake_input: f32,
    }

    impl VehicleController {
        pub fn new(entity_id: u64) -> Self {
            Self {
                entity_id,
                state: VehicleState::Parked,
                speed: 0.0,
                max_speed: 60.0,
                acceleration: 15.0,
                brake_force: 30.0,
                linear_damping: 0.4,
                steering: 0.0,
                max_steering: 0.5,
                position: Vec3::ZERO,
                rotation: ow_math::Quat::IDENTITY,
                velocity: Vec3::ZERO,
                angular_velocity: 0.0,
                mass: 1500.0,
                engine_force: 2000.0,
                brake_torque: 3000.0,
                wheel_friction: 0.8,
                suspension_stiffness: 5.0,
                suspension_damping: 0.5,
                suspension_rest_length: 0.5,
                is_engine_running: false,
                is_braking: false,
                steer_input: 0.0,
                throttle_input: 0.0,
                brake_input: 0.0,
            }
        }

        pub fn update(&mut self, dt: f32) {
            if self.state == VehicleState::Destroyed {
                return;
            }

            let forward = self.rotation.rotate_vector(Vec3::FORWARD);

            if self.is_engine_running && self.throttle_input > 0.0 {
                let force = forward * self.engine_force * self.throttle_input / self.mass;
                self.velocity = self.velocity + force * dt;
            }

            if self.brake_input > 0.0 || self.is_braking {
                let brake_decel = self.brake_force * dt / self.mass * 10.0;
                let current_speed = self.velocity.length();
                if current_speed > brake_decel {
                    self.velocity = self.velocity * (1.0 - brake_decel / current_speed);
                } else {
                    self.velocity = Vec3::ZERO;
                }
            }

            let rolling_resistance = 1.0 - (self.linear_damping * dt).min(0.99);
            self.velocity = self.velocity * rolling_resistance;

            self.speed = self.velocity.dot(&forward);

            if self.speed.abs() > self.max_speed {
                self.velocity = self.velocity * (self.max_speed / self.speed.abs());
                self.speed = self.velocity.dot(&forward);
            }

            let steer_amount = self.steer_input * self.max_steering * (self.speed / self.max_speed);
            let delta_rotation = ow_math::Quat::from_axis_angle(Vec3::UP, steer_amount * dt);
            self.rotation = (delta_rotation * self.rotation).normalize();

            self.position = self.position + forward * self.speed * dt;

            if self.speed > 0.1 {
                self.state = VehicleState::Driving;
            } else if self.speed < -0.1 {
                self.state = VehicleState::Reversed;
            } else {
                self.state = if self.is_engine_running {
                    VehicleState::Idle
                } else {
                    VehicleState::Parked
                };
            }
        }

        pub fn start_engine(&mut self) {
            self.is_engine_running = true;
            if self.state == VehicleState::Parked {
                self.state = VehicleState::Idle;
            }
        }

        pub fn stop_engine(&mut self) {
            self.is_engine_running = false;
            self.throttle_input = 0.0;
            self.brake_input = 0.0;
            self.state = VehicleState::Parked;
        }

        pub fn accelerate(&mut self, amount: f32) {
            self.throttle_input = amount.clamp(0.0, 1.0);
        }

        pub fn brake(&mut self, amount: f32) {
            self.brake_input = amount.clamp(0.0, 1.0);
        }

        pub fn steer(&mut self, amount: f32) {
            self.steer_input = amount.clamp(-1.0, 1.0);
        }

        pub fn get_speed_kmh(&self) -> f32 {
            self.speed * 3.6
        }
    }
}

pub mod weapon {
    

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum WeaponType {
        Melee,
        Pistol,
        SMG,
        Rifle,
        Shotgun,
        Sniper,
        Explosive,
        Thrown,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum WeaponState {
        Idle,
        Reloading,
        Firing,
        Empty,
    }

    #[derive(Debug, Clone)]
    pub struct Weapon {
        pub name: String,
        pub weapon_type: WeaponType,
        pub state: WeaponState,
        pub damage: f32,
        pub fire_rate: f32,
        pub range: f32,
        pub accuracy: f32,
        pub magazine_size: u32,
        pub current_ammo: u32,
        pub reserve_ammo: u32,
        pub reload_time: f32,
        pub current_reload_time: f32,
        pub is_auto: bool,
        pub recoil: f32,
        pub spread: f32,
    }

    impl Weapon {
        pub fn new(name: impl Into<String>, weapon_type: WeaponType) -> Self {
            Self {
                name: name.into(),
                weapon_type,
                state: WeaponState::Idle,
                damage: 25.0,
                fire_rate: 10.0,
                range: 100.0,
                accuracy: 0.95,
                magazine_size: 30,
                current_ammo: 30,
                reserve_ammo: 120,
                reload_time: 2.0,
                current_reload_time: 0.0,
                is_auto: false,
                recoil: 0.02,
                spread: 0.01,
            }
        }

        pub fn fire(&mut self) -> bool {
            if self.state != WeaponState::Idle {
                return false;
            }
            if self.current_ammo == 0 {
                self.state = WeaponState::Empty;
                return false;
            }
            self.current_ammo -= 1;
            self.state = WeaponState::Firing;
            if self.current_ammo == 0 {
                self.state = WeaponState::Empty;
            } else {
                self.state = WeaponState::Idle;
            }
            true
        }

        pub fn reload(&mut self) -> bool {
            if self.state != WeaponState::Idle && self.state != WeaponState::Empty {
                return false;
            }
            if self.reserve_ammo == 0 || self.current_ammo == self.magazine_size {
                return false;
            }
            self.state = WeaponState::Reloading;
            self.current_reload_time = self.reload_time;
            true
        }

        pub fn update(&mut self, dt: f32) {
            if self.state == WeaponState::Reloading {
                self.current_reload_time -= dt;
                if self.current_reload_time <= 0.0 {
                    let needed = self.magazine_size - self.current_ammo;
                    let available = needed.min(self.reserve_ammo);
                    self.current_ammo += available;
                    self.reserve_ammo -= available;
                    self.state = WeaponState::Idle;
                }
            }
        }

        pub fn can_fire(&self) -> bool {
            (self.state == WeaponState::Idle || self.state == WeaponState::Firing) && self.current_ammo > 0
        }
    }
}

pub mod ai {
    use ow_math::Vec3;
    use ow_ecs::component::{Health, Position, Velocity};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum AIState {
        Idle,
        Patrol,
        Chase,
        Attack,
        Flee,
        Dead,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum AIBehavior {
        Passive,
        Aggressive,
        Defensive,
        Coward,
    }

    #[derive(Debug, Clone)]
    pub struct AIController {
        pub entity_id: u64,
        pub state: AIState,
        pub behavior: AIBehavior,
        pub position: Position,
        pub velocity: Velocity,
        pub health: Health,
        pub target: Option<u64>,
        pub patrol_points: Vec<Vec3>,
        pub current_patrol_index: usize,
        pub detection_range: f32,
        pub attack_range: f32,
        pub move_speed: f32,
        pub turn_speed: f32,
        pub is_alerted: bool,
    }

    impl AIController {
        pub fn new(entity_id: u64) -> Self {
            Self {
                entity_id,
                state: AIState::Idle,
                behavior: AIBehavior::Passive,
                position: Position(Vec3::ZERO),
                velocity: Velocity(Vec3::ZERO),
                health: Health::new(100.0),
                target: None,
                patrol_points: Vec::new(),
                current_patrol_index: 0,
                detection_range: 20.0,
                attack_range: 2.0,
                move_speed: 3.0,
                turn_speed: 5.0,
                is_alerted: false,
            }
        }

        pub fn update(&mut self, dt: f32, player_pos: Vec3) {
            if !self.health.is_alive() {
                self.state = AIState::Dead;
                return;
            }

            let dist_to_player = (self.position.0 - player_pos).length();

            match self.behavior {
                AIBehavior::Passive => {
                    if self.state == AIState::Idle && !self.patrol_points.is_empty() {
                        self.state = AIState::Patrol;
                    }
                }
                AIBehavior::Aggressive => {
                    if dist_to_player < self.detection_range {
                        self.is_alerted = true;
                        if dist_to_player > self.attack_range {
                            self.state = AIState::Chase;
                        } else {
                            self.state = AIState::Attack;
                        }
                    }
                }
                AIBehavior::Defensive => {
                    if dist_to_player < self.detection_range {
                        self.is_alerted = true;
                        self.state = AIState::Attack;
                    }
                }
                AIBehavior::Coward => {
                    if dist_to_player < self.detection_range {
                        self.is_alerted = true;
                        self.state = AIState::Flee;
                    }
                }
            }

            match self.state {
                AIState::Patrol => self.update_patrol(dt),
                AIState::Chase => self.update_chase(dt, player_pos),
                AIState::Flee => self.update_flee(dt, player_pos),
                AIState::Attack => {}
                AIState::Idle | AIState::Dead => {}
            }
        }

        fn update_patrol(&mut self, dt: f32) {
            if self.patrol_points.is_empty() {
                self.state = AIState::Idle;
                return;
            }

            let target = self.patrol_points[self.current_patrol_index];
            let dir = (target - self.position.0).normalize();
            let new_pos = self.position.0 + dir * self.move_speed * dt;
            self.position = Position(new_pos);

            if (target - self.position.0).length() < 0.5 {
                self.current_patrol_index = (self.current_patrol_index + 1) % self.patrol_points.len();
            }
        }

        fn update_chase(&mut self, dt: f32, target_pos: Vec3) {
            let dir = (target_pos - self.position.0).normalize();
            let new_pos = self.position.0 + dir * self.move_speed * dt;
            self.position = Position(new_pos);
        }

        fn update_flee(&mut self, dt: f32, threat_pos: Vec3) {
            let dir = (self.position.0 - threat_pos).normalize();
            let new_pos = self.position.0 + dir * self.move_speed * dt;
            self.position = Position(new_pos);
        }
    }
}

pub mod quest {
    use std::collections::HashMap;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum QuestState {
        Locked,
        Available,
        Active,
        Completed,
        Failed,
        TurnedIn,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum ObjectiveType {
        Kill,
        Collect,
        Reach,
        Interact,
        Escort,
        Defend,
        Stealth,
        Race,
    }

    #[derive(Debug, Clone)]
    pub struct Objective {
        pub id: u32,
        pub objective_type: ObjectiveType,
        pub description: String,
        pub target_count: u32,
        pub current_count: u32,
        pub is_completed: bool,
        pub is_optional: bool,
        pub target_entity: Option<u64>,
        pub target_position: Option<ow_math::Vec3>,
    }

    impl Objective {
        pub fn new(id: u32, objective_type: ObjectiveType, description: impl Into<String>, target_count: u32) -> Self {
            Self {
                id,
                objective_type,
                description: description.into(),
                target_count,
                current_count: 0,
                is_completed: false,
                is_optional: false,
                target_entity: None,
                target_position: None,
            }
        }

        pub fn update_progress(&mut self, amount: u32) {
            self.current_count = (self.current_count + amount).min(self.target_count);
            if self.current_count >= self.target_count {
                self.is_completed = true;
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct Quest {
        pub id: u32,
        pub name: String,
        pub description: String,
        pub state: QuestState,
        pub objectives: Vec<Objective>,
        pub rewards: QuestRewards,
        pub prerequisites: Vec<u32>,
        pub time_limit: Option<f32>,
        pub current_time: f32,
    }

    #[derive(Debug, Clone, Default)]
    pub struct QuestRewards {
        pub experience: u32,
        pub money: u32,
        pub items: Vec<String>,
        pub reputation: HashMap<String, i32>,
    }

    impl Quest {
        pub fn new(id: u32, name: impl Into<String>, description: impl Into<String>) -> Self {
            Self {
                id,
                name: name.into(),
                description: description.into(),
                state: QuestState::Available,
                objectives: Vec::new(),
                rewards: QuestRewards::default(),
                prerequisites: Vec::new(),
                time_limit: None,
                current_time: 0.0,
            }
        }

        pub fn start(&mut self) -> bool {
            if self.state != QuestState::Available {
                return false;
            }
            self.state = QuestState::Active;
            true
        }

        pub fn update(&mut self, dt: f32) {
            if self.state != QuestState::Active {
                return;
            }

            if let Some(time_limit) = self.time_limit {
                self.current_time += dt;
                if self.current_time >= time_limit {
                    self.state = QuestState::Failed;
                }
            }

            let all_completed = self.objectives.iter()
                .filter(|o| !o.is_optional)
                .all(|o| o.is_completed);

            if all_completed {
                self.state = QuestState::Completed;
            }
        }

        pub fn is_completed(&self) -> bool {
            self.state == QuestState::Completed || self.state == QuestState::TurnedIn
        }
    }

    #[derive(Debug, Clone, Default)]
    pub struct QuestManager {
        quests: HashMap<u32, Quest>,
        active_quests: Vec<u32>,
        completed_quests: Vec<u32>,
    }

    impl QuestManager {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn add_quest(&mut self, quest: Quest) {
            self.quests.insert(quest.id, quest);
        }

        pub fn get_quest(&self, id: u32) -> Option<&Quest> {
            self.quests.get(&id)
        }

        pub fn get_quest_mut(&mut self, id: u32) -> Option<&mut Quest> {
            self.quests.get_mut(&id)
        }

        pub fn start_quest(&mut self, id: u32) -> bool {
            if let Some(quest) = self.quests.get_mut(&id) {
                if quest.start() {
                    self.active_quests.push(id);
                    return true;
                }
            }
            false
        }

        pub fn update(&mut self, dt: f32) {
            for &id in &self.active_quests.clone() {
                if let Some(quest) = self.quests.get_mut(&id) {
                    quest.update(dt);
                    if quest.state == QuestState::Completed {
                        self.active_quests.retain(|&q| q != id);
                        self.completed_quests.push(id);
                    }
                }
            }
        }

        pub fn active_count(&self) -> usize {
            self.active_quests.len()
        }

        pub fn completed_count(&self) -> usize {
            self.completed_quests.len()
        }
    }
}

pub mod world {
    use ow_math::Vec3;
    

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct TimeOfDay {
        pub hours: f32,
        pub minutes: f32,
        pub seconds: f32,
    }

    impl TimeOfDay {
        pub fn new(hours: f32, minutes: f32, seconds: f32) -> Self {
            Self { hours, minutes, seconds }
        }

        pub fn total_seconds(&self) -> f32 {
            self.hours * 3600.0 + self.minutes * 60.0 + self.seconds
        }

        pub fn normalized(&self) -> f32 {
            (self.total_seconds() / 86400.0).fract()
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum WeatherType {
        Clear,
        Cloudy,
        Overcast,
        Rain,
        Thunderstorm,
        Snow,
        Fog,
        Sandstorm,
    }

    #[derive(Debug, Clone)]
    pub struct Weather {
        pub current: WeatherType,
        pub target: WeatherType,
        pub transition_time: f32,
        pub current_transition: f32,
        pub intensity: f32,
        pub wind_direction: Vec3,
        pub wind_speed: f32,
        pub temperature: f32,
        pub humidity: f32,
    }

    impl Default for Weather {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Weather {
        pub fn new() -> Self {
            Self {
                current: WeatherType::Clear,
                target: WeatherType::Clear,
                transition_time: 10.0,
                current_transition: 0.0,
                intensity: 0.0,
                wind_direction: Vec3::FORWARD,
                wind_speed: 0.0,
                temperature: 20.0,
                humidity: 0.5,
            }
        }

        pub fn update(&mut self, dt: f32) {
            if self.current != self.target {
                self.current_transition += dt;
                let t = (self.current_transition / self.transition_time).min(1.0);
                self.intensity = t;
                if t >= 1.0 {
                    self.current = self.target;
                    self.current_transition = 0.0;
                }
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct SpawnPoint {
        pub position: Vec3,
        pub rotation: ow_math::Quat,
        pub name: String,
        pub is_active: bool,
    }

    impl SpawnPoint {
        pub fn new(position: Vec3, name: impl Into<String>) -> Self {
            Self {
                position,
                rotation: ow_math::Quat::IDENTITY,
                name: name.into(),
                is_active: true,
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct WorldConfig {
        pub name: String,
        pub time_scale: f32,
        pub gravity: Vec3,
        pub spawn_points: Vec<SpawnPoint>,
    }

    impl WorldConfig {
        pub fn new(name: impl Into<String>) -> Self {
            Self {
                name: name.into(),
                time_scale: 1.0,
                gravity: Vec3::new(0.0, -9.81, 0.0),
                spawn_points: Vec::new(),
            }
        }
    }
}
