//! Een kleine XML-lezer: elementen, attributen en tekst. Genoeg voor de onderdelen van een .mcdx.

#[derive(Debug, Clone, PartialEq)]
pub enum Child {
    Elem(Node),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    /// De naam zoals in het bestand, met voorvoegsel (`ml:apply`).
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Child>,
}

impl Node {
    /// De naam zonder voorvoegsel (`apply`).
    pub fn local(&self) -> &str {
        self.name.rsplit(':').next().unwrap_or(&self.name)
    }

    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// De kindelementen (zonder tekst).
    pub fn elems(&self) -> impl Iterator<Item = &Node> {
        self.children.iter().filter_map(|c| match c {
            Child::Elem(n) => Some(n),
            Child::Text(_) => None,
        })
    }

    /// Het eerste kindelement met deze naam (zonder voorvoegsel).
    pub fn child(&self, local: &str) -> Option<&Node> {
        self.elems().find(|n| n.local() == local)
    }

    /// Alle nakomelingen met deze naam, in documentvolgorde.
    pub fn descendants<'a>(&'a self, local: &str, out: &mut Vec<&'a Node>) {
        for n in self.elems() {
            if n.local() == local {
                out.push(n);
            }
            n.descendants(local, out);
        }
    }

    /// Alle tekst in dit element en daaronder.
    pub fn text(&self) -> String {
        let mut s = String::new();
        for c in &self.children {
            match c {
                Child::Text(t) => s.push_str(t),
                Child::Elem(n) => s.push_str(&n.text()),
            }
        }
        s
    }
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(k) = rest.find('&') {
        out.push_str(&rest[..k]);
        rest = &rest[k..];
        let Some(end) = rest.find(';') else { break };
        let ent = &rest[1..end];
        let ch = match ent {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if ent.starts_with("#x") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

struct P<'a> {
    s: &'a str,
    i: usize,
}

impl P<'_> {
    fn err(&self, msg: &str) -> String {
        format!("XML: {msg} (positie {})", self.i)
    }

    fn rest(&self) -> &str {
        &self.s[self.i..]
    }

    fn skip_ws(&mut self) {
        while self.rest().starts_with(|c: char| c.is_whitespace()) {
            self.i += self.rest().chars().next().map_or(1, char::len_utf8);
        }
    }

    /// Commentaar, `<?...?>` en `<!DOCTYPE ...>` overslaan.
    fn skip_misc(&mut self) -> Result<bool, String> {
        let r = self.rest();
        let end = if r.starts_with("<!--") {
            r.find("-->").map(|k| k + 3)
        } else if r.starts_with("<?") {
            r.find("?>").map(|k| k + 2)
        } else if r.starts_with("<!") && !r.starts_with("<![CDATA[") {
            r.find('>').map(|k| k + 1)
        } else {
            return Ok(false);
        };
        self.i += end.ok_or_else(|| self.err("niet afgesloten"))?;
        Ok(true)
    }

    fn name(&mut self) -> String {
        let r = self.rest();
        let end = r.find(|c: char| c.is_whitespace() || matches!(c, '>' | '/' | '=')).unwrap_or(r.len());
        let name = r[..end].to_string();
        self.i += end;
        name
    }

    fn element(&mut self) -> Result<Node, String> {
        // na '<'
        self.i += 1;
        let mut node = Node { name: self.name(), ..Default::default() };
        loop {
            self.skip_ws();
            let r = self.rest();
            if r.starts_with("/>") {
                self.i += 2;
                return Ok(node);
            }
            if r.starts_with('>') {
                self.i += 1;
                break;
            }
            let key = self.name();
            if key.is_empty() {
                return Err(self.err("ongeldig attribuut"));
            }
            self.skip_ws();
            if !self.rest().starts_with('=') {
                return Err(self.err("verwacht '='"));
            }
            self.i += 1;
            self.skip_ws();
            let q = self.rest().chars().next().ok_or_else(|| self.err("einde"))?;
            if q != '"' && q != '\'' {
                return Err(self.err("verwacht een aanhalingsteken"));
            }
            self.i += 1;
            let end = self.rest().find(q).ok_or_else(|| self.err("attribuut niet afgesloten"))?;
            let val = unescape(&self.rest()[..end]);
            self.i += end + 1;
            node.attrs.push((key, val));
        }
        // inhoud
        loop {
            if self.i >= self.s.len() {
                return Err(self.err(&format!("<{}> niet afgesloten", node.name)));
            }
            if self.skip_misc()? {
                continue;
            }
            let r = self.rest();
            if let Some(cdata) = r.strip_prefix("<![CDATA[") {
                let end = cdata.find("]]>").ok_or_else(|| self.err("CDATA niet afgesloten"))?;
                node.children.push(Child::Text(cdata[..end].to_string()));
                self.i += 9 + end + 3;
            } else if r.starts_with("</") {
                let end = r.find('>').ok_or_else(|| self.err("eindtag niet afgesloten"))?;
                self.i += end + 1;
                return Ok(node);
            } else if r.starts_with('<') {
                let child = self.element()?;
                node.children.push(Child::Elem(child));
            } else {
                let end = r.find('<').unwrap_or(r.len());
                node.children.push(Child::Text(unescape(&r[..end])));
                self.i += end;
            }
        }
    }
}

/// Lees een XML-document; geeft het hoofdelement.
pub fn parse(s: &str) -> Result<Node, String> {
    let s = s.strip_prefix('\u{feff}').unwrap_or(s);
    let mut p = P { s, i: 0 };
    loop {
        p.skip_ws();
        if !p.skip_misc()? {
            break;
        }
    }
    if !p.rest().starts_with('<') {
        return Err(p.err("geen XML"));
    }
    p.element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elementen_attributen_en_tekst() {
        let n = parse("<?xml version=\"1.0\"?><a x=\"1 &amp; 2\"><!-- c --><b>tekst &lt;3&#x3B1;</b><c/></a>").unwrap();
        assert_eq!(n.attr("x"), Some("1 & 2"));
        assert_eq!(n.child("b").unwrap().text(), "tekst <3α");
        assert_eq!(n.elems().count(), 2);
    }
}
