# Nevy: Streamlined Networking for Bevy Game Engine 🌐

## Introduction 📢

Due to the extremely complex nature of networking in games, and the vast number of use cases Nevy wants to accomodate, development is currently ongoing and Nevy is not useable. The repo has been published so feedback can be provided and different APIs can be experimented and prototyped with in an iterative fashion.

If you are a game developer please check back in a few weeks and hopefully this project will be more applicable to you.
If you are a engine or networking developer and wish to contribute or have feedback, feel free to reach out in the bevy discord or my DMs

Goals 🎯
Nevy is driven by a set of core objectives, designed to deliver a robust and developer-friendly networking solution:

High-Level Abstraction 🚀: Nevy aims to simplify game networking, providing a user-friendly interface that masks underlying complexities. This approach offers ease and clarity in implementing networked interactions, without sacrificing depth and control.

Performance-Oriented Design 💨: With a keen focus on efficiency, Nevy is engineered for high performance. Embracing data-oriented techniques and novel ECS-optimized compression, it ensures streamlined data handling and minimal bandwidth usage. 

ECS-Friendly Architecture 🧩: Nevy is meticulously designed to align with Bevy's Entity Component System (ECS), ensuring seamless integration. This harmonious compatibility provides Bevy developers with an intuitive and fluid experience, enhancing both development efficiency and gameplay quality.

Abstract Compositions with Bundles 📦: Emulating Bevy's approach, Nevy allows for defining abstract compositions through Bundles. This feature facilitates streamlined and organized handling of networked entities and their behaviors.

Event-Driven Interactions for Entities 🎭: Nevy supports event-based mechanisms, enabling developers to efficiently manage entity-specific interactions, such as animations and particle effects, through networked events.

Customizable Message Definitions 📝: The library offers flexibility in defining custom messages, allowing for tailored communication protocols that fit the unique requirements of each game.

Advanced RPC Calls with Promise-Like Syntax 🌐: Nevy introduces one-way and two-way Remote Procedure Calls (RPCs), including a JavaScript-like .then() syntax for handling the responses of two-way RPCs. This feature provides a powerful and intuitive way to manage network call responses.

By adhering to these goals, Nevy aspires to revolutionize networking in the Bevy game engine, making it more accessible, efficient, and compatible with modern game development practices. 🌟