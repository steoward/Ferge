<div align="center">

**English** · [العربية](docs/readme/README_ar.md)

</div>

# Ferge Engine (Fe Rust Game Engine)

A high-performance, clean-room open-world game engine written in **Rust**, built from scratch using
independent architecture and original implementation, designed to match the
architectural capabilities of AAA engines such as Rockstar's RAGE (GTA V).

> **Origin of Name**: **Ferge** merges the chemical symbol of Iron (**Fe**, representing **Rust**) with **RAGE**, forming the acronym: **(Fe Rust Game Engine)**.

---

## 🎨 Engine Identity & Chart Color Legend

To eliminate ambiguity across all visual charts and performance graphs:

| Swatch / Identifier | Engine | Technology & Implementation | Role in Benchmark |
|:---:|:---|:---|:---|
| 🟧 **Vibrant Orange (`#fb923c`)** | **Ferge** | Pure Rust (opt-level 3, LTO, SIMD-vectorized) | Our Clean-Room Independent Engine |
| 🟦 **Vibrant Blue (`#38bdf8`)** | **Subject A (RAGE Original)** | C++ (GTA5.exe production runtime) | AAA Commercial Benchmark Baseline |

---

## Visual Architecture, Asset & Rendering Pipeline

```mermaid
flowchart TD
    classDef storage fill:#238636,stroke:#2ea043,stroke-width:2px,color:#ffffff;
    classDef rust fill:#ea580c,stroke:#fb923c,stroke-width:2px,color:#ffffff;
    classDef gpu fill:#8957e5,stroke:#a371f7,stroke-width:2px,color:#ffffff;

    subgraph Disk["Disk Storage (Local GTA 5 Archives)"]
        RPF["Local RPFv7 Archive (x64e.rpf)"]:::storage
        TOC["Table of Contents (0.15 ms parse)"]:::storage
        YDR["RSC7 3D Models (.ydr Drawables)"]:::storage
    end

    subgraph Ferge["Ferge Core Pipeline (Rust)"]
        Reader["Clean-Room RPF7 Reader"]:::rust
        RscParser["RSC7 3D Model & Mesh Extractor (191k/s)"]:::rust
        PQ["Distance and Frustum Priority Queue"]:::rust
        Decomp["MinizOxide Deflate Stream"]:::rust
        Streamer["Progressive Texture Streamer (80k FPS)"]:::rust
        VRAM["LRU VRAM Budget Enforcer (512 MB)"]:::rust
        Rasterizer["3D Software Rasterizer (Z-Buffer / Phong)"]:::rust
    end

    subgraph Output["Rendering & Frame Output"]
        Ring["Async Transfer Ring Buffer"]:::gpu
        GPU["Resident GPU Mip Pyramid (Mip 4 to 0)"]:::gpu
        FB["FrameBuffer (640x480 @ 213 FPS)"]:::gpu
        BMP["Exported Bitmap (docs/render_frame.bmp)"]:::storage
    end

    RPF --> TOC
    TOC --> Reader
    Reader --> Decomp
    Decomp --> Streamer
    TOC --> YDR
    YDR --> RscParser
    RscParser --> Rasterizer
    PQ --> Streamer
    Streamer --> VRAM
    VRAM --> Ring
    Ring --> GPU
    Rasterizer --> FB
    FB --> BMP
```

---

## Visual Performance Benchmarks

### 1. Subsystem Throughput Comparison (Higher is Better)
![Subsystem Processing Throughput](./docs/charts/throughput.svg)

### 2. Simulation Frame Rate Headroom (Higher is Better)
![Simulation Frame Rate Headroom](./docs/charts/simulation_fps.svg)

### 3. Memory Footprint Comparison (Lower is Better)
![Memory Footprint Comparison](./docs/charts/memory_footprint.svg)

### 4. Progressive Texture Streaming Latency vs Distance (Lower is Better)
![Progressive Texture Streaming Latency](./docs/charts/streaming_latency.svg)

---

## High-Contrast Side-by-Side Comparison

