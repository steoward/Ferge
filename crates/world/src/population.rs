use std::collections::HashMap;
use ow_math::Vec3;

/// A node along a road network representing an intersection or curve waypoint.
#[derive(Debug, Clone)]
pub struct RoadNode {
    pub id: u32,
    pub position: Vec3,
    pub is_intersection: bool,
    pub connected_lane_ids: Vec<u32>,
}

/// A directed traffic lane connecting two road nodes.
#[derive(Debug, Clone)]
pub struct RoadLane {
    pub id: u32,
    pub start_node_id: u32,
    pub end_node_id: u32,
    pub speed_limit_kmh: f32,
    pub width: f32,
}

/// Road graph network defining vehicle path topology.
#[derive(Debug, Clone, Default)]
pub struct RoadNetwork {
    pub nodes: HashMap<u32, RoadNode>,
    pub lanes: HashMap<u32, RoadLane>,
}

impl RoadNetwork {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, id: u32, position: Vec3, is_intersection: bool) {
        self.nodes.insert(
            id,
            RoadNode {
                id,
                position,
                is_intersection,
                connected_lane_ids: Vec::new(),
            },
        );
    }

    pub fn add_lane(&mut self, id: u32, start_node: u32, end_node: u32, speed_limit_kmh: f32) {
        self.lanes.insert(
            id,
            RoadLane {
                id,
                start_node_id: start_node,
                end_node_id: end_node,
                speed_limit_kmh,
                width: 3.5,
            },
        );
        if let Some(node) = self.nodes.get_mut(&start_node) {
            node.connected_lane_ids.push(id);
        }
    }

    /// Finds the closest road lane to a given point in space.
    pub fn find_nearest_lane(&self, point: Vec3) -> Option<&RoadLane> {
        let mut nearest: Option<&RoadLane> = None;
        let mut min_dist_sq = f32::MAX;

        for lane in self.lanes.values() {
            if let (Some(start), Some(end)) = (self.nodes.get(&lane.start_node_id), self.nodes.get(&lane.end_node_id)) {
                let mid = (start.position + end.position) * 0.5;
                let d_sq = (mid - point).length_squared();
                if d_sq < min_dist_sq {
                    min_dist_sq = d_sq;
                    nearest = Some(lane);
                }
            }
        }

        nearest
    }
}

/// An ambient AI vehicle driving in traffic.
#[derive(Debug, Clone)]
pub struct AmbientVehicle {
    pub id: u64,
    pub position: Vec3,
    pub velocity: Vec3,
    pub speed: f32,
    pub target_speed: f32,
    pub current_lane_id: u32,
    pub target_node_id: u32,
    pub model_id: u32,
}

impl AmbientVehicle {
    pub fn update(&mut self, dt: f32, road_network: &RoadNetwork) {
        if let Some(target_node) = road_network.nodes.get(&self.target_node_id) {
            let to_target = target_node.position - self.position;
            let dist = to_target.length();

            if dist < 4.0 {
                // Reached target waypoint, pick next connected lane
                if let Some(current_lane) = road_network.lanes.get(&self.current_lane_id) {
                    if let Some(end_node) = road_network.nodes.get(&current_lane.end_node_id) {
                        if !end_node.connected_lane_ids.is_empty() {
                            // Cycle through next available lane
                            let next_lane_id = end_node.connected_lane_ids[0];
                            if let Some(next_lane) = road_network.lanes.get(&next_lane_id) {
                                self.current_lane_id = next_lane_id;
                                self.target_node_id = next_lane.end_node_id;
                            }
                        }
                    }
                }
            } else {
                let forward = to_target.normalize();
                // Smooth acceleration toward target speed
                self.speed += (self.target_speed - self.speed) * (4.0 * dt).min(1.0);
                self.velocity = forward * self.speed;
                self.position = self.position + self.velocity * dt;
            }
        }
    }
}

/// An ambient pedestrian walking along sidewalks or public zones.
#[derive(Debug, Clone)]
pub struct AmbientPedestrian {
    pub id: u64,
    pub position: Vec3,
    pub velocity: Vec3,
    pub wander_target: Vec3,
    pub walk_speed: f32,
    pub is_panicked: bool,
}

impl AmbientPedestrian {
    pub fn update(&mut self, dt: f32) {
        let to_target = self.wander_target - self.position;
        let dist = to_target.length();

        if dist < 1.5 {
            // Pick new nearby wander offset
            let angle = (self.id as f32 * 1.618).sin() * std::f32::consts::PI;
            let radius = 10.0;
            self.wander_target = self.position + Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius);
        } else {
            let forward = to_target.normalize();
            let effective_speed = if self.is_panicked { self.walk_speed * 3.0 } else { self.walk_speed };
            self.velocity = forward * effective_speed;
            self.position = self.position + self.velocity * dt;
        }
    }

    pub fn trigger_panic(&mut self, threat_source: Vec3) {
        self.is_panicked = true;
        let away_dir = (self.position - threat_source).normalize();
        self.wander_target = self.position + away_dir * 30.0;
    }
}

