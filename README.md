# Nevy: Streamlined Networking for Bevy Game Engine 🌐

## Introduction 📢

**Nevy** is an advanced networking framework designed for the Bevy game engine. It combines Bevy's simplicity with robust networking capabilities, making it an ideal choice for developers building interconnected gaming experiences.

## Key Features 🌟

### Bundle-Based Architecture 📦

- **Elegant Grouping** 🧩: Utilizes `NetBundle` to encapsulate networked entities, providing a clean and organized approach to manage game states and behaviors.

### Flexible Synchronization ⚙️

- **Controlled Sync** 🔄: Offers precise control over the synchronization of each entity's state, ensuring efficient and consistent multiplayer experiences.

### Custom Entity Messages 💌

- **Tailored Communication** 💬: Supports custom entity messages, enabling a flexible framework for specific network communication needs.

### Optimized Performance 🚀

- **Efficient Updates** ⏩: Nevy enhances performance by batching updates per archetype, significantly reducing overhead and improving overall game responsiveness.

## Getting Started 🚀

Here's a simple example to get you started with Nevy:

```rust
#[derive(NetBundle)]
#[init(init_player)]
pub struct PlayerBundle {
    #[sync(always)]
    name: Name,
    #[server]
    role: Role
}

fn test(mut cmds: Commands) {
    cmds.spawn_networked::<PlayerBundle>(ServerPlayerBundle {
        name: Name::new("Blah"),
        role: Role::User
    });
}
```