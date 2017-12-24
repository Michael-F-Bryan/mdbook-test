extern crate skeptic;

fn main(){
    let deps: Vec<&str> = vec![$DEPS];
    skeptic::generate_doc_tests(&deps);
}