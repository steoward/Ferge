use ow_math::Vec3;

/// 24-hour astronomical clock driving the daylight cycle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeOfDay {
    pub hours: f32,
    pub minutes: f32,
    pub seconds: f32,
    /// Rate at which in-game time passes relative to real time (e.g. 30.0 = 48 minutes per 24h day).
    pub time_scale: f32,
}

impl Default for TimeOfDay {
    fn default() -> Self {
        Self {
            hours: 12.0, // Start at noon
            minutes: 0.0,
            seconds: 0.0,
            time_scale: 30.0,
        }
    }
}

impl TimeOfDay {
    pub fn new(hours: f32, minutes: f32, seconds: f32) -> Self {
        Self {
            hours,
            minutes,
            seconds,
            time_scale: 30.0,
        }
    }

    /// Advances the in-game clock by real delta time in seconds.
    pub fn advance(&mut self, dt: f32) {
        self.seconds += dt * self.time_scale;
        while self.seconds >= 60.0 {
            self.seconds -= 60.0;
            self.minutes += 1.0;
        }
        while self.minutes >= 60.0 {
            self.minutes -= 60.0;
            self.hours += 1.0;
        }
        while self.hours >= 24.0 {
            self.hours -= 24.0;
        }
    }

    pub fn total_seconds(&self) -> f32 {
        self.hours * 3600.0 + self.minutes * 60.0 + self.seconds
    }

    /// Normalized progress through the day in the range [0.0, 1.0).
    /// 0.0 = 00:00 (Midnight), 0.25 = 06:00 (Sunrise), 0.50 = 12:00 (Noon), 0.75 = 18:00 (Sunset).
    pub fn normalized(&self) -> f32 {
        (self.total_seconds() / 86400.0).fract()
    }

    pub fn is_daytime(&self) -> bool {
        let norm = self.normalized();
        (0.23..=0.77).contains(&norm)
    }

    pub fn is_nighttime(&self) -> bool {
        !self.is_daytime()
    }

    /// Computes normalized directional vectors for the sun and moon.
    pub fn compute_celestial_vectors(&self) -> (Vec3, Vec3) {
        let norm = self.normalized();
        // Offset so 0.25 (6:00) is sunrise, 0.5 (12:00) is zenith overhead
        let angle = (norm - 0.25) * std::f32::consts::TAU;

        let inclination = 0.35f32; // Axial tilt
        let sun_dir = Vec3::new(
            -angle.cos(),
            angle.sin(),
            angle.cos() * inclination,
        ).normalize();

        // Moon orbits directly opposite to the sun
        let moon_dir = (-sun_dir).normalize();

        (sun_dir, moon_dir)
    }
}

/// Weather types reflecting open-world atmospheric states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeatherPreset {
    ExtraSunny,
    Clear,
    Clouds,
    Overcast,
    Rain,
    Thunderstorm,
    Foggy,
    Smog,
}

/// Dynamic atmospheric lighting and post-process parameters calculated per frame.
#[derive(Debug, Clone)]
pub struct AtmosphereState {
    pub sun_direction: Vec3,
    pub moon_direction: Vec3,
    pub sun_color: Vec3,
    pub sun_intensity: f32,
    pub ambient_color: Vec3,
    pub sky_zenith_color: Vec3,
    pub sky_horizon_color: Vec3,
    pub fog_color: Vec3,
    pub fog_density: f32,
    pub fog_start: f32,
    pub fog_end: f32,
    pub rain_intensity: f32,
    pub wind_direction: Vec3,
    pub wind_speed: f32,
    pub surface_wetness: f32,
}

/// Profile values for a specific weather condition.
#[derive(Debug, Clone)]
struct WeatherProfile {
    sun_intensity_mult: f32,
    sun_color_tint: Vec3,
    ambient_color_tint: Vec3,
    fog_density: f32,
    fog_start: f32,
    fog_end: f32,
    rain_rate: f32,
    wind_speed: f32,
}

