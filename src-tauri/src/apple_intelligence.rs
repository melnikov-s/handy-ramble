use std::os::raw::c_int;

// Link to the Swift functions
extern "C" {
    pub fn is_apple_intelligence_available() -> c_int;
}

// Safe wrapper functions
pub fn check_apple_intelligence_availability() -> bool {
    unsafe { is_apple_intelligence_available() == 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_availability() {
        let available = check_apple_intelligence_availability();
        println!("Apple Intelligence available: {}", available);
    }
}
