use pest::iterators::Pair;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{window, console, Element};
use std::collections::HashMap;
use regex::Regex;
use std::panic;
use pest::Parser;
use pest_derive::Parser;


macro_rules! log {
    ($($arg:tt)*) => {{
        console::log_1(&format!($($arg)*).into());
    }};
}

macro_rules! err {
    ($($arg:tt)*) => {{
        Err(format!($($arg)*).into())
    }};
}


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

fn err(msg: &str, line_nr: u32, line: &str) -> Result<(), JsValue> {
    return Err(format!("NiceHTML Error: {}\nline {}: `{}`", msg, line_nr, line).into())
}

#[wasm_bindgen]
pub fn render(script: &str) -> Result<(), JsValue> {
    // setup error logging
    panic::set_hook(Box::new(console_error_panic_hook::hook));

    // initialize variables
    let window = window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    let body = document.body().unwrap();
    let lines: Vec<&str> = script.lines().collect();
    let mut parent_stack: Vec<(Element, &str)> = vec![(body.clone().into(), "")];  // stack of elements, which can be variables/functions (then the name is set)
    let mut variables: HashMap<&str, (Element, Vec<(&str, Vec<String>)>)> = HashMap::new();  // mapping from variable/function name to tuple of Element and places for each argument to be inserted (list of lists)
    let mut next_placeholder_id: i32 = 0;
    let mut current_indent = 0;
    
    for (line, line_nr) in lines.iter().zip(1..) {
        // ignore empty lines and comments
        if line.trim().is_empty() || line.trim().starts_with("#") {
            continue;
        }
        // Parse line for element, id, class, etc.
        // For simplicity, let's assume it's just the tag name
        
        let indent = line.chars().take_while(|&c| c == ' ').count() / 4 + 1;
        if indent > current_indent + 1 {
            return err("too much indentation at once", line_nr, line);
        }
        while indent <= current_indent {
            let (finished_element, var_name) = parent_stack.pop().unwrap();
            // console::log_1(&format!("finished element {}", finished_element.outer_html()).into());  // for debugging
            current_indent -= 1;
            if var_name.is_empty() {
                // append normal elements to their parent. Don't do this for variables/functions (for which `var_name` is set)
                parent_stack.last().unwrap().0.append_child(&finished_element)?;
            } else {
                // overwrite the dummy variable element with the new finished element
                let prev_var_entry = variables.remove(var_name).unwrap();
                variables.insert(var_name, (finished_element, prev_var_entry.1));
            }
        }
        let raw_line = line.trim();

        // assignment of variable / function
        if raw_line.ends_with("=") {
            let definition = raw_line[0..raw_line.len()-1].trim();
            let name: &str;
            let mut args: Vec<&str> = vec![];

            // variable
            if definition.split_whitespace().count() == 1 && is_valid_identifier(definition) {
                name = definition;

            // function
            } else if definition.contains("(") && definition.contains(")") {
                name = definition.split("(").next().unwrap();
                if !is_valid_identifier(name) {
                    return err("not a valid variable or function definition", line_nr, line);
                }
                if !definition.split("(").nth(1).unwrap().split(")").nth(1).unwrap().trim().is_empty() {
                    return err("not a valid variable or function definition", line_nr, line);
                }
                args = definition.split("(").nth(1).unwrap().split(")").next().unwrap().split(",").collect();

            } else {
                return err("not a valid variable or function definition", line_nr, line);
            }

            // initialize the argument places arrayws for the function
            let mut arg_places: Vec<(&str,Vec<String>)> = vec![];
            for arg_name in args {
                arg_places.push((arg_name.trim(), vec![]));
            }

            // create div where the function content is put into
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
        } else if Regex::new(r#"^[a-zA-Z_]\w*(\s+\w+="[^"]*")*$"#).unwrap().is_match(raw_line) {
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

        // variable access / function call
        } else if raw_line.starts_with("$") && is_valid_identifier(&raw_line.split("(").next().unwrap()[1..]) {
            let name = raw_line.split("(").next().unwrap()[1..].trim();
            let new_element: Element;

            // if this is inside a function definition and an argument is accessed, create an empty div as placeholder
            let mut placeholder: Option<Element> = None;
            for (_, parent_name) in parent_stack.iter().rev() {
                if let Some((_, parent_arg_places)) = variables.get_mut(parent_name) {
                    // if parent is a function
                    if !parent_arg_places.is_empty() {
                        // if the variable is an argument of the function, insert a placeholder
                        if parent_arg_places.iter().any(|x| x.0 == name) {
                            let placeholder_id = format!("nicehtmlID-{}-{}-{}", parent_name, name, next_placeholder_id);
                            let placeholder_elem = document.create_element("div")?;
                            placeholder_elem.set_attribute("id", &placeholder_id)?;
                            let arg_pos = parent_arg_places.iter().position(|x| x.0 == name).unwrap();
                            parent_arg_places[arg_pos].1.push(placeholder_id);
                            next_placeholder_id += 1;
                            placeholder = Some(placeholder_elem);
                            break;
                        }
                    }
                }
            }

            if placeholder.is_some() {
                // the found variable is an argument, so use the placeholder
                new_element = placeholder.unwrap();
            } else if variables.contains_key(name) {
                // make sure there are no references to variables that are still being defined:
                for (_, x) in parent_stack.as_slice() {
                    if *x == name {
                        return err("variable is still being defined", line_nr, line);
                    }
                }
                let (_, arg_places) = variables.get(name).unwrap();
                // function call
                if arg_places.len() > 0 {
                    if raw_line.split("(").count() != 2 {
                        return err("invalid function call", line_nr, line);
                    }
                    // get given args
                    let given_args_raw = raw_line.split("(").nth(1).unwrap().split(")").next().unwrap();
                    let re_full = Regex::new(r#"((".*?"|\$[a-zA-Z_][a-zA-Z_0-9]*),\s*)*(".*?"|\$[a-zA-Z_][a-zA-Z_0-9]*)?,?"#).unwrap();
                    if !re_full.is_match(given_args_raw) {
                        return err("invalid function call", line_nr, line);
                    }
                    let re = Regex::new(r#"".*?"|\$[a-zA-Z_][a-zA-Z_0-9]*"#).unwrap();
                    let mut given_args: Vec<Element> = vec![];
                    for arg_cap in re.captures_iter(given_args_raw) {
                        // let arg = arg_raw.trim();
                        let arg = &arg_cap[0];
                        // string
                        if arg.starts_with("\"") && arg.ends_with("\"") {
                            // create span element with string inside
                            let element = document.create_element("span")?;
                            element.set_inner_html(&arg[1..arg.len()-1]);
                            given_args.push(element);
                        } else if arg.starts_with("$") {
                            // insert variable element
                            // TODO: not only check variables, but also function arguments?
                            // TODO: for nested functions, it seems like we receive here the called function as argument instead of the given arguments
                            let (arg_var, arg_var_arg_places) = variables.get(name).unwrap();
                            if !arg_var_arg_places.iter().any(|x| x.1.len() > 0) {
                                return err("Function calls inside other function calls are not allowed", line_nr, line);
                            }
                            let cloned_given_arg = arg_var.clone_node_with_deep(true).unwrap().dyn_into::<web_sys::Element>().unwrap();
                            given_args.push(cloned_given_arg);
                        } else {
                            return err(&format!("unknown argument type for `{}`", arg), line_nr, line);
                        }
                    }

                    // create a copy of the function body
                    new_element = variables.get(name).unwrap().0.clone_node_with_deep(true).unwrap().dyn_into::<web_sys::Element>().unwrap();

                    // insert args into place
                    for ((_, arg_place), given_arg) in arg_places.iter().zip(given_args.iter()) {
                        for place in arg_place {
                            // search for the placeholder with the correct id and insert the argument there
                            if let Some(placeholder_elem) = new_element.query_selector(&format!("#{}",place))? {
                                placeholder_elem.append_child(&given_arg.clone_node_with_deep(true).unwrap().dyn_into::<web_sys::Element>().unwrap())?;
                            } else {
                                // internal error, this should not happen
                                return err(&format!("placeholder (`{}`) not found in element `{}`", place, new_element.inner_html()), line_nr, line);
                            }
                        }
                    }

                // variable access
                } else {
                    new_element = variables.get(name).unwrap().0.clone_node_with_deep(true).unwrap().dyn_into::<web_sys::Element>().unwrap();
                }
            } else {
                return err("Variable not defined", line_nr, line);
            }
            parent_stack.push((new_element, ""));
            current_indent += 1;

        // error
        } else {
            return err("Not sure what this line is", line_nr, line);
        }
    }
    // pop all remaining elements (except body) from the stack
    while current_indent > 0 {
        let (previous_top, var_name) = parent_stack.pop().unwrap();
        current_indent -= 1;
        if var_name.is_empty() {
            // append normal elements to their parent. Don't do this for variables/functions
            parent_stack.last().unwrap().0.append_child(&previous_top)?;
        }
    }
    
    Ok(())
}



struct VariableHashMapStack<K, V> {
    stack: Vec<HashMap<K, V>>,
}

impl<K, V> VariableHashMapStack<K, V>
where
    K: std::cmp::Eq + std::hash::Hash + Clone,
    V: Clone,
{
    fn new() -> Self {
        VariableHashMapStack { stack: vec![HashMap::new()] }
    }

    fn push(&mut self) {
        self.stack.push(HashMap::new());
    }

    fn pop(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
        }
    }

    fn insert(&mut self, key: K, value: V) {
        if let Some(top) = self.stack.last_mut() {
            top.insert(key, value);
        }
    }

    fn get(&self, key: &K) -> Option<V> {
        for map in self.stack.iter().rev() {
            if let Some(value) = map.get(key) {
                return Some(value.clone());
            }
        }
        None
    }
}

// variable_name: (argument_names, definition?, element?)
// definition: Pairs are good to easily insert normal variables during the processing of the function body
// element: Elements are good to handle nested function calls, where pairs would cause arugment function calls to use the inner variable scope instead of the outer one
type VarStack<'a> = VariableHashMapStack<&'a str, (Vec<&'a str>, Option<Pair<'a, Rule>>, Option<Element>)>;


#[derive(Parser)]
#[grammar = "src/grammar.pest"]
struct NiceHTMLParser;

#[wasm_bindgen]
pub fn transpile(input: &str) -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    let window = window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    let body = document.body().unwrap();
    let pairs = NiceHTMLParser::parse(Rule::root, &input).map_err(|err| err.to_string())?;
    let mut variables: VarStack = VariableHashMapStack::new();

    // log pairs to console
    for pair in pairs.clone() {
        console::log_1(&format!("{:?}", pair).into());
    }

    // process pairs
    for pair in pairs {
        let element = match pair.as_rule() {
            Rule::definition => {process_definition(pair, &document, &mut variables)?; None},
            Rule::element => Some(process_element(pair, &document, &mut variables)?),
            Rule::variable => Some(process_variable(pair, &document, &mut variables)?),
            Rule::string_line => Some(process_string_line(pair, &document)?),
            Rule::EOI => None,
            idk => return err!("invalid child: {:?}", idk)
        };
        if let Some(element) = element {
            body.append_child(&element)?;
        }
    }
    Ok(())
}