impl WeatherPreset {
    fn profile(&self) -> WeatherProfile {
        match self {
            WeatherPreset::ExtraSunny => WeatherProfile {
                sun_intensity_mult: 1.3,
                sun_color_tint: Vec3::new(1.0, 0.98, 0.92),
                ambient_color_tint: Vec3::new(0.35, 0.40, 0.50),
                fog_density: 0.001,
                fog_start: 300.0,
                fog_end: 2500.0,
                rain_rate: 0.0,
                wind_speed: 2.0,
            },
            WeatherPreset::Clear => WeatherProfile {
                sun_intensity_mult: 1.0,
                sun_color_tint: Vec3::new(1.0, 0.95, 0.88),
                ambient_color_tint: Vec3::new(0.30, 0.35, 0.45),
                fog_density: 0.002,
                fog_start: 200.0,
                fog_end: 2000.0,
                rain_rate: 0.0,
                wind_speed: 5.0,
            },
            WeatherPreset::Clouds => WeatherProfile {
                sun_intensity_mult: 0.85,
                sun_color_tint: Vec3::new(0.95, 0.95, 0.95),
                ambient_color_tint: Vec3::new(0.35, 0.38, 0.42),
                fog_density: 0.004,
                fog_start: 150.0,
                fog_end: 1600.0,
                rain_rate: 0.0,
                wind_speed: 8.0,
            },
            WeatherPreset::Overcast => WeatherProfile {
                sun_intensity_mult: 0.50,
                sun_color_tint: Vec3::new(0.80, 0.82, 0.85),
                ambient_color_tint: Vec3::new(0.28, 0.30, 0.32),
                fog_density: 0.008,
                fog_start: 100.0,
                fog_end: 1200.0,
                rain_rate: 0.0,
                wind_speed: 12.0,
            },
            WeatherPreset::Rain => WeatherProfile {
                sun_intensity_mult: 0.35,
                sun_color_tint: Vec3::new(0.70, 0.72, 0.78),
                ambient_color_tint: Vec3::new(0.22, 0.24, 0.28),
                fog_density: 0.015,
                fog_start: 50.0,
                fog_end: 800.0,
                rain_rate: 0.8,
                wind_speed: 18.0,
            },
            WeatherPreset::Thunderstorm => WeatherProfile {
                sun_intensity_mult: 0.20,
                sun_color_tint: Vec3::new(0.55, 0.60, 0.70),
                ambient_color_tint: Vec3::new(0.18, 0.20, 0.25),
                fog_density: 0.025,
                fog_start: 30.0,
                fog_end: 500.0,
                rain_rate: 1.0,
                wind_speed: 30.0,
            },
            WeatherPreset::Foggy => WeatherProfile {
                sun_intensity_mult: 0.40,
                sun_color_tint: Vec3::new(0.85, 0.85, 0.85),
                ambient_color_tint: Vec3::new(0.40, 0.40, 0.40),
                fog_density: 0.040,
                fog_start: 10.0,
                fog_end: 250.0,
                rain_rate: 0.0,
                wind_speed: 1.5,
            },
            WeatherPreset::Smog => WeatherProfile {
                sun_intensity_mult: 0.60,
                sun_color_tint: Vec3::new(0.90, 0.80, 0.65),
                ambient_color_tint: Vec3::new(0.35, 0.32, 0.28),
                fog_density: 0.012,
                fog_start: 80.0,
                fog_end: 900.0,
                rain_rate: 0.0,
                wind_speed: 3.0,
            },
        }
    }
}

/// Complete environment management system simulating day/night and dynamic weather.
pub struct EnvironmentSystem {
    pub time: TimeOfDay,
    pub current_weather: WeatherPreset,
    pub target_weather: WeatherPreset,
    pub transition_duration: f32,
    pub transition_progress: f32,
    pub surface_wetness: f32,
    pub wind_direction: Vec3,
}

impl Default for EnvironmentSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl EnvironmentSystem {
    pub fn new() -> Self {
        Self {
            time: TimeOfDay::default(),
            current_weather: WeatherPreset::Clear,
            target_weather: WeatherPreset::Clear,
            transition_duration: 15.0,
            transition_progress: 1.0,
            surface_wetness: 0.0,
            wind_direction: Vec3::new(1.0, 0.0, 0.2).normalize(),
        }
    }

    /// Triggers a smooth weather transition to a new preset over the given duration in seconds.
    pub fn set_weather(&mut self, target: WeatherPreset, duration_secs: f32) {
        if self.target_weather == target {
            return;
        }
        self.current_weather = self.sample_interpolated_preset();
        self.target_weather = target;
        self.transition_duration = duration_secs.max(0.1);
        self.transition_progress = 0.0;
    }

