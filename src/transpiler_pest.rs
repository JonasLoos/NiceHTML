use wasm_bindgen::prelude::*;
use pest::Parser;
use pest_derive::Parser;
use web_sys::{window, console, Element};
use std::panic;


#[derive(Parser)]
#[grammar = "src/grammar.pest"]
struct MyParser;

#[wasm_bindgen]
pub fn transpile(input: &str) -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    let pairs = MyParser::parse(Rule::file, &input).map_err(|err| err.to_string())?;
    let output = process_pairs(pairs);
    console::log_1(&output.into());
    Ok(())
}

fn process_pairs(pairs: pest::iterators::Pairs<Rule>) -> String {
    let mut output = String::new();
    for pair in pairs {
        match pair.as_rule() {
            Rule::function_definition => {
                // process function_definition
                output.push_str(&process_pairs(pair.into_inner()));
            }
            Rule::function_call => {
                // process function_call
                output.push_str(&process_pairs(pair.into_inner()));
            }
            Rule::tag => {
                // process tag
                output.push_str(&process_pairs(pair.into_inner()));
            }
            // ...
            _ => {}
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test_transpile() {
        let input = "your_input";
        let output = transpile(input).unwrap();
        assert_eq!(output, "expected_output");
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn main_js() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    Ok(())
}
