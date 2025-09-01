const std = @import("std");

export fn add_two_numbers(a: i32, b: i32) i32 {
    return a + b;
}

test "add_two_numbers test" {
    const result = add_two_numbers(1, 2);
    try std.testing.expect(result == 3);
}
