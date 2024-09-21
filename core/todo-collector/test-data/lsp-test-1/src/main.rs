pub struct PubStruct {
    thing: String,
}

struct PrivStruct {
    thing: String,
}

#[derive(Debug)]
pub enum PubEnum {
    Variant1,
    Variant2,
}

struct TupleStruct();

enum PrivEnum {
    Variant1,
}

fn main() {
    println!("Hello, world!");
}

fn func_2() {

    // make this muliti line
}

fn func_3() -> String {
    String::new()
}

fn func_4() -> () {}

/// Documentation
fn func_5() -> String {
    // TODO: contructor
    String::new()
}

fn func_6() {
    // TODO: mesage
}

fn func_7() {
    //
}

fn func_8() -> String {
    println!("");
}

fn two_prints() {
    println!("foo");
    println!("bar");
}

async fn one_print() {
    println!("foobar");
}

async fn priv_async_fn() {}

pub async fn pub_async_fn() {}

pub fn pub_sync_fn() {}