fn process_definition(pair: Pair<Rule>, _document: &web_sys::Document, variables: &mut VarStack) -> Result<(), JsValue> {
    // process definition and its children recursively
    log!("processing definition {:?}", pair);
    let mut pair_inner = pair.into_inner();
    let name = unwrap_identifier(pair_inner.next().unwrap())?;
    let mut tmp: Vec<Pair<Rule>> = vec![];
    while let Some(x) = pair_inner.next() {
        tmp.push(x);
    }
    let arg_names = tmp[..tmp.len()-1].iter().map(|x| unwrap_identifier(x.clone()).unwrap()).collect();
    let definition_body = tmp.last().unwrap().clone();
    let new_pair: Pair<Rule> = Pair::new();
    variables.insert(name, (arg_names, Some(definition_body), None));

    Ok(())
}

fn process_element(pair: Pair<Rule>, document: &web_sys::Document, variables: &mut VarStack) -> Result<Element, JsValue> {
    // process element and its children recursively
    log!("processing element {:?}", pair);
    let mut pair_inner = pair.into_inner();
    let mut line_inner = pair_inner.next().unwrap().into_inner();

    // create element
    let tag_name = unwrap_identifier(line_inner.next().unwrap())?;
    let element = document.create_element(tag_name)?;
    
    // handle attributes
    if let Some(attributes) = line_inner.next() {
        log!("processing attributes {:?}", attributes);
        // TODO
    }

    // process children
    if let Some(children) = pair_inner.next() {
        for child in process_children(children, document, variables)? {
            element.append_child(&child)?;
        }
    }

    Ok(element)
}

