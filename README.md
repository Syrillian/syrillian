[![Codacy Badge](https://app.codacy.com/project/badge/Grade/337033d4547044cf96a1584bf82b1ce8)](https://app.codacy.com/gh/Syrillian/syrillian/dashboard?utm_source=gh&utm_medium=referral&utm_content=&utm_campaign=Badge_grade)
[![codecov](https://codecov.io/github/Syrillian/syrillian/graph/badge.svg?token=QORLO7MO2I)](https://codecov.io/github/Syrillian/syrillian)
![GitHub commit activity](https://img.shields.io/github/commit-activity/m/Syrillian/syrillian)
[![Discord](https://img.shields.io/discord/1401869988796698696?style=flat\&label=Discord)](https://discord.gg/hZWycSwSm4)

# Syrillian Engine

Syrillian is a **magically simple 2D & 3D game engine in Rust**. Built to feel *frictionless* while staying flexible. ✨ \
The goal is straightforward: **simple high-level code, less boilerplate, more support**.

If you like Rust because it’s high-level, clean, and safe, Syrillian tries to match that.

---

## Always simple, powerful when you need it 🚀

Syrillian is intentionally opinionated about one thing: **Building your world should feel great!**

That means:

* You can "just start" without learning hundreds of concepts first.
* Everything should look like simple, high-level Rust code.
* You shouldn’t need ages to get a window, a camera, and a cube.
* You should feel great settling into Syrillian, coming from Unity or Unreal Engine.

---

## WIP (Work in Progress) 🚧

Syrillian is still actively evolving. Expect ongoing progress, occasional breaking changes, and a few “this will get nicer soon” spots as the API and features settle.

If you like the direction (especially the focus on simplicity), a ⭐ on the repo genuinely helps a lot. It makes the project more visible and keeps momentum going. Thanks for supporting it!

This project scopes far beyond what's usually done or expected in Rust to get all the simplicity and the cleanest Developer Interface possible.

A dedicated GUI editor is also underway.

---

## Quickstart

### Use Syrillian as a library

```bash
cargo add syrillian
```

### Try the repo examples

```bash
git clone https://github.com/Syrillian/syrillian.git
cd syrillian

cargo run --example my-main
```

You’ll get a window with a pre-made rendered scene, we use for testing new features.

(NixOS folks: there are development flakes in the repo.)

---

## The "Hello, Cube!" example 🧊

This is the kind of setup Syrillian aims for: clear, tiny, and easy to extend.

```rust
use std::error::Error;

use syrillian::{AppState, CubePrefab, World};
use syrillian::SyrillianApp;
use syrillian_components::CubePrefab;

#[derive(Debug, Default, SyrillianApp)]
struct YourGame;

impl AppState for YourGame {
    fn init(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        world.set_window_title("Syrillian: hello cube");

        world.new_camera();

        // Spawn a couple cubes in a row.
        for x in -2..=2 {
            world.spawn(&CubePrefab::default()).at(x * 2, 0, -10);
        }

        world.print_objects(); // scene hierarchy to console
        Ok(())
    }

    fn update(&mut self, _world: &mut World) -> Result<(), Box<dyn Error>> {
        // Per-frame game logic goes here.
        Ok(())
    }
}
```

Yes. This is the only code in the project to get a working application.\
That’s it.\
You’ve got ✨ a runtime, a multi-threaded world and renderer, and a scene you can iterate on ✨

---

## What you get out of the box ✅

Syrillian tries hard to ship useful tools so you can focus on game code:

* A **"just get it started"** workflow with a heavy focus on user-side simplicity.
* A growing set of **presets** (components / prefabs / compatibility helpers).
* **Meshes + physics**, plus visual debugging features.
* Game objects that are **builder-extensible**, so spawning + configuring stays smooth.
* Solid open-source foundations:
    * Physics integration via **Rapier**
    * Rendering abstraction via **wgpu** (Vulkan / Metal / DirectX + web via wgpu)
    * Clean windowing and native input via **winit**
    * Controller support by **gilrs**

---

## Showroom

![rabbit\_hires](https://github.com/user-attachments/assets/19a0f25d-c1f3-44e7-b9a4-93a7865087ff)

*An animated rabbit, in a multi-light scene.*

---

![](https://i.ibb.co/F9gywNk/Screenshot-2025-08-04-at-12-37-22.png)

*Picking up a physics-enabled cube with an animated shader that emits a light source.*
From this example: [syrillian_examples/examples/my-main.rs](./syrillian_examples/examples/my-main.rs)

*You can request your own visuals too*

---

## Roadmap & contributing 🤝

Planning lives in GitHub Issues (including epics):
[Issues here](https://github.com/Syrillian/syrillian/issues?q=state%3Aopen%20label%3Aepic)

Contributions are welcome, especially anything that improves the "simple, fun, frictionless" feel:

1. Open an issue (bug, idea, API rant, feature request; all welcome).
2. Discuss options and direction.
3. Send a PR if you want.

Before pushing, please run:

```bash
cargo fmt
cargo clippy --workspace --all-targets --features derive
cargo test --workspace
```

---

## History

This started as a hobby project (a personal gem), with hundreds of hours of solo development before early contributors showed up. It’s not monetized, and it’s not built for monetization.

If you possess the ability to make it **better, more stable, and easier to use** come join us.

Community: **join us on Discord** 👉 [https://discord.gg/hZWycSwSm4](https://discord.gg/hZWycSwSm4)

---

## License

Syrillian Engine is distributed under the MIT License. See [LICENSE](LICENSE) for details.

---

Syrillian Engine ❤️ Building the backbone of your next great 2D & 3D experiences.
