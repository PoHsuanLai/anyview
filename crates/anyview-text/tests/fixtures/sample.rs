//! A small module.

/// Adds two numbers.
pub fn add(a: u32, b: u32) -> u32 {
    a + b // sum
}

struct Point {
    x: f64,
    y: f64,
}

fn main() {
    let name = "world";
    println!("hello {name}, {}", add(1, 2));
}
