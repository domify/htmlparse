//! html5ever's answers to html5lib-tests' inputs, as JSON lines the ris
//! tests read: spec/golden/tokenizer.jsonl and spec/golden/tree.jsonl.

use std::borrow::Cow;
use std::cell::RefCell;
use std::fs;
use std::io::Write;
use std::path::Path;

use html5ever::interface::{ElemName, ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::tokenizer::states::{self, RawKind};
use html5ever::tokenizer::{
    BufferQueue, Tag, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
};
use html5ever::tree_builder::TreeBuilderOpts;
use html5ever::serialize::{serialize, AttrRef, Serialize, SerializeOpts, Serializer, TraversalScope};
use html5ever::{
    local_name, ns, parse_document, parse_fragment, Attribute, LocalName, Namespace, ParseOpts,
    QualName,
};
use serde_json::{json, Value};

fn main() {
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join("../spec");
    fs::create_dir_all(spec.join("golden")).unwrap();
    tokenizer_tests(&spec);
    tree_tests(&spec);
}

// --- tokenizer --------------------------------------------------------------

struct Tokens {
    out: RefCell<Vec<Value>>,
    text: RefCell<String>,
}

impl Tokens {
    fn flush(&self) {
        let mut text = self.text.borrow_mut();
        if !text.is_empty() {
            self.out.borrow_mut().push(json!(["Character", *text]));
            text.clear();
        }
    }

    fn push(&self, token: Value) {
        self.flush();
        self.out.borrow_mut().push(token);
    }
}

fn opt(value: &Option<StrTendril>) -> Value {
    match value {
        Some(v) => json!(&**v),
        None => Value::Null,
    }
}

impl TokenSink for Tokens {
    type Handle = ();

    fn process_token(&self, token: Token, _line: u64) -> TokenSinkResult<()> {
        match token {
            Token::CharacterTokens(text) => self.text.borrow_mut().push_str(&text),
            Token::NullCharacterToken => self.text.borrow_mut().push('\0'),
            Token::TagToken(Tag { kind: TagKind::StartTag, name, self_closing, attrs }) => {
                let mut map = serde_json::Map::new();
                for a in attrs {
                    map.insert(a.name.local.to_string(), json!(&*a.value));
                }
                let mut token = vec![json!("StartTag"), json!(&*name), Value::Object(map)];
                if self_closing {
                    token.push(json!(true));
                }
                self.push(Value::Array(token));
            }
            Token::TagToken(Tag { kind: TagKind::EndTag, name, .. }) => {
                self.push(json!(["EndTag", &*name]))
            }
            Token::CommentToken(text) => self.push(json!(["Comment", &*text])),
            Token::DoctypeToken(d) => self.push(json!([
                "DOCTYPE",
                opt(&d.name),
                opt(&d.public_id),
                opt(&d.system_id),
                !d.force_quirks
            ])),
            Token::EOFToken => self.flush(),
            Token::ParseError(_) => {}
        }
        TokenSinkResult::Continue
    }
}

fn state(name: &str) -> states::State {
    match name {
        "Data state" => states::Data,
        "PLAINTEXT state" => states::Plaintext,
        "RCDATA state" => states::RawData(RawKind::Rcdata),
        "RAWTEXT state" => states::RawData(RawKind::Rawtext),
        "Script data state" => states::RawData(RawKind::ScriptData),
        "CDATA section state" => states::CdataSection,
        other => panic!("unknown state {other}"),
    }
}

fn tokenize(input: &str, initial: &str, last_start_tag: Option<String>) -> Vec<Value> {
    let sink = Tokens { out: RefCell::new(vec![]), text: RefCell::new(String::new()) };
    let opts = TokenizerOpts {
        initial_state: Some(state(initial)),
        last_start_tag_name: last_start_tag,
        discard_bom: false,
        ..TokenizerOpts::default()
    };
    let tokenizer = Tokenizer::new(sink, opts);
    let queue = BufferQueue::default();
    queue.push_back(StrTendril::from(input));
    let _ = tokenizer.feed(&queue);
    tokenizer.end();
    tokenizer.sink.out.take()
}

fn tokenizer_tests(spec: &Path) {
    let mut out = fs::File::create(spec.join("golden/tokenizer.jsonl")).unwrap();
    let mut files: Vec<_> = fs::read_dir(spec.join("tokenizer"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "test"))
        .collect();
    files.sort();
    let mut count = 0;
    for path in files {
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        let doc: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let key = if doc.get("tests").is_some() { "tests" } else { "xmlViolationTests" };
        for (index, test) in doc[key].as_array().unwrap().iter().enumerate() {
            // Lone surrogates have no place in a Rust (or ris) string.
            if test.get("doubleEscaped").is_some() {
                continue;
            }
            let input = test["input"].as_str().unwrap();
            let states = match test.get("initialStates") {
                Some(s) => s.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect(),
                None => vec!["Data state".to_string()],
            };
            let last = test.get("lastStartTag").and_then(|v| v.as_str()).map(String::from);
            for st in states {
                let tokens = tokenize(input, &st, last.clone());
                let line = json!({
                    "file": file, "index": index, "description": test["description"],
                    "input": input, "state": st, "lastStartTag": last,
                    "output": tokens, "expected": test["output"],
                });
                writeln!(out, "{}", line).unwrap();
                count += 1;
            }
        }
    }
    eprintln!("tokenizer: {count} cases");
}

// --- tree construction ------------------------------------------------------

enum Data {
    Document,
    Element { name: QualName, attrs: Vec<Attribute>, template: Option<usize> },
    Text(String),
    Comment(String),
    Doctype { name: String, public_id: String, system_id: String },
}

struct Node {
    data: Data,
    parent: Option<usize>,
    children: Vec<usize>,
}

struct Sink {
    nodes: RefCell<Vec<Node>>,
}

#[derive(Debug)]
struct Name(QualName);

impl ElemName for Name {
    fn ns(&self) -> &Namespace {
        &self.0.ns
    }
    fn local_name(&self) -> &LocalName {
        &self.0.local
    }
}

impl Sink {
    fn new() -> Self {
        Sink { nodes: RefCell::new(vec![Node { data: Data::Document, parent: None, children: vec![] }]) }
    }

    fn add(&self, data: Data) -> usize {
        let mut nodes = self.nodes.borrow_mut();
        nodes.push(Node { data, parent: None, children: vec![] });
        nodes.len() - 1
    }

    fn detach(&self, id: usize) {
        let mut nodes = self.nodes.borrow_mut();
        if let Some(p) = nodes[id].parent.take() {
            nodes[p].children.retain(|&c| c != id);
        }
    }

    fn insert_at(&self, parent: usize, index: usize, child: NodeOrText<usize>) {
        match child {
            NodeOrText::AppendNode(node) => {
                self.detach(node);
                let mut nodes = self.nodes.borrow_mut();
                nodes[node].parent = Some(parent);
                nodes[parent].children.insert(index, node);
            }
            NodeOrText::AppendText(text) => {
                if index > 0 {
                    let prev = self.nodes.borrow()[parent].children[index - 1];
                    if let Data::Text(t) = &mut self.nodes.borrow_mut()[prev].data {
                        t.push_str(&text);
                        return;
                    }
                }
                let node = self.add(Data::Text(text.to_string()));
                let mut nodes = self.nodes.borrow_mut();
                nodes[node].parent = Some(parent);
                nodes[parent].children.insert(index, node);
            }
        }
    }
}

impl TreeSink for Sink {
    type Handle = usize;
    type Output = Vec<Node>;
    type ElemName<'a> = Name;

    fn finish(self) -> Vec<Node> {
        self.nodes.into_inner()
    }
    fn parse_error(&self, _msg: Cow<'static, str>) {}
    fn get_document(&self) -> usize {
        0
    }
    fn elem_name<'a>(&'a self, target: &'a usize) -> Name {
        match &self.nodes.borrow()[*target].data {
            Data::Element { name, .. } => Name(name.clone()),
            _ => panic!("not an element"),
        }
    }
    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> usize {
        let template = if flags.template { Some(self.add(Data::Document)) } else { None };
        self.add(Data::Element { name, attrs, template })
    }
    fn create_comment(&self, text: StrTendril) -> usize {
        self.add(Data::Comment(text.to_string()))
    }
    fn create_pi(&self, _target: StrTendril, data: StrTendril) -> usize {
        self.add(Data::Comment(data.to_string()))
    }
    fn append(&self, parent: &usize, child: NodeOrText<usize>) {
        let index = self.nodes.borrow()[*parent].children.len();
        self.insert_at(*parent, index, child);
    }
    fn append_based_on_parent_node(&self, element: &usize, prev: &usize, child: NodeOrText<usize>) {
        if self.nodes.borrow()[*element].parent.is_some() {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev, child);
        }
    }
    fn append_doctype_to_document(&self, name: StrTendril, public_id: StrTendril, system_id: StrTendril) {
        let node = self.add(Data::Doctype {
            name: name.to_string(),
            public_id: public_id.to_string(),
            system_id: system_id.to_string(),
        });
        self.append(&0, NodeOrText::AppendNode(node));
    }
    fn get_template_contents(&self, target: &usize) -> usize {
        match &self.nodes.borrow()[*target].data {
            Data::Element { template: Some(t), .. } => *t,
            _ => panic!("not a template"),
        }
    }
    fn same_node(&self, x: &usize, y: &usize) -> bool {
        x == y
    }
    fn set_quirks_mode(&self, _mode: QuirksMode) {}
    fn append_before_sibling(&self, sibling: &usize, new_node: NodeOrText<usize>) {
        let parent = self.nodes.borrow()[*sibling].parent.unwrap();
        let index = self.nodes.borrow()[parent].children.iter().position(|c| c == sibling).unwrap();
        self.insert_at(parent, index, new_node);
    }
    fn add_attrs_if_missing(&self, target: &usize, attrs: Vec<Attribute>) {
        if let Data::Element { attrs: existing, .. } = &mut self.nodes.borrow_mut()[*target].data {
            for a in attrs {
                if !existing.iter().any(|e| e.name == a.name) {
                    existing.push(a);
                }
            }
        }
    }
    fn remove_from_parent(&self, target: &usize) {
        self.detach(*target);
    }
    fn reparent_children(&self, node: &usize, new_parent: &usize) {
        let children = std::mem::take(&mut self.nodes.borrow_mut()[*node].children);
        for child in children {
            self.nodes.borrow_mut()[child].parent = None;
            self.append(new_parent, NodeOrText::AppendNode(child));
        }
    }
}

