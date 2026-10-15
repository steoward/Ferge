# Architecture & Feature Comparison

**Subject A** — `GTA5.exe` (GTA V / Enhanced, legally owned build)
**Subject B** — Open World Engine (clean-room Rust reimplementation in this repo)

This document compares the two on **architecture**, **subsystem decomposition**
and **operational behaviour**. Only non-copyrightable architectural facts are
used. No code, strings, identifiers, or asset data from Subject A were copied
into Subject B.

---

## 1. Measured Physical Facts

All Subject A figures were extracted from the shipped binary via headless
Ghidra 12.1.4 and REA binary analysis tools (loader-level facts: sections, import table, function bodies, PE resource hashes).
All Subject B figures were measured from the Rust source code.

| Metric | Subject A (GTA5.exe) | Subject B (Rust engine) |
|---|---|---|
| Binary / source size | 54,386,152 bytes (54.4 MB) | 13,366 lines of Rust |
| Source files | binary (monolithic image) | 79 `.rs` files |
| Top-level units | 1 monolithic executable | 13 crates in workspace |
| Discovered functions | **91,045** | 680+ public functions |
| Memory blocks / segments | **12** (including dual `.text` regions) | heap + stack (Rust memory safety) |
| External library deps | **33 DLLs, 445 symbols** | 1 (`parking_lot`) + std |
| Test coverage | proprietary internal QA | **130 passing integration tests** |
| Memory safety | raw C++ pointers, manual heaps | safe Rust (generational handles, zero unsafe API) |

---

## 2. Layered Architecture Comparison

Both engines decompose into equivalent logical layers, but diverge fundamentally in structural modularity:

| Layer | Subject A (RAGE) | Subject B (Open World Engine) |
|---|---|---|
| Platform / OS | direct Win32 (33 DLLs: D3D9, DINPUT8, XINPUT, DSOUND) | `ow-platform` (window, fs, thread pool, timer, dynamic lib) |
| Core Memory | custom sub-allocators over a 19.8 MB `.data` segment | `ow-memory` (generational pool, bump, free-list, stack) |
| Math | SIMD intrinsics inside `.text` | `ow-math` (Vec2/3/4, Mat3/4, Quat, Transform, Copy POD) |
| Input | DINPUT8 + XINPUT1_4 + IMM32 | `ow-input` (keyboard, mouse, gamepad with deadzones & actions) |
| Physics | in-house RAGE solver + GJK/EPA | `ow-physics` (GJK/EPA narrowphase, SpatialHash broadphase, impulse solver) |
| World Streaming | sector streaming from `.rpf` archives | `ow-world::streaming` (spatial grid, predictive velocity cache, budget LRU) |
| Atmosphere & Sky | time-of-day orbital cycle & weather presets | `ow-world::environment` (24h celestial orbits, weather blending, surface wetness) |
| Camera System | 3rd person & vehicle follow with obstacle raycast | `ow-world::camera` (OrbitCameraRig with predictive wall collision clamping) |
| Crowd & Traffic | ambient population generator along road nodes | `ow-world::population` (annulus spawner, road network, vehicle & ped AI) |
| Vehicle Dynamics | RAGE `handling.dat` 4-wheel raycast simulation | `ow-gameplay::handling` (4-wheel raycast suspension, transmission, Pacejka slip) |
| Skeletal Animation| multi-layer blend trees, 2-bone IK, rigs | `ow-animation` (bone hierarchies, 1D/2D blend trees, 2-bone IK, GPU skinning) |
| Asset Pipeline | proprietary `#rsc` / `.ydr` containers | `ow-asset` (glTF 2.0 / GLB skinned mesh reader with custom JSON parser) |
| Scripting VM | cooperative fibers running `.ysc` bytecode | `ow-gameplay::script` (cooperative ScriptFiber scheduler with wait conditions) |

---

## 3. Subsystem-by-Subsystem Deep Dive

### 3.1 World Streaming & Spatial Partitioning
- **Subject A**: World map is divided into streaming boxes stored within encrypted `.rpf` archives. The streaming manager monitors camera position and vehicle speed to prioritize asset streaming.
- **Subject B (`ow-world::streaming`)**: Divides the world into an infinite XZ spatial grid (`SectorId`). Implements a multi-tier LOD hierarchy (`High`, `Medium`, `Low`, `Impostor`). Implements **predictive velocity caching** ($p_{\text{focus}} = p + v \cdot \Delta t_{\text{anticipate}}$) to pre-load upcoming sectors ahead of high-speed vehicles. Strictly enforces memory budgets (`StreamingBudget`) with LRU eviction.