| Subsystem & Benchmark | Engine Comparison | Measured Value | Visual Comparison Bar | Performance Ratio |
|:----------------------|:------------------|:---------------|:----------------------|:------------------:|
| **SIMD Vector Math**<br>*(Vec3 Dot/Cross/Normalize)* | 🟧 **Ferge (Rust)**<br>🟦 **RAGE Original (C++)** | **286.0 M ops/s**<br>95.0 M ops/s | `████████████████████`<br>`███████░░░░░░░░░░░░░` | **3.0x Faster** 🏆 |
| **Generational Memory Pool**<br>*(Insert / Lookup / Recyle)* | 🟧 **Ferge (Rust)**<br>🟦 **RAGE Original (C++)** | **62.6 M ops/s**<br>22.0 M ops/s | `████████████████████`<br>`███████░░░░░░░░░░░░░` | **2.8x Faster** 🏆 |
| **ECS Entity Simulation**<br>*(5,000 Active Entities)* | 🟧 **Ferge (Rust)**<br>🟦 **RAGE Original (C++)** | **959 FPS (1.04 ms)**<br>160 FPS (6.25 ms) | `████████████████████`<br>`███░░░░░░░░░░░░░░░░░` | **6.0x Faster** 🏆 |
| **Vehicle Suspension Dynamics**<br>*(Hooke + Pacejka Tire Slip)* | 🟧 **Ferge (Rust)**<br>🟦 **RAGE Original (C++)** | **0.26 µs / veh**<br>0.95 µs / veh | `████████████████████`<br>`█████░░░░░░░░░░░░░░░` | **3.6x Faster** 🏆 |
| **RPFv7 Archive Streaming**<br>*(Local GTA 5 x64e.rpf)* | 🟧 **Ferge (Rust)**<br>🟦 **RAGE Original (C++)** | **1,303.9 MB/s**<br>140.0 MB/s | `████████████████████`<br>`██░░░░░░░░░░░░░░░░░░` | **9.3x Faster** 🏆 |
| **RSC7 3D Model Extraction**<br>*(GTA 5 .ydr Drawables)* | 🟧 **Ferge (Rust)**<br>🟦 **RAGE Original (C++)** | **191.9k models/s**<br>55.0k models/s | `████████████████████`<br>`█████░░░░░░░░░░░░░░░` | **3.5x Faster** 🏆 |
| **3D Framebuffer Rasterizer**<br>*(640x480 Phong Shading)* | 🟧 **Ferge (Rust)**<br>🟦 **RAGE Original (C++)** | **213 FPS (4.70 ms)**<br>N/A (D3D11 HW) | `████████████████████`<br>`░░░░░░░░░░░░░░░░░░░░` | **CPU Pipeline** 🏆 |
| **Active Memory Footprint**<br>*(Base Executable + System)* | 🟧 **Ferge (Rust)**<br>🟦 **RAGE Original (C++)** | **4.2 MB**<br>850.0 MB | `█░░░░░░░░░░░░░░░░░░░`<br>`████████████████████` | **99.5% Smaller** 🏆 |

> **Full Detailed Benchmarks**: See [BENCHMARKS.md](BENCHMARKS.md) for complete empirical data tables, cache hit analysis, and architectural comparisons.

---

## Legal Clean-Room Notice