fn process_variable(pair: Pair<Rule>, document: &web_sys::Document, variables: &mut VarStack) -> Result<Element, JsValue> {
    // process variable
    let mut pair_inner = pair.into_inner();

    // get variable name and definition
    let var_name = unwrap_identifier(pair_inner.next().unwrap())?;
    if variables.get(&var_name).is_none() {
        return err!("variable `{}` not defined", var_name);
    }
    let (arg_names, var_definition_, var_element_) = variables.get(&var_name).unwrap();

    if let Some(var_definition) = var_definition_ {
        // variable is a function that has not been processed yet
        // parse arguments
        let mut args: Vec<(&str, Element)> = vec![];
        for arg_name in arg_names {
            if let Some(arg_pair) = pair_inner.next() {
                match arg_pair.as_rule() {
                    Rule::string => {
                        let element = document.create_element("span")?;
                        element.set_inner_html(unwrap_string(arg_pair)?);
                        args.push((arg_name, element));
                    }
                    Rule::variable => {
                        let element = process_variable(arg_pair, document, variables)?;
                        args.push((arg_name, element));
                    }
                    _ => {}
                }
            } else {
                return err!("not enough arguments for function `{}`", var_name)
            }
        }
        if let Some(_) = pair_inner.next() {
            return err!("too many arguments for function `{}`", var_name)
        }
        variables.push();
        for (arg_name, arg_element) in args {
            variables.insert(arg_name, (vec![], None, Some(arg_element)));
        }
        let var_element = document.create_element("div")?;
        for child in process_children(var_definition, document, variables)? {
            var_element.append_child(&child)?;
        }
        variables.pop();
        Ok(var_element)
    } else if let Some(var_element) = var_element_ {
        // variable is a variable
        Ok(var_element)
    } else {
        // variable is not defined
        err!("variable `{}` not defined", var_name)
    }
}