fn dump(nodes: &[Node], id: usize, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    match &nodes[id].data {
        Data::Document => {}
        Data::Doctype { name, public_id, system_id } => {
            if public_id.is_empty() && system_id.is_empty() {
                out.push_str(&format!("| {indent}<!DOCTYPE {name}>\n"));
            } else {
                out.push_str(&format!("| {indent}<!DOCTYPE {name} \"{public_id}\" \"{system_id}\">\n"));
            }
        }
        Data::Comment(text) => out.push_str(&format!("| {indent}<!-- {text} -->\n")),
        Data::Text(text) => out.push_str(&format!("| {indent}\"{text}\"\n")),
        Data::Element { name, attrs, template } => {
            let prefix = match name.ns {
                ns!(svg) => "svg ",
                ns!(mathml) => "math ",
                _ => "",
            };
            out.push_str(&format!("| {indent}<{prefix}{}>\n", name.local));
            let mut lines: Vec<String> = attrs
                .iter()
                .map(|a| {
                    let p = match a.name.ns {
                        ns!(xlink) => "xlink ",
                        ns!(xml) => "xml ",
                        ns!(xmlns) => "xmlns ",
                        _ => "",
                    };
                    format!("{p}{}=\"{}\"", a.name.local, a.value)
                })
                .collect();
            lines.sort();
            for l in lines {
                out.push_str(&format!("| {indent}  {l}\n"));
            }
            if let Some(t) = template {
                out.push_str(&format!("| {indent}  content\n"));
                for &c in &nodes[*t].children {
                    dump(nodes, c, depth + 2, out);
                }
            }
        }
    }
    for &c in &nodes[id].children {
        dump(nodes, c, depth + 1, out);
    }
}

