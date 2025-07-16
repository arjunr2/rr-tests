// Import handler implementations
pub fn host_double_fn(param: i32) -> i32 { param * 2 }
pub fn host_complex_fn(p1: i32, p2: i64) -> (i32, i64, f32) { ( (p1 as f32).sqrt() as i32, (p1 * p1) as i64 * p2, 8.66 ) }