### 3.2 4-Wheel Vehicle Dynamics & Handling
- **Subject A**: Uses the famous `handling.dat` specification featuring ~40 parameters per vehicle (mass, drive bias, transmission gear ratios, suspension stiffness, damping, roll bars, and traction curves).
- **Subject B (`ow-gameplay::handling`)**: Complete clean-room 4-wheel raycast vehicle physics:
  - Independent raycast suspension per wheel using Hooke's Law spring force and differential bump/rebound shock damping.
  - Speed-sensitive front wheel steering lock reduction.
  - Multi-gear automatic transmission with engine RPM tachometer curves and auto upshift/downshift.
  - Drivetrain torque delivery (FWD, RWD, AWD) with customizable front/rear bias.
  - Pacejka-approximated lateral cornering grip, load transfer, and handbrake rear-wheel lock initiating drift slip.

### 3.3 Atmosphere & Dynamic Weather
- **Subject A**: Atmospheric lighting system driven by a 24-hour game clock with orbital sun and moon positioning, and weather states (ExtraSunny, Clear, Clouds, Overcast, Rain, Thunder, Foggy, Smog).
- **Subject B (`ow-world::environment`)**: 24-hour astronomical clock with orbital mathematics computing directional vectors and zenith angles for Sun and Moon. Dynamic weather presets with continuous parameter interpolation (sun intensity, ambient tint, volumetric fog start/end, wind speed). Features dynamic ground surface puddle wetness accumulation during rainfall and evaporation under sunlight.

### 3.4 Camera Collision Clamping
- **Subject A**: Camera follows character or vehicle and casts rays into the world to clamp distance, preventing the camera from clipping into walls or terrain.
- **Subject B (`ow-world::camera`)**: `OrbitCameraRig` supporting character and vehicle tracking modes with exponential spring-damper easing. Integrates ray-AABB slab intersection to predictively clamp follow distance against obstacle colliders, ensuring the camera remains strictly outside geometric boundaries.

### 3.5 Ambient Crowd & Traffic Simulation
- **Subject A**: Popcycle engine generates ambient pedestrian and vehicle traffic along road graph nodes within an active ring around the player.
- **Subject B (`ow-world::population`)**: `PopulationManager` with `RoadNetwork` graph. Synthesizes vehicles along directed traffic lanes and pedestrians along sidewalk zones in an annulus ahead of the player (35m - 110m), smoothly despawning entities when they exit the active boundary.

### 3.6 Cooperative Script Fibers (Script VM)
- **Subject A**: RAGE executes gameplay missions and ambient scripts in cooperative fibers (`.ysc`), allowing scripts to sleep for frames or seconds without halting the game loop.
- **Subject B (`ow-gameplay::script`)**: `ScriptManager` provides lightweight cooperative `ScriptFiber` coroutines that yield execution with `WaitFrames`, `WaitSeconds`, or `WaitUntil` predicate closures, sharing global state flags (`ScriptContext`) across missions and ambient triggers.

---

## 4. Verification & Test Metrics

| Test Suite | File | Tests Passed | Status |
|---|---|---|---|
| World & Simulation | `tests/world_and_simulation_tests.rs` | 12 | PASS |
| Animation System | `tests/animation_tests.rs` | 33 | PASS |
| Physics & Collision | `tests/collision_tests.rs` | 20 | PASS |
| GLTF Asset Pipeline | `tests/gltf_tests.rs` | 17 | PASS |
| Engine Core Systems | `tests/engine_systems_tests.rs` | 23 | PASS |
| Math & Algebra | `tests/engine_tests.rs` | 19 | PASS |
| ECS Subsystem | `tests/systems_tests.rs` | 6 | PASS |
| **Total Workspace** | **7 integration test suites** | **130** | **100% PASS** |

- **Compiler Status**: Debug and Release builds compile with **0 warnings**.
- **Linter Status**: `cargo clippy --workspace` passes with **0 warnings**.
- **Runtime Execution**: `cargo run --bin open-world-engine` executes cleanly with all subsystems active.

---

## 5. Clean-Room Legal Compliance

1. **Independent Implementation**: All code was written from scratch in Rust.
2. **No Proprietary Assets**: Zero textures, models, sound files, strings, or decompiled C++ tokens exist in the codebase.
3. **Architectural Observation Only**: Binary inspection via REA and Ghidra was strictly confined to non-copyrightable physical and architectural facts (section sizes, DLL dependencies, entry point structure, and observable real-time simulation paradigms).
4. **Clean Boundaries**: The engine compiles to a standalone, modern multi-crate Rust workspace with no link dependencies on proprietary game libraries.
