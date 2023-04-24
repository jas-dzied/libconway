struct Config {
    width: u32,
    height: u32
}

@group(0)
@binding(0)
var<uniform> config: Config;

@group(0)
@binding(1)
var<storage, read> input_buffer: array<u32>;

@group(0)
@binding(2)
var<storage, write> output_buffer: array<u32>;

@group(0)
@binding(3)
var texture: texture_storage_2d<rgba32float, write>;

fn from_xy(x: u32, y: u32) -> u32 {
    return y * config.width + x;
}

fn modulus(a: i32, b: i32) -> i32 {
    return ((a % b) + b) % b;
}

// fn get_at(position: vec3<u32>, x_mod: i32, y_mod: i32) -> u32 {
//     let x = u32(modulus(i32(position.x) + x_mod, i32(config.width)));
//     let y = u32(modulus(i32(position.y) + y_mod, i32(config.height)));
//     let index = from_xy(x, y);
//     let value = input_buffer[index];
//     return value;
// }

fn get_at(position: vec3<u32>, x_mod: i32, y_mod: i32) -> u32 {
    let x = i32(position.x) - x_mod;
    let y = i32(position.y) - y_mod;
    if x == -1 {
        return u32(0);
    } else if x == i32(config.width) {
        return u32(0);
    } else if y == -1 {
        return u32(0);
    } else if y == i32(config.height) {
        return u32(0);
    } else {
        let index = from_xy(u32(x), u32(y));
        return input_buffer[index];
    }
}

@compute
@workgroup_size(5, 5)
fn main(@builtin(global_invocation_id) position: vec3<u32>) {

    let index = from_xy(position.x, position.y);
    let old_value = input_buffer[index];

    var total: u32;
    total += get_at(position, -1, -1);
    total += get_at(position, 0, -1);
    total += get_at(position, 1, -1);
    total += get_at(position, -1, 0);
    total += get_at(position, 1, 0);
    total += get_at(position, -1, 1);
    total += get_at(position, 0, 1);
    total += get_at(position, 1, 1);

    var alive_rules = array(0, 0, 1, 1, 0, 0, 0, 0, 0);
    var dead_rules = array(0, 0, 0, 1, 0, 0, 0, 0, 0);

    var is_alive: u32;
    if old_value == u32(1) {
        is_alive = u32(alive_rules[total]);
    } else {
        is_alive = u32(dead_rules[total]);    
    }

    output_buffer[index] = is_alive;
    textureStore(
        texture,
        vec2<u32>(position.x, position.y),
        vec4<f32>(
          f32(is_alive),
          f32(is_alive),
          f32(is_alive),
          1.0
        )
    ); 
}
