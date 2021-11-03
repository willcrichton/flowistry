pub fn basic_constant_and_variable() {
    let mut x = 1;
    let y = x + 2;
    let z = y;
}

pub fn basic_unused() {
    let x = 1;
    let y = 1 + 2;
    let z = x + y;
}

pub fn basic_update() {
    let mut x = 1;
    x += 1;
    let y = x;
}

fn condition() {
    let x = 1;
    let y = 2;
    let z = if true {
        x
    } else {
        y
    };
    let w = z;
}

fn pointer_write() {
    let mut x = 1;
    let y = &mut x;
    *y += 2;
}

fn function_params(x: i32) {
    let y = x + 1;
    let z = y;
}

struct Point(i32, i32);
fn struct_param(p: &mut Point) {
  p.0 += 1;
}