/// A node of a finished tree, for html5ever's serializer: its children,
/// a template's contents first.
struct Ser<'a> {
    nodes: &'a [Node],
    id: usize,
}

impl Serialize for Ser<'_> {
    fn serialize<S: Serializer>(&self, s: &mut S, scope: TraversalScope) -> std::io::Result<()> {
        let node = &self.nodes[self.id];
        let children = |s: &mut S| -> std::io::Result<()> {
            if let Data::Element { template: Some(t), .. } = &node.data {
                for &c in &self.nodes[*t].children {
                    Ser { nodes: self.nodes, id: c }.serialize(s, TraversalScope::IncludeNode)?;
                }
            }
            for &c in &node.children {
                Ser { nodes: self.nodes, id: c }.serialize(s, TraversalScope::IncludeNode)?;
            }
            Ok(())
        };
        if scope != TraversalScope::IncludeNode {
            return children(s);
        }
        match &node.data {
            Data::Document => children(s),
            Data::Element { name, attrs, .. } => {
                let refs: Vec<AttrRef> = attrs.iter().map(|a| (&a.name, &*a.value)).collect();
                s.start_elem(name.clone(), refs.into_iter())?;
                children(s)?;
                s.end_elem(name.clone())
            }
            Data::Text(t) => s.write_text(t),
            Data::Comment(t) => s.write_comment(t),
            Data::Doctype { name, .. } => s.write_doctype(name),
        }
    }
}

