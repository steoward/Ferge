# FerrumRAGE Engine

A high-performance, clean-room open-world game engine written in **Rust**, built from scratch using
independent architecture and original implementation, designed to match the
architectural capabilities of AAA engines such as Rockstar's RAGE (GTA V).

## Legal Notice

This project is a **clean-room implementation**. No source code, binary code,
textures, models, or any other protected assets from any commercial game were
copied, decompiled, or translated into this codebase.

Analysis of legally-owned binaries (`GTA5.exe`) via binary analysis tools (REA and Ghidra)
was restricted strictly to observing **publicly derivable architectural concepts** —
such as subsystem decomposition (spatial world streaming, predictive cache, 4-wheel raycast
vehicle handling, cooperative script fibers, astronomical day/night cycles, and generational ECS).
Concepts, mathematical equations (Hooke's law, Pacejka slip models, celestial orbital mechanics),
and module boundaries are **not copyrightable**; the implementation here is original,
was authored independently in Rust, and shares zero code, tokens, or asset data with
the analyzed products.

All code in this repository was authored from scratch for this project.

## Architecture

```
┌────────────────────────────────────────────────────────────────────────┐
│                              World Layer                               │
│   spatial streaming · astronomical sky · dynamic weather · traffic     │
├────────────────────────────────────────────────────────────────────────┤
│                            Gameplay Layer                              │
│   character · 4-wheel handling · weapons · AI · quests · script fibers │
├────────────────────────────────────────────────────────────────────────┤
│                           Animation & Asset                            │
│   skeletal rigs · blend trees · 2-bone IK · glTF 2.0 / GLB pipeline    │
├────────────────────────────────────────────────────────────────────────┤
│                          Core Engine Layer                             │
│   memory · input · physics (GJK/EPA) · render passes · audio · ECS     │
├────────────────────────────────────────────────────────────────────────┤
│                           Platform Layer                               │
│   window · filesystem · thread pool · time · dynamic library loading   │
└────────────────────────────────────────────────────────────────────────┘
```

## Crates

| Crate | Responsibility |
|---|---|
| `ow-math` | `Vec2/3/4`, `Mat3/4`, `Quat`, `Transform` (SIMD-ready POD) |
| `ow-core` | Engine bootstrap, event bus, scene graph, resources |
| `ow-memory` | Generational pool allocator, bump allocator, free-list, stack |
| `ow-input` | Keyboard, mouse, gamepad with deadzone mapping & actions |
| `ow-ecs` | Data-oriented ECS with stage/priority scheduling |
| `ow-physics` | Rigid bodies, colliders, GJK/EPA narrowphase, SpatialHash broadphase, raycasts |
| `ow-render` | Cameras, meshes, materials, shaders, lights, multi-pass deferred pipeline |
| `ow-audio` | Audio listener, 3D spatial channels, mixing groups |
| `ow-animation`| Bone hierarchies, keyframe tracks, blend trees, 2-bone IK, GPU skinning matrices |
| `ow-asset` | glTF 2.0 & binary GLB container loader, JSON parser |
| `ow-gameplay` | Character controller, 4-wheel vehicle physics (transmission & slip), weapons, AI, cooperative script fibers |
| `ow-world` | World streaming with predictive velocity caching, 24h day/night, dynamic weather, orbit camera rig, ambient population & traffic |
| `ow-platform` | Cross-platform windowing, thread pool, filesystem, timers, dynamic libraries |

## Build & Verify

```bash
cargo build --workspace          # debug build (0 warnings)
cargo build --workspace --release # optimized release build
cargo test --workspace           # run full test suite (130 tests)
cargo run --bin ferrum-rage       # run engine simulation showcase
cargo run --release --bin benchmark # run full performance benchmark suite
```

## Tests

**130 passing integration tests** across 7 test suites:

- **World & Simulation** — sector conversions, predictive velocity streaming, budget evictions, astronomical sun/moon orbits, dynamic weather blending, puddle wetness accumulation, camera collision clamping, ambient road network traffic, 4-wheel vehicle suspension, automatic transmission gear shifts, handbrake drift slip, script fiber cooperative yielding.
- **Animation** — bone hierarchy propagation, slerp quaternion tracks, 1D/2D blend trees, two-bone inverse kinematics (IK), aim constraints, vertex deformation skinning.
- **Asset Pipeline** — glTF 2.0 document parsing, binary GLB chunk extraction, accessor buffer view offsets, skinned mesh extraction.
- **Collision & Physics** — GJK intersection, EPA penetration depth & contact normals, convex hull support mapping, spatial hash grid partitioning, rigid body gravity & restitution impulses.
- **Engine Systems** — generational pool recycling, character movement & health, weapon ammo & reload cycles, AI behavior state machines, quest objectives.
- **Math** — vector/matrix/quaternion algebra, transform compositions, view and perspective matrices.
- **ECS** — entity lifecycle, component storage, priority-based system scheduling.

## RAGE-Equivalent Subsystems Implemented

1. **Spatial World Streaming**: Chunks partition the world on a spatial grid with multi-tier LOD (High, Medium, Low/SLOD, Impostor). Predictive streaming anticipates camera movement based on vehicle speed, loading upcoming chunks along the velocity vector while strictly enforcing memory quotas via LRU eviction.
2. **4-Wheel Raycast Vehicle Dynamics**: Implements independent 4-wheel raycast suspension (Hooke's law with compression/rebound damping), speed-sensitive steering angle attenuation, longitudinal drivetrain traction (FWD/RWD/AWD with customizable drive bias), automatic multi-gear transmission with realistic engine RPM curves, and Pacejka-approximated lateral cornering grip with handbrake drift slip initiation.
3. **Atmospheric Environment & Dynamic Weather**: 24-hour orbital cycle calculating real celestial elevation vectors for Sun and Moon, dynamic weather presets (ExtraSunny, Clear, Clouds, Overcast, Rain, Thunderstorm, Foggy, Smog) with smooth parameter blending, volumetric fog, and ground puddle wetness accumulation during rain.
4. **Orbit Camera Rig with Obstacle Avoidance**: 3rd-person follow camera with velocity-lagged vehicle trailing and speed-dependent FOV stretching, coupled with predictive raycast obstacle checking that clamps distance to eliminate wall interior clipping.
5. **Ambient Crowd & Traffic Generation**: Directed road network graph with traffic lanes, speed limits, and intersection waypoints. Spawns ambient vehicles and pedestrians dynamically in an annulus ahead of the player while evicting distant entities.
6. **Cooperative Script Fiber Runner**: Fiber-based coroutine scheduler allowing stateful mission logic, cutscenes, and ambient events to yield execution cooperatively (`WaitFrames`, `WaitSeconds`, `WaitUntil`) without stalling the main loop.

## License

Apache-2.0. See `LICENSE`.