    fn sample_interpolated_preset(&self) -> WeatherPreset {
        if self.transition_progress >= 0.5 {
            self.target_weather
        } else {
            self.current_weather
        }
    }

    /// Advances simulation time, weather interpolation, and surface puddle physics.
    pub fn update(&mut self, dt: f32) {
        self.time.advance(dt);

        if self.transition_progress < 1.0 {
            self.transition_progress += dt / self.transition_duration;
            if self.transition_progress >= 1.0 {
                self.transition_progress = 1.0;
                self.current_weather = self.target_weather;
            }
        }

        // Wetness accumulates in rain and dries under direct sun
        let profile_a = self.current_weather.profile();
        let profile_b = self.target_weather.profile();
        let t = self.transition_progress.clamp(0.0, 1.0);
        let rain_rate = profile_a.rain_rate * (1.0 - t) + profile_b.rain_rate * t;

        if rain_rate > 0.05 {
            self.surface_wetness = (self.surface_wetness + rain_rate * 0.1 * dt).min(1.0);
        } else {
            // Evaporation drying rate
            let drying_rate = if self.time.is_daytime() { 0.03 } else { 0.01 };
            self.surface_wetness = (self.surface_wetness - drying_rate * dt).max(0.0);
        }
    }

    /// Computes the complete evaluated atmosphere state for the current frame.
    pub fn evaluate_atmosphere(&self) -> AtmosphereState {
        let (sun_dir, moon_dir) = self.time.compute_celestial_vectors();

        let profile_a = self.current_weather.profile();
        let profile_b = self.target_weather.profile();
        let t = self.transition_progress.clamp(0.0, 1.0);

        // Interpolate weather factors
        let sun_mult = profile_a.sun_intensity_mult * (1.0 - t) + profile_b.sun_intensity_mult * t;
        let sun_tint = profile_a.sun_color_tint * (1.0 - t) + profile_b.sun_color_tint * t;
        let ambient_tint = profile_a.ambient_color_tint * (1.0 - t) + profile_b.ambient_color_tint * t;
        let fog_density = profile_a.fog_density * (1.0 - t) + profile_b.fog_density * t;
        let fog_start = profile_a.fog_start * (1.0 - t) + profile_b.fog_start * t;
        let fog_end = profile_a.fog_end * (1.0 - t) + profile_b.fog_end * t;
        let rain_intensity = profile_a.rain_rate * (1.0 - t) + profile_b.rain_rate * t;
        let wind_speed = profile_a.wind_speed * (1.0 - t) + profile_b.wind_speed * t;

        // Base sun elevation factor: 0.0 below horizon, 1.0 at zenith
        let sun_elevation = sun_dir.y.max(0.0);
        let sun_intensity = sun_elevation * 1.5 * sun_mult;

        // Golden hour / sunset reddening calculation
        let sunset_factor = (1.0 - (sun_elevation - 0.1).abs() * 5.0).clamp(0.0, 1.0);
        let sunset_tint = Vec3::new(1.0, 0.45, 0.2) * sunset_factor;
        let final_sun_color = (sun_tint + sunset_tint).normalize();

        // Night ambient vs daytime ambient
        let day_ambient = ambient_tint * (0.3 + sun_elevation * 0.7);
        let night_ambient = Vec3::new(0.04, 0.06, 0.12);
        let ambient_color = day_ambient * sun_elevation + night_ambient * (1.0 - sun_elevation);

        // Sky colors
        let sky_zenith = if sun_elevation > 0.05 {
            Vec3::new(0.18, 0.42, 0.85) * (0.4 + sun_elevation * 0.6)
        } else {
            Vec3::new(0.01, 0.02, 0.05)
        };

        let sky_horizon = if sun_elevation > 0.05 {
            Vec3::new(0.65, 0.75, 0.90) * (0.3 + sun_elevation * 0.7) + sunset_tint * 0.6
        } else {
            Vec3::new(0.02, 0.03, 0.07)
        };

        AtmosphereState {
            sun_direction: sun_dir,
            moon_direction: moon_dir,
            sun_color: final_sun_color,
            sun_intensity,
            ambient_color,
            sky_zenith_color: sky_zenith,
            sky_horizon_color: sky_horizon,
            fog_color: ambient_color * 1.2,
            fog_density,
            fog_start,
            fog_end,
            rain_intensity,
            wind_direction: self.wind_direction,
            wind_speed,
            surface_wetness: self.surface_wetness,
        }
    }
}
