use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{window, console, Element};
use std::collections::HashMap;
use regex::Regex;


#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn run_compiler(content: String) -> Result<(), JsValue> {
    console::log_1(&"Hello using web-sys".into());

    let window = window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");

    // Get the body
    let body = document.body().expect("document should have a body");

    // Create an element
    let pre = document.create_element("pre")?;
    let code = document.create_element("code")?;
    code.set_inner_html(&content);
    pre.append_child(&code)?;
    body.append_child(&pre)?;

    Ok(())
}

fn is_valid_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => (),
        _ => return false,
    }

    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[wasm_bindgen]
pub fn render(script: &str) -> Result<(), JsValue> {
    let window = window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    let body = document.body().unwrap();
    let lines: Vec<&str> = script.lines().collect();
    let mut parent_stack: Vec<(Element, &str)> = vec![(body.clone().into(), "")];  // stack of elements, which can be variables/functions (then the name is set)
    let mut variables = HashMap::new();  // mapping from variable/function name to tuple of Element and places for each argument to be inserted (list of lists)
    // let mut fun_def_args = HashMap::new();
    let mut current_indent = 0;
    
    for line in lines {
        // ignore empty lines and comments
        if line.trim().is_empty() || line.trim().starts_with("#") {
            continue;
        }
        // Parse line for element, id, class, etc.
        // For simplicity, let's assume it's just the tag name
        
        let indent = line.chars().take_while(|&c| c == ' ').count() / 4;
        if indent > current_indent + 1 {
            return Err("Indentation error: too much indentation at once".into());
        }
        while indent <= current_indent {
            let (previous_top, var_name) = parent_stack.pop().unwrap();
            current_indent -= 1;
            if var_name.is_empty() {
                // append normal elements to their parent. Don't do this for variables/functions
                parent_stack.last().unwrap().0.append_child(&previous_top)?;
            }
        }
        let raw_line = line.trim();

        // variable / function
        if raw_line.ends_with("=") {
            let definition = raw_line[0..raw_line.len()-1].trim();
            let name: &str;
            let mut args: Vec<&str> = vec![];
            // variable
            if definition.split_whitespace().count() == 1 && is_valid_identifier(definition) {
                name = definition;
            } else if definition.contains("(") && definition.contains(")") {
                name = definition.split("(").next().unwrap();
                if !is_valid_identifier(name) {
                    return Err("Syntax error: not a valid variable or function definition".into());
                }
                if !definition.split("(").nth(1).unwrap().split(")").nth(1).unwrap().trim().is_empty() {
                    return Err("Syntax error: not a valid variable or function definition".into());
                }
                args = definition.split("(").nth(1).unwrap().split(")").next().unwrap().split(",").collect();
            } else {
                return Err("Syntax error: not a valid variable or function definition".into());
            }
            let mut arg_places: Vec<(&str,Vec<Element>)> = vec![];
            for arg_name in args {
                arg_places.push((arg_name, vec![]));
            }
            let var_elem = document.create_element("div")?;
            variables.insert(name, (var_elem.clone(), arg_places));
            parent_stack.push((var_elem, name));
            current_indent += 1;

        // string
        } else if raw_line.starts_with("\"") && raw_line.ends_with("\"") {
            // create a span element and put the string inside
            let element = document.create_element("span")?;
            element.set_inner_html(&raw_line[1..raw_line.len()-1]);
            parent_stack.push((element, ""));
            current_indent += 1;

        // normal tag
        } else if is_valid_identifier(raw_line.split_whitespace().next().unwrap()) {
            let tag_name = raw_line.split_whitespace().next().unwrap();
            
            // Create element
            let element = document.create_element(tag_name)?;

            // Use a regular expression to split the attributes string into pairs of attribute_name and attribute_value
            let re = Regex::new(r#"(?P<name>\w+)="(?P<value>[^"]+)""#).unwrap();
            
            // Set each attribute on the element
            for cap in re.captures_iter(&raw_line[tag_name.len()..].trim()) {
                let name = &cap["name"];
                let value = &cap["value"];
                element.set_attribute(name, value)?;
            }
            parent_stack.push((element, ""));
            current_indent += 1;

        // variable
        } else if raw_line.starts_with("$") && is_valid_identifier(&raw_line.split("(").next().unwrap()[1..]) {
            let name = &raw_line.split("(").next().unwrap()[1..];
            if variables.contains_key(name) {
                // make sure there are no references to variables that are still being defined:
                for (_, x) in parent_stack.as_slice() {
                    if *x == name {
                        return Err("Syntax error: variable is still being defined".into());
                    }
                }
                let (var, arg_places) = variables.get(name).unwrap();
                // function call
                if arg_places.len() > 0 {
                    if raw_line.split("(").count() != 2 {
                        return Err("Syntax Error: invalid function call".into());
                    }
                    // get given args
                    let given_args_raw : Vec<&str> = raw_line.split("(").nth(1).unwrap().split(")").next().unwrap().split(",").collect();
                    let mut given_args: Vec<Element> = vec![];
                    for arg_raw in given_args_raw {
                        let arg = arg_raw.trim();
                        // string
                        if arg.starts_with("\"") && arg.ends_with("\"") {
                            // create span element with string inside
                            let element = document.create_element("span")?;
                            element.set_inner_html(&raw_line[1..raw_line.len()-1]);
                            given_args.push(element);
                        } else if arg.starts_with("$") {
                            // insert variable element
                            let (arg_var, arg_var_arg_places) = variables.get(name).unwrap();
                            if !arg_var_arg_places.is_empty() {
                                return Err("Syntax Error: Function calls inside other function calls are not allowed".into());
                            }
                            let cloned_given_arg = arg_var.clone_node_with_deep(true).unwrap().dyn_into::<web_sys::Element>().unwrap();
                            given_args.push(cloned_given_arg);
                        } else {
                            return Err("Syntax Error: unknown argument type".into());
                        }
                    }
                    // insert args into place
                    for ((_, arg_place), given_arg) in arg_places.iter().zip(given_args.iter()) {
                        for place in arg_place {
                            let _ = place.append_child(given_arg);
                        }
                    }
                    // create a copy of the prepared function body
                    let new_element = var.clone_node_with_deep(true).unwrap().dyn_into::<web_sys::Element>().unwrap();
                    // undo inserting args, so the function is ready for the next time
                    for (_, arg_place) in arg_places {
                        for place in arg_place {
                            let _ = place.remove_child(&place.last_child().unwrap());
                        }
                    }
                    // add new prepared and cloned function element to the stack
                    parent_stack.push((new_element, ""));
                // variable access
                } else {
                    let new_element = var.clone_node_with_deep(true).unwrap().dyn_into::<web_sys::Element>().unwrap();
                    parent_stack.push((new_element, ""));
                }
                current_indent += 1;
            } else {
                return Err("Error: variable not defined".into());
            }

        // error
        } else {
            return Err("Syntax error: not sure that this line is".into());
        }
    }
    
    Ok(())
}