/// Configuration governing ambient spawning densities and radiuses.
#[derive(Debug, Clone)]
pub struct PopulationConfig {
    pub spawn_inner_radius: f32,
    pub spawn_outer_radius: f32,
    pub despawn_radius: f32,
    pub max_vehicles: usize,
    pub max_pedestrians: usize,
}

impl Default for PopulationConfig {
    fn default() -> Self {
        Self {
            spawn_inner_radius: 35.0,  // Never pop in right in front of camera
            spawn_outer_radius: 110.0, // Active population annulus
            despawn_radius: 140.0,     // Eviction threshold
            max_vehicles: 20,
            max_pedestrians: 30,
        }
    }
}

/// Dynamic ambient crowd and traffic generator for open-world simulation.
pub struct PopulationManager {
    pub config: PopulationConfig,
    pub road_network: RoadNetwork,
    pub vehicles: Vec<AmbientVehicle>,
    pub pedestrians: Vec<AmbientPedestrian>,
    next_entity_id: u64,
    spawn_timer: f32,
}

impl PopulationManager {
    pub fn new(config: PopulationConfig, road_network: RoadNetwork) -> Self {
        Self {
            config,
            road_network,
            vehicles: Vec::new(),
            pedestrians: Vec::new(),
            next_entity_id: 1000,
            spawn_timer: 0.0,
        }
    }

    /// Updates existing ambient entities and spawns/despawns according to proximity.
    pub fn update(&mut self, dt: f32, player_pos: Vec3, player_forward: Vec3) {
        // 1. Update active entities
        for vehicle in &mut self.vehicles {
            vehicle.update(dt, &self.road_network);
        }
        for ped in &mut self.pedestrians {
            ped.update(dt);
        }

        // 2. Despawn entities exceeding despawn distance
        let despawn_dist_sq = self.config.despawn_radius * self.config.despawn_radius;
        self.vehicles.retain(|v| (v.position - player_pos).length_squared() <= despawn_dist_sq);
        self.pedestrians.retain(|p| (p.position - player_pos).length_squared() <= despawn_dist_sq);

        // 3. Periodic spawning in forward annulus
        self.spawn_timer += dt;
        if self.spawn_timer >= 1.0 {
            self.spawn_timer = 0.0;
            self.attempt_spawn(player_pos, player_forward);
        }
    }

    /// Spawns ambient vehicles and pedestrians in the camera's forward field of interest.
    fn attempt_spawn(&mut self, player_pos: Vec3, player_forward: Vec3) {
        // Spawn vehicle if below capacity
        if self.vehicles.len() < self.config.max_vehicles {
            // Pick a candidate position ahead in the road network
            let spawn_dist = (self.config.spawn_inner_radius + self.config.spawn_outer_radius) * 0.5;
            let candidate_pos = player_pos + player_forward * spawn_dist;

            if let Some(lane) = self.road_network.find_nearest_lane(candidate_pos) {
                if let Some(start_node) = self.road_network.nodes.get(&lane.start_node_id) {
                    let vehicle_id = self.next_entity_id;
                    self.next_entity_id += 1;

                    self.vehicles.push(AmbientVehicle {
                        id: vehicle_id,
                        position: start_node.position,
                        velocity: Vec3::ZERO,
                        speed: lane.speed_limit_kmh / 3.6,
                        target_speed: lane.speed_limit_kmh / 3.6,
                        current_lane_id: lane.id,
                        target_node_id: lane.end_node_id,
                        model_id: (vehicle_id % 5) as u32,
                    });
                }
            }
        }

        // Spawn pedestrian if below capacity
        if self.pedestrians.len() < self.config.max_pedestrians {
            let ped_id = self.next_entity_id;
            self.next_entity_id += 1;

            let angle = (ped_id as f32 * 0.77).sin() * 0.8; // Bias toward player front
            let lateral_offset = Vec3::new(-player_forward.z, 0.0, player_forward.x) * angle * 25.0;
            let spawn_pos = player_pos + player_forward * self.config.spawn_inner_radius + lateral_offset;

            self.pedestrians.push(AmbientPedestrian {
                id: ped_id,
                position: spawn_pos,
                velocity: Vec3::ZERO,
                wander_target: spawn_pos + player_forward * 15.0,
                walk_speed: 1.4,
                is_panicked: false,
            });
        }
    }

    pub fn vehicle_count(&self) -> usize {
        self.vehicles.len()
    }

    pub fn pedestrian_count(&self) -> usize {
        self.pedestrians.len()
    }
}
