//mod other_mod;

fn library_function(name: &str) {
    println!("Called with: {}", name);
}

fn consumer_one() {
    library_function("Consumer one");
}

fn consumer_two() {
    library_function();

}

fn main() {
    consumer_one();
    consumer_two();
}
