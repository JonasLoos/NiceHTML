use std::borrow::BorrowMut;
use std::ops::Deref;
use std::rc::Rc;
use std::cell::RefCell;
use pest::iterators::Pair;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::Document;
use web_sys::HtmlElement;
use web_sys::Node;
use web_sys::{window, console, Element};
use std::collections::HashMap;
use std::hash::Hash;
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


#[derive(Debug)]
#[derive(Clone)]
struct NiceElement {
    tag_name: String,
    attributes: Vec<(String, String)>,
    children: Vec<NiceThing>,
    scope: Rc<RefCell<VarScope>>,  // TODO: do I need RefCell? I only want to change it when I insert it's parent. Maybe I can do that before I add it as child
}

#[derive(Debug)]
#[derive(Clone)]
struct NiceString {
    content: String,
}

#[derive(Debug)]
#[derive(Clone)]
struct NicePlaceholder {
    arg_name: String,
}

#[derive(Debug)]
#[derive(Clone)]
enum NiceThing {
    Element(NiceElement),
    Str(NiceString),
    Placeholder(NicePlaceholder),
}

#[derive(Debug)]
#[derive(Clone)]
struct NiceVariable {
    name: String,
    args: Vec<String>,
    body: NiceThing,
}

#[derive(Debug)]
#[derive(Clone)]
struct VarScope {
    scope: Option<HashMap<String, NiceVariable>>,
    parent: Option<Rc<RefCell<VarScope>>>,
}

impl NiceElement {

    fn new_empty() -> Self {
        Self {
            tag_name: "div".to_string(),
            attributes: vec![],
            children: vec![],
            scope: Rc::new(RefCell::new(VarScope{scope: None, parent: None})),
        }
    }

    fn new_str(&mut self, string: String){
        self.append_child(NiceThing::Str(NiceString{content: string}));
    }

    fn new_placeholder(&mut self, arg_name: String) {
        self.append_child(NiceThing::Placeholder(NicePlaceholder{arg_name: arg_name}));
    }

    fn append_child(&mut self, child: NiceThing) {
        self.children.push(child);
    }

    fn add_variable(&mut self, variable: NiceVariable) {
        let mut scope = self.scope.as_ref().borrow_mut();
        if scope.scope.is_none() {
            scope.scope = Some(HashMap::new());
        }
        scope.scope.as_mut().unwrap().insert(variable.name.clone(), variable);
    }

    fn get_variable(&self, name: &str) -> Option<NiceVariable> {
        self.scope.as_ref().borrow().get_variable(name).clone()
    }
    
    

    fn insert_arguments(&mut self, arg_names: &Vec<String>, args: Vec<NiceThing>) {
        for child in self.children.iter_mut() {
            match child {
                NiceThing::Element(element) => {
                    element.insert_arguments(arg_names, args.clone());
                },
                NiceThing::Str(_) => {},
                NiceThing::Placeholder(placeholder) => {
                    if let Some(pos) = arg_names.iter().position(|x| *x == placeholder.arg_name) {
                        *child = args[pos].clone();
                    }
                }
            }
        }
    }
}

impl VarScope {
    fn get_variable(&self, name: &str) -> Option<NiceVariable> {
        if let Some(scope) = &self.scope {
            if let Some(var) = scope.get(name) {
                return Some(var.clone());
            }
        }
        if let Some(parent) = &self.parent {
            return parent.as_ref().borrow().get_variable(name).clone();
        }
        None
    }
}


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
    let mut stack: NiceElement = NiceElement::new_empty();

    // process pairs
    for pair in pairs {
        process_line(pair, &mut stack)?;
    }

    // log
    log!("{}", &stack_to_str(&stack)?);

    // convert stack to html and append to body
    for elem in stack_to_html(&stack, &document)? {
        body.append_child(&elem)?;
    }

    Ok(())
}

fn process_line(pair: Pair<Rule>, stack: &mut NiceElement) -> Result<(), JsValue> {
    match pair.as_rule() {
        Rule::definition => {process_definition(pair, stack)?; None},
        Rule::element => Some(process_element(pair, stack)?),
        Rule::variable => Some(process_variable(pair, stack)?),
        Rule::string_line => Some(process_string_line(pair, stack)?),
        Rule::EOI => None,
        idk => return err!("invalid child: {:?}", idk)
    };
    Ok(())
}

fn process_definition(pair: Pair<Rule>, stack: &mut NiceElement) -> Result<(), JsValue> {
    // process definition and its children recursively
    let mut pair_inner = pair.into_inner();
    let name = unwrap_identifier(pair_inner.next().unwrap())?;
    let mut tmp: Vec<Pair<Rule>> = vec![];
    while let Some(x) = pair_inner.next() {
        tmp.push(x);
    }
    let arg_names: Vec<String> = tmp[..tmp.len()-1].iter().map(|x| unwrap_identifier(x.clone()).unwrap().to_string()).collect();
    let definition_body = tmp.last().unwrap().clone();
    let mut definition_body_element = NiceElement::new_empty();
    {
        let mut scope = definition_body_element.scope.as_ref().borrow_mut();
        scope.parent = Some(stack.scope.clone());
        let mut map = HashMap::new();
        for arg_name in arg_names.clone() {
            map.insert(arg_name.clone(), NiceVariable { name: arg_name.clone(), args: vec![], body: NiceThing::Placeholder(NicePlaceholder {arg_name: arg_name}) });
        }
        scope.scope = Some(map);
    }
    process_children(definition_body, &mut definition_body_element)?;

    // create variable
    let variable = NiceVariable {
        name: name.to_string(),
        args: arg_names,
        body: NiceThing::Element(definition_body_element),
    };

    log!("created variable `{}`", variable.name);

    stack.add_variable(variable);

    Ok(())
}