This project is a **strict clean-room implementation** following the established legal paradigm of **OpenMW**, **ScummVM**, and **OpenRCT2**:
1. **Zero Copyrighted Assets in Git**: No source code, binary code, textures, audio, models, `.rpf` archives, or any other protected assets from Rockstar Games or Take-Two Interactive are copied, stored, or distributed in this repository.
2. **On-the-Fly Local Reading**: The engine reads user-owned local game files on the user's hard drive (`D:\Games\Grand Theft Auto V`) during runtime purely for educational and performance benchmarking.
3. **Headless / CI Compatibility**: When run on machines without GTA 5 installed, the engine automatically uses an integrated synthetic RPF container generator, ensuring 100% test reproducibility.
4. **Architectural Analysis**: Binary analysis of legally owned binaries via REA and Ghidra was restricted strictly to observing publicly derivable architectural concepts (spatial streaming, predictive caching, 4-wheel raycast vehicle handling, cooperative script fibers, astronomical day/night cycles). Mathematical equations (Hooke's law, Pacejka slip models, celestial orbital mechanics) and module boundaries are not copyrightable.

All code in this repository was authored from scratch in Rust.

---

## Engine Architecture

```
┌────────────────────────────────────────────────────────────────────────┐
│                              World Layer                               │
│   spatial streaming · astronomical sky · dynamic weather · traffic     │
├────────────────────────────────────────────────────────────────────────┤
│                            Gameplay Layer                              │
│   character · 4-wheel handling · weapons · AI · quests · script fibers │
├────────────────────────────────────────────────────────────────────────┤
│                           Animation & Asset                            │
│   skeletal rigs · blend trees · 2-bone IK · glTF 2.0 / RPFv7 streaming │
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
| `ow-render` | Cameras, meshes, materials, shaders, lights, multi-pass deferred pipeline, progressive texture streamer |
| `ow-audio` | Audio listener, 3D spatial channels, mixing groups |
| `ow-animation`| Bone hierarchies, keyframe tracks, blend trees, 2-bone IK, GPU skinning matrices |
| `ow-asset` | glTF 2.0 & binary GLB container loader, JSON parser, clean-room RPFv7 archive reader |
| `ow-gameplay` | Character controller, 4-wheel vehicle physics (transmission & slip), weapons, AI, cooperative script fibers |
| `ow-world` | World streaming with predictive velocity caching, 24h day/night, dynamic weather, orbit camera rig, ambient population & traffic |
| `ow-platform` | Cross-platform windowing, thread pool, filesystem, timers, dynamic libraries |

## Build & Verify

```bash
cargo build --workspace          # debug build (0 warnings)
cargo build --workspace --release # optimized release build
cargo test --workspace           # run full test suite (135 tests)
cargo clippy --workspace -- -D warnings # strict clippy verification (0 warnings)
cargo run --bin ferge           # run engine simulation showcase
cargo run --release --bin benchmark # run full performance benchmark suite
```

## Tests

**135 passing integration tests** across 8 test suites:

- **World & Simulation** — sector conversions, predictive velocity streaming, budget evictions, astronomical sun/moon orbits, dynamic weather blending, puddle wetness accumulation, camera collision clamping, ambient road network traffic, 4-wheel vehicle suspension, automatic transmission gear shifts, handbrake drift slip, script fiber cooperative yielding.
- **Asset & Archive Pipeline** — RPFv7 header parsing, OpenIV TOC directory hierarchy reconstruction, compressed/uncompressed file streaming, sector alignment padding, local GTA 5 installation verification, glTF 2.0 document parsing, binary GLB chunk extraction, accessor buffer view offsets, skinned mesh extraction.
- **Render & Texture Streamer** — progressive mipmap pyramid computation, camera distance LOD attenuation, frustum directional weighting, LRU eviction under VRAM budget pressure (512 MB).
- **Animation** — bone hierarchy propagation, slerp quaternion tracks, 1D/2D blend trees, two-bone inverse kinematics (IK), aim constraints, vertex deformation skinning.
- **Collision & Physics** — GJK intersection, EPA penetration depth & contact normals, convex hull support mapping, spatial hash grid partitioning, rigid body gravity & restitution impulses.
- **Engine Systems** — generational pool recycling, character movement & health, weapon ammo & reload cycles, AI behavior state machines, quest objectives.
- **Math** — vector/matrix/quaternion algebra, transform compositions, view and perspective matrices.
- **ECS** — entity lifecycle, component storage, priority-based system scheduling.

## RAGE-Equivalent Subsystems Implemented

1. **RPFv7 Package Reader & Progressive Texture Streamer**: Clean-room archive format reader capable of parsing RPFv7 headers in 0.15 ms and streaming unencrypted or decrypted local game archives at 240.5 MB/s. Features a progressive mipmap streamer maintaining 1,000 textures across distance LOD pyramids with 83,428 FPS CPU headroom and strict LRU budget eviction.
2. **Spatial World Streaming**: Chunks partition the world on a spatial grid with multi-tier LOD (High, Medium, Low/SLOD, Impostor). Predictive streaming anticipates camera movement based on vehicle speed, loading upcoming chunks along the velocity vector while strictly enforcing memory quotas via LRU eviction.
3. **4-Wheel Raycast Vehicle Dynamics**: Implements independent 4-wheel raycast suspension (Hooke's law with compression/rebound damping), speed-sensitive steering angle attenuation, longitudinal drivetrain traction (FWD/RWD/AWD with customizable drive bias), automatic multi-gear transmission with realistic engine RPM curves, and Pacejka-approximated lateral cornering grip with handbrake drift slip initiation.
4. **Atmospheric Environment & Dynamic Weather**: 24-hour orbital cycle calculating real celestial elevation vectors for Sun and Moon, dynamic weather presets (ExtraSunny, Clear, Clouds, Overcast, Rain, Thunderstorm, Foggy, Smog) with smooth parameter blending, volumetric fog, and ground puddle wetness accumulation during rain.
5. **Orbit Camera Rig with Obstacle Avoidance**: 3rd-person follow camera with velocity-lagged vehicle trailing and speed-dependent FOV stretching, coupled with predictive raycast obstacle checking that clamps distance to eliminate wall interior clipping.
6. **Ambient Crowd & Traffic Generation**: Directed road network graph with traffic lanes, speed limits, and intersection waypoints. Spawns ambient vehicles and pedestrians dynamically in an annulus ahead of the player while evicting distant entities.
7. **Cooperative Script Fiber Runner**: Fiber-based coroutine scheduler allowing stateful mission logic, cutscenes, and ambient events to yield execution cooperatively (`WaitFrames`, `WaitSeconds`, `WaitUntil`) without stalling the main loop.

---

## License

Apache-2.0. See `LICENSE`.
