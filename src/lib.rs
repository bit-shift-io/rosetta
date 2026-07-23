#![recursion_limit = "256"]

pub mod bridge;
pub mod config;
pub mod gif;
pub mod persistence;
pub mod services;

#[cfg(test)]
mod tests {
    #[test]
    fn test_placeholder() {
        assert!(true);
    }
}