fn process_string_line(pair: Pair<Rule>, document: &web_sys::Document) -> Result<Element, JsValue> {
    // process string
    log!("processing string line {:?}", pair);
    let mut pair_inner = pair.into_inner();
    let string = unwrap_string(pair_inner.next().unwrap())?;
    let element = document.create_element("span")?;
    element.set_inner_html(string);
    Ok(element)
}

fn process_children(pair: Pair<Rule>, document: &web_sys::Document, variables: &mut VarStack) -> Result<Vec<Element>, JsValue> {
    // process children
    log!("processing children {:?}", pair);
    let mut elements: Vec<Element> = vec![];
    for child in pair.into_inner() {
        let element: Option<Element>  = match child.as_rule() {
            Rule::definition => {process_definition(child, document, variables)?; None},
            Rule::element => Some(process_element(child, document, variables)?),
            Rule::variable => Some(process_variable(child, document, variables)?),
            Rule::string_line => Some(process_string_line(child, document)?),
            Rule::EOI => None,
            idk => return err!("invalid child: {:?}", idk)

        };
        if let Some(element) = element {
            elements.push(element);
        }
    }
    Ok(elements)
}

fn unwrap_identifier(identifier: Pair<Rule>) -> Result<&str, JsValue> {
    if identifier.as_rule() != Rule::identifier {
        return err!("expected identifier");
    }
    Ok(identifier.as_str())
}

fn unwrap_string(string: Pair<Rule>) -> Result<&str, JsValue> {
    if string.as_rule() != Rule::string {
        return err!("expected string");
    }
    Ok(string.as_str())
}
