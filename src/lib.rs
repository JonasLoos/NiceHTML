use std::rc::Rc;
use std::cell::RefCell;
use pest::iterators::Pair;
use wasm_bindgen::prelude::*;
use web_sys::Document;
use web_sys::{window, console, Element};
use std::collections::HashMap;
use std::hash::Hash;
use std::panic;
use pest::Parser;
use pest_derive::Parser;



/// simple logging using a format string.
macro_rules! log {
    ($($arg:tt)*) => {{
        console::log_1(&format!($($arg)*).into());
    }};
}

/// simple error creation using a format string.
macro_rules! err {
    ($($arg:tt)*) => {{
        Err(format!($($arg)*).into())
    }};
}



/// An element representing something like a tag in html. It can have attributes and children. It also has a scope field for variable support.
#[derive(Debug)]
#[derive(Clone)]
struct NiceElement {
    tag_name: String,
    attributes: Vec<(String, String)>,
    children: Vec<NiceThing>,
    scope: Rc<RefCell<NiceScope>>,
}

/// A simple string element
#[derive(Debug)]
#[derive(Clone)]
struct NiceString {
    content: String,
}

/// A placeholder element for function arguments. It gets replaced when the function is called.
#[derive(Debug)]
#[derive(Clone)]
struct NicePlaceholder {
    arg_name: String,
}

/// Either an element, string, or placeholder.
#[derive(Debug)]
#[derive(Clone)]
enum NiceThing {
    Element(NiceElement),
    Str(NiceString),
    Placeholder(NicePlaceholder),
}

/// A variable element for use inside the scopes (i.e. in the variable hashmap stack)
#[derive(Debug)]
#[derive(Clone)]
struct NiceVariable {
    name: String,
    args: Vec<String>,
    body: NiceThing,
}

/// A scope for variables and arguments with a hash-map for lookup. Is connected with parent (upper level element) scopes, creating something like a stack.
#[derive(Debug)]
#[derive(Clone)]
struct NiceScope {
    scope: Option<HashMap<String, NiceVariable>>,
    parent: Option<Rc<RefCell<NiceScope>>>,
}

impl NiceElement {

    fn new_empty() -> Self {
        Self {
            tag_name: "div".to_string(),
            attributes: vec![],
            children: vec![],
            scope: Rc::new(RefCell::new(NiceScope{scope: None, parent: None})),
        }
    }

    fn new_str(&mut self, string: String){
        self.append_child(NiceThing::Str(NiceString{content: string}));
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
        self.scope.as_ref().borrow().get_variable(name)
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

impl NiceScope {
    fn get_variable(&self, name: &str) -> Option<NiceVariable> {
        if let Some(scope) = &self.scope {
            if let Some(var) = scope.get(name) {
                return Some(var.clone());
            }
        }
        if let Some(parent) = &self.parent {
            return parent.as_ref().borrow().get_variable(name);
        }
        None
    }
}



#[derive(Parser)]
#[grammar = "src/grammar.pest"]
struct NiceHTMLParser;

/// Transpile NiceHTML to real HTML, with gets inserted into the DOM.
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
    // log!("{}", &stack_to_str(&stack)?);

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
            map.insert(arg_name.clone(), NiceVariable { name: arg_name.clone(), args: vec![], body: NiceThing::Placeholder(NicePlaceholder {arg_name}) });
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
    let mut id_attributes: Vec<String> = vec![];
    let mut class_attributes: Vec<String> = vec![];
    while let Some(attribute_) = line_inner.next() {
        let attribute = attribute_.into_inner().next().unwrap();
        match attribute.as_rule() {
            Rule::id_attribute => {
                let attr_value = unwrap_identifier(attribute.into_inner().next().unwrap())?;
                id_attributes.push(attr_value.to_string());
            },
            Rule::class_attribute => {
                let attr_value = unwrap_identifier(attribute.into_inner().next().unwrap())?;
                class_attributes.push(attr_value.to_string());
            },
            _ => {
                let mut attribute_inner = attribute.into_inner();
                let attr_name = unwrap_identifier(attribute_inner.next().unwrap())?;
                let attr_value = unwrap_string(attribute_inner.next().unwrap())?;
                attributes.push((attr_name.to_string(), attr_value.to_string()));
            }
        }
    }
    if id_attributes.len() > 1 {
        return err!("multiple id attributes found");
    }
    for id in id_attributes {
        attributes.push(("id".to_string(), id));
    }
    if class_attributes.len() > 0 {
        attributes.push(("class".to_string(), class_attributes.join(" ")));
    }

    // process children
    let mut new_stack: NiceElement = NiceElement {
        tag_name: tag_name.to_string(),
        attributes: attributes,
        children: vec![],
        scope: Rc::new(RefCell::new(NiceScope{scope: None, parent: Some(stack.scope.clone())})),
    };
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
        arg_names = variable.args;
        var_body = variable.body;
        for arg_name in arg_names.clone() {
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
                result.push_str(element.tag_name.as_str());
                result.push_str("[");
                result.push_str(stack_to_str(&element)?.as_str());
                result.push_str("], ");
            },
            NiceThing::Str(string) => {
                result.push_str("\"");
                result.push_str(&string.content);
                result.push_str("\", ");
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

