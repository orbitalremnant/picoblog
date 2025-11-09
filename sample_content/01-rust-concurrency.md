---
title: "Exploring Concurrency in Rust"
description: "A brief look at Rust's powerful and safe concurrency features, including threads and channels."
tags: ["rust", "programming", "concurrency", "tech"]
---

Rust's approach to concurrency is one of its most celebrated features. The language's ownership model and type system work together to prevent entire classes of concurrency bugs at compile time. This means you can write fearless concurrent code without worrying about data races.

### Spawning Threads

Spawning a new thread is straightforward with `std::thread::spawn`:

```rust
use std::thread;
use std::time::Duration;

fn main() {
    let handle = thread::spawn(|| {
        for i in 1..10 {
            println!("hi number {} from the spawned thread!", i);
            thread::sleep(Duration::from_millis(1));
        }
    });

    handle.join().unwrap();
}