fn process_element(pair: Pair<Rule>, stack: &mut NiceElement) -> Result<(), JsValue> {
    // process element and its children recursively
    let mut pair_inner = pair.into_inner();
    let mut line_inner = pair_inner.next().unwrap().into_inner();

    // create element
    let tag_name = unwrap_identifier(line_inner.next().unwrap())?;
    
    // handle attributes
    let mut attributes: Vec<(String, String)> = vec![];
    while let Some(attribute) = line_inner.next() {
        let mut attribute_inner = attribute.into_inner();
        let attr_name = unwrap_identifier(attribute_inner.next().unwrap())?;
        let attr_value = unwrap_string(attribute_inner.next().unwrap())?;
        attributes.push((attr_name.to_string(), attr_value.to_string()));
    }

    let mut new_stack: NiceElement = NiceElement {
        tag_name: tag_name.to_string(),
        attributes: attributes,
        children: vec![],
        scope: Rc::new(RefCell::new(VarScope{scope: None, parent: Some(stack.scope.clone())})),
    };

    // process children
    if let Some(children) = pair_inner.next() {
        process_children(children, &mut new_stack)?
    }

    log!("created element `{}`", new_stack.tag_name);

    stack.append_child(NiceThing::Element(new_stack));

    Ok(())
}


fn process_variable(pair: Pair<Rule>, stack: &mut NiceElement) -> Result<(), JsValue> {
    // process variable
    let mut pair_inner = pair.into_inner();

    // get variable name and definition
    let var_name = unwrap_identifier(pair_inner.next().unwrap())?;
    let mut var_body: NiceThing;
    let arg_names: Vec<String>;
    let mut args: Vec<NiceThing> = vec![];
    if let Some(variable) = stack.get_variable(var_name) {
        arg_names = variable.args.clone();
        var_body = variable.body.clone();
        for arg_name in variable.args.clone() {
            if let Some(arg_pair) = pair_inner.next() {
                let mut arg_elem = NiceElement::new_empty();
                arg_elem.scope.as_ref().borrow_mut().parent = Some(stack.scope.clone());
                match arg_pair.as_rule() {
                    Rule::string => {
                        arg_elem.new_str(unwrap_string(arg_pair)?.to_string());
                    }
                    Rule::variable => {
                        process_variable(arg_pair, &mut arg_elem)?;
                    }
                    tmp => return err!("invalid argument type `{:?}` for arg `{}` for function `{}`", tmp, arg_name, var_name)
                }
                args.push(NiceThing::Element(arg_elem));
            } else {
                return err!("not enough arguments for function `{}`", var_name)
            }
        }
        if let Some(_) = pair_inner.next() {
            return err!("too many arguments for function `{}`", var_name)
        }
    } else {
        // TODO: this also happens, if there is a variable wich is an argument to an other calling function
        // TODO: this might be solvable, if the scope of the calling function is prepended to the scope of the called function
        return err!("variable `{}` not defined. Current scope: `{:?}`", var_name, stack.scope.as_ref().borrow());
    }

    if let NiceThing::Element(ref mut var_body_elem) = &mut var_body {
        var_body_elem.scope.as_ref().borrow_mut().parent = Some(stack.scope.clone());  // TODO: check if correct
        var_body_elem.insert_arguments(&arg_names, args);
        log!("inserted variable `{}({}) = {:?}[{}]`", var_name, arg_names.join(", "), var_body_elem.tag_name, stack_to_str(var_body_elem)?);
    }


    stack.append_child(var_body);

    Ok(())
}

fn process_string_line(pair: Pair<Rule>, stack: &mut NiceElement) -> Result<(), JsValue> {
    // process string
    let mut pair_inner = pair.into_inner();
    let string = unwrap_string(pair_inner.next().unwrap())?;
    stack.new_str(string.to_string());
    Ok(())
}

fn process_children(pair: Pair<Rule>, stack: &mut NiceElement) -> Result<(), JsValue> {
    // process children
    for child in pair.into_inner() {
        process_line(child, stack)?;
    }
    Ok(())
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
    let str = string.as_str();
    Ok(&str[1..str.len()-1])
}

fn stack_to_html(stack: &NiceElement, document: &Document) -> Result<Vec<Element>, JsValue> {
    let mut result = vec![];
    for child in &stack.children {
        match child {
            NiceThing::Element(element) => {
                let new_element = document.create_element(&element.tag_name)?;
                for (name, value) in &element.attributes {
                    new_element.set_attribute(&name, &value)?;
                }
                let children = stack_to_html(&element, document)?;
                for child in children {
                    new_element.append_child(&child)?;
                }
                result.push(new_element)
            },
            NiceThing::Str(string) => {
                let new_element = document.create_element("span")?;
                new_element.set_inner_html(&string.content);
                result.push(new_element);
            },
            NiceThing::Placeholder(_) => {
                return err!("placeholder found in final stack");
            }
        }
    }

    Ok(result)
}

fn stack_to_str(stack: &NiceElement) -> Result<String, JsValue> {
    let mut result = String::new();
    for child in &stack.children {
        match child {
            NiceThing::Element(element) => {
                for (name, value) in &element.attributes {
                    // ...
                }
                result.push_str(element.tag_name.as_str());
                result.push_str("[");
                result.push_str(stack_to_str(&element)?.as_str());
                result.push_str("], ");
            },
            NiceThing::Str(string) => {
                result.push_str("\"");
                result.push_str(&string.content);
                result.push_str("\"");
                result.push_str(", ");
            },
            NiceThing::Placeholder(placeholder) => {
                result.push_str("$");
                result.push_str(placeholder.arg_name.as_str());
                result.push_str(", ");
            }
        }
    }

    Ok(result)
}