fn html(nodes: &[Node], root: usize, scripting: bool) -> String {
    let mut out = Vec::new();
    let opts = SerializeOpts { scripting_enabled: scripting, ..SerializeOpts::default() };
    serialize(&mut out, &Ser { nodes, id: root }, opts).unwrap();
    String::from_utf8(out).unwrap()
}

fn tree(input: &str, fragment: Option<&str>, scripting: bool) -> (String, String) {
    let opts = ParseOpts {
        tokenizer: TokenizerOpts::default(),
        tree_builder: TreeBuilderOpts { scripting_enabled: scripting, ..TreeBuilderOpts::default() },
    };
    let mut out = String::new();
    match fragment {
        None => {
            let nodes = parse_document(Sink::new(), opts).one(input);
            for &c in &nodes[0].children {
                dump(&nodes, c, 0, &mut out);
            }
            (out, html(&nodes, 0, scripting))
        }
        Some(context) => {
            let (ns, local) = match context.split_once(' ') {
                Some(("svg", l)) => (ns!(svg), l),
                Some(("math", l)) => (ns!(mathml), l),
                Some(_) => panic!("context {context}"),
                None => (ns!(html), context),
            };
            let name = QualName::new(None, ns, LocalName::from(local));
            let nodes = parse_fragment(Sink::new(), opts, name, vec![], scripting).one(input);
            let html = nodes[0]
                .children
                .iter()
                .copied()
                .find(|&c| matches!(&nodes[c].data, Data::Element { name, .. } if name.local == local_name!("html")))
                .unwrap();
            for &c in &nodes[html].children {
                dump(&nodes, c, 0, &mut out);
            }
            (out, self::html(&nodes, html, scripting))
        }
    }
}

struct DatTest {
    data: String,
    fragment: Option<String>,
    script: Option<bool>,
    expected: String,
}

fn read_dat(text: &str) -> Vec<DatTest> {
    let mut tests = vec![];
    let mut section = String::new();
    let mut cur: Option<DatTest> = None;
    let lines: Vec<&str> = text.split('\n').collect();
    for line in lines {
        if line == "#data" {
            if let Some(t) = cur.take() {
                tests.push(t);
            }
            cur = Some(DatTest { data: String::new(), fragment: None, script: None, expected: String::new() });
            section = "data".into();
            continue;
        }
        let Some(t) = cur.as_mut() else { continue };
        match line {
            "#errors" | "#new-errors" | "#document" => {
                section = line[1..].into();
                continue;
            }
            "#document-fragment" => {
                section = "fragment".into();
                continue;
            }
            "#script-on" => {
                t.script = Some(true);
                continue;
            }
            "#script-off" => {
                t.script = Some(false);
                continue;
            }
            _ => {}
        }
        match section.as_str() {
            "data" => {
                t.data.push_str(line);
                t.data.push('\n');
            }
            "fragment" => t.fragment = Some(line.trim().to_string()),
            "document" => {
                t.expected.push_str(line);
                t.expected.push('\n');
            }
            _ => {}
        }
    }
    if let Some(t) = cur {
        tests.push(t);
    }
    for t in &mut tests {
        t.data.pop();
        while t.expected.ends_with("\n\n") {
            t.expected.pop();
        }
        if t.expected == "\n" {
            t.expected.clear();
        }
    }
    tests
}

fn tree_tests(spec: &Path) {
    let mut out = fs::File::create(spec.join("golden/tree.jsonl")).unwrap();
    let mut files: Vec<_> = fs::read_dir(spec.join("tree-construction"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "dat"))
        .collect();
    files.sort();
    let (mut count, mut agree) = (0, 0);
    for path in files {
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        for (index, t) in read_dat(&fs::read_to_string(&path).unwrap()).into_iter().enumerate() {
            let scripting = t.script.unwrap_or(false);
            let result = std::panic::catch_unwind(|| tree(&t.data, t.fragment.as_deref(), scripting));
            let Ok((output, serialized)) = result else {
                eprintln!("html5ever panicked: {file} #{index}");
                continue;
            };
            count += 1;
            if output == t.expected {
                agree += 1;
            }
            let line = json!({
                "file": file, "index": index, "input": t.data, "fragment": t.fragment,
                "scripting": scripting, "output": output, "expected": t.expected,
                "html": serialized,
            });
            writeln!(out, "{}", line).unwrap();
        }
    }
    eprintln!("tree: {count} cases, {agree} as html5lib-tests expect");
}
