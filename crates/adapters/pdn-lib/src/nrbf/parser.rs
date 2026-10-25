use super::*;

pub(super) struct Parser<'a, 'c> {
    bytes: &'a [u8],
    position: usize,
    limits: GraphLimits,
    metadata: usize,
    records: usize,
    check: &'c dyn Fn() -> Result<()>,
    graph: Graph<'a>,
    libraries: Vec<(i32, &'a str)>,
}
#[derive(Debug)]
enum Record {
    Value(Value),
    NullRun(usize),
    End,
}
impl<'a, 'c> Parser<'a, 'c> {
    pub fn new(bytes: &'a [u8], limits: GraphLimits, check: &'c dyn Fn() -> Result<()>) -> Self {
        Self {
            bytes,
            position: 0,
            limits,
            metadata: 0,
            records: 0,
            check,
            graph: Graph {
                root: 0,
                consumed: 0,
                nodes: Vec::new(),
                schemas: Vec::new(),
            },
            libraries: Vec::new(),
        }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(n)
            .ok_or_else(|| resource("PDN graph offset overflow"))?;
        if end > self.limits.bytes {
            return Err(resource("PDN graph exceeds byte limit"));
        }
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| malformed("Truncated PDN graph field"))?;
        self.position = end;
        Ok(bytes)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn id(&mut self) -> Result<i32> {
        let id = self.i32()?;
        if id == 0 {
            Err(malformed("Zero PDN graph identifier"))
        } else {
            Ok(id)
        }
    }
    fn count(&mut self, max: usize) -> Result<usize> {
        let n = self.i32()?;
        let n = usize::try_from(n).map_err(|_| malformed("Negative PDN graph count"))?;
        if n > max {
            Err(resource("PDN graph count exceeds limits"))
        } else {
            Ok(n)
        }
    }
    fn text(&mut self) -> Result<&'a str> {
        let mut length = 0usize;
        for shift in [0, 7, 14, 21, 28] {
            let next = self.u8()?;
            if shift == 28 && next > 7 {
                return Err(malformed("Invalid PDN string length"));
            }
            length |= usize::from(next & 0x7f) << shift;
            if next & 0x80 == 0 {
                if length > self.limits.bytes {
                    return Err(resource("PDN string exceeds graph limit"));
                }
                return std::str::from_utf8(self.take(length)?)
                    .map_err(|_| malformed("PDN string is not UTF-8"));
            }
        }
        Err(malformed("Invalid PDN string length"))
    }
    fn charge<T>(&mut self, n: usize) -> Result<()> {
        self.metadata = self
            .metadata
            .checked_add(
                n.checked_mul(std::mem::size_of::<T>())
                    .ok_or_else(|| resource("PDN metadata size overflow"))?,
            )
            .ok_or_else(|| resource("PDN metadata size overflow"))?;
        if self.metadata > self.limits.metadata {
            return Err(resource("PDN graph metadata exceeds limits"));
        }
        Ok(())
    }
    fn vector<T>(&mut self, n: usize) -> Result<Vec<T>> {
        self.charge::<T>(n)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(n)
            .map_err(|_| resource("PDN metadata allocation failed"))?;
        Ok(values)
    }
    fn primitive(&mut self, kind: u8) -> Result<Value> {
        Ok(match kind {
            1 => match self.u8()? {
                0 => Value::Bool(false),
                1 => Value::Bool(true),
                _ => return Err(malformed("Invalid PDN boolean")),
            },
            2 => Value::UInt(u64::from(self.u8()?)),
            6 => Value::Bits(u64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            7 => Value::Int(i64::from(i16::from_le_bytes(
                self.take(2)?.try_into().unwrap(),
            ))),
            8 => Value::Int(i64::from(self.i32()?)),
            9 | 12 => Value::Int(i64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            10 => Value::Int(i64::from(self.u8()? as i8)),
            11 => Value::Bits(u64::from(u32::from_le_bytes(
                self.take(4)?.try_into().unwrap(),
            ))),
            13 | 16 => Value::UInt(u64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            14 => Value::UInt(u64::from(u16::from_le_bytes(
                self.take(2)?.try_into().unwrap(),
            ))),
            15 => Value::UInt(u64::from(u32::from_le_bytes(
                self.take(4)?.try_into().unwrap(),
            ))),
            _ => return Err(unsupported("Unsupported PDN primitive type")),
        })
    }
    fn binding(&mut self, kind: u8) -> Result<Binding<'a>> {
        let mut binding = Binding {
            kind,
            primitive: 0,
            class: None,
            library: None,
        };
        match kind {
            0 | 7 => binding.primitive = self.u8()?,
            1 | 2 | 5 | 6 => {}
            3 | 4 => {
                let name = self.text()?;
                if !allowed(name) && !name.strip_suffix("[]").is_some_and(allowed) {
                    return Err(unsupported("Unqualified PDN member type"));
                }
                binding.class = Some(name);
                if kind == 4 {
                    binding.library = Some(self.id()?);
                }
            }
            _ => return Err(unsupported("Unsupported PDN binary member type")),
        }
        Ok(binding)
    }
    fn node(&mut self, id: i32, offset: usize, kind: NodeKind<'a>) -> Result<Value> {
        if self.graph.nodes.iter().any(|node| node.id == id) {
            return Err(malformed("Duplicate PDN graph object"));
        }
        if self.graph.nodes.len() >= self.limits.objects {
            return Err(resource("PDN object count exceeds limits"));
        }
        // The pool was fully reserved once, so untrusted records cannot trigger
        // infallible growth or unaccounted schema/field vectors.
        self.graph.nodes.push(Node { id, offset, kind });
        Ok(Value::Ref(id))
    }
    fn values(&mut self, binding: Binding<'a>, count: usize, depth: usize) -> Result<Vec<Value>> {
        let mut values = self.vector(count)?;
        while values.len() < count {
            (self.check)()?;
            if binding.kind == 0 {
                values.push(self.primitive(binding.primitive)?);
                continue;
            }
            match self.record(depth + 1)? {
                Record::Value(value) => values.push(value),
                Record::NullRun(n) => {
                    if n == 0 || n > count - values.len() {
                        return Err(malformed("PDN null run exceeds array bounds"));
                    }
                    values.resize(values.len() + n, Value::Null);
                }
                Record::End => return Err(malformed("PDN graph ended inside an array")),
            }
        }
        Ok(values)
    }
    fn class(&mut self, record: u8, offset: usize, depth: usize) -> Result<Value> {
        let id = self.id()?;
        let schema = if record == 1 {
            let metadata = self.id()?;
            self.graph
                .schemas
                .iter()
                .position(|schema| schema.id == metadata)
                .ok_or_else(|| malformed("Unresolved PDN class metadata"))?
        } else {
            let name = self.text()?;
            if !allowed(name) {
                return Err(unsupported(
                    "Unqualified PDN class; no active types are instantiated",
                ));
            }
            let count = self.count(self.limits.members)?;
            let mut names = self.vector(count)?;
            for _ in 0..count {
                let name = self.text()?;
                if names.contains(&name) {
                    return Err(malformed("Duplicate PDN class member"));
                }
                names.push(name);
            }
            let kinds = self.take(count)?;
            let mut members = self.vector(count)?;
            for (&kind, name) in kinds.iter().zip(names) {
                members.push(Member {
                    name,
                    binding: self.binding(kind)?,
                });
            }
            let library = if record == 5 { Some(self.id()?) } else { None };
            if self.graph.schemas.len() >= self.limits.objects {
                return Err(resource("PDN class metadata count exceeds limits"));
            }
            if self.graph.schemas.iter().any(|s| s.id == id) {
                return Err(malformed("Duplicate PDN class metadata"));
            }
            self.graph.schemas.push(Schema {
                id,
                name,
                library,
                members,
            });
            self.graph.schemas.len() - 1
        };
        let count = self.graph.schemas[schema].members.len();
        let mut values = self.vector(count)?;
        for index in 0..count {
            (self.check)()?;
            let binding = self.graph.schemas[schema].members[index].binding;
            let value = if binding.kind == 0 {
                self.primitive(binding.primitive)?
            } else {
                match self.record(depth + 1)? {
                    Record::Value(value) => value,
                    _ => return Err(malformed("Invalid PDN class member record")),
                }
            };
            values.push(value);
        }
        self.node(id, offset, NodeKind::Object { schema, values })
    }
    fn record(&mut self, depth: usize) -> Result<Record> {
        if depth > self.limits.depth.min(64) {
            return Err(resource("PDN object depth exceeds limits"));
        }
        loop {
            (self.check)()?;
            self.records = self
                .records
                .checked_add(1)
                .ok_or_else(|| resource("PDN record count overflow"))?;
            if self.records > self.limits.objects.saturating_mul(32) {
                return Err(resource("PDN record work exceeds limits"));
            }
            let offset = self.position;
            let record = self.u8()?;
            let value = match record {
                1 | 4 | 5 => self.class(record, offset, depth)?,
                6 => {
                    let id = self.id()?;
                    let text = self.text()?;
                    self.node(id, offset, NodeKind::String(text))?
                }
                7 | 15 | 16 | 17 => {
                    let id = self.id()?;
                    let (binding, count) = if record == 7 {
                        if self.u8()? != 0 || self.i32()? != 1 {
                            return Err(unsupported(
                                "Only one-dimensional zero-based PDN arrays are supported",
                            ));
                        }
                        let count = self.count(self.limits.array_items)?;
                        let kind = self.u8()?;
                        (self.binding(kind)?, count)
                    } else {
                        let count = self.count(self.limits.array_items)?;
                        let binding = if record == 15 {
                            self.binding(0)?
                        } else {
                            Binding {
                                kind: if record == 16 { 2 } else { 1 },
                                primitive: 0,
                                class: None,
                                library: None,
                            }
                        };
                        (binding, count)
                    };
                    let values = self.values(binding, count, depth)?;
                    self.node(id, offset, NodeKind::Array { binding, values })?
                }
                8 => {
                    let kind = self.u8()?;
                    self.primitive(kind)?
                }
                9 => Value::Ref(self.id()?),
                10 => Value::Null,
                11 => return Ok(Record::End),
                12 => {
                    let id = self.id()?;
                    let name = self.text()?;
                    if ![
                        "PaintDotNet.Data, Version=",
                        "PaintDotNet.Core, Version=",
                        "mscorlib, Version=",
                    ]
                    .iter()
                    .any(|prefix| name.starts_with(prefix))
                    {
                        return Err(unsupported("Unqualified PDN library identifier"));
                    }
                    if self.libraries.iter().any(|(existing, _)| *existing == id)
                        || self.libraries.len() >= self.limits.objects
                    {
                        return Err(malformed("Duplicate or excessive PDN library identifier"));
                    }
                    self.libraries.push((id, name));
                    continue;
                }
                13 => return Ok(Record::NullRun(usize::from(self.u8()?))),
                14 => return Ok(Record::NullRun(self.count(self.limits.array_items)?)),
                _ => {
                    return Err(unsupported(
                        "Unsupported NRBF record; type loading and method calls are disabled",
                    ));
                }
            };
            return Ok(Record::Value(value));
        }
    }
    fn validate_value(&self, value: Value, binding: Binding<'a>) -> Result<()> {
        if value == Value::Null {
            return Ok(());
        }
        let node = if let Value::Ref(id) = value {
            Some(self.graph.node(id)?)
        } else {
            None
        };
        let valid = match binding.kind {
            0 => node.is_none(),
            1 => node.is_some_and(|node| matches!(node.kind, NodeKind::String(_))),
            2 => true,
            3 | 4 => node.is_some_and(|node| {
                if let Some(base)=binding.class.and_then(|name|name.strip_suffix("[]")) {
                    matches!(&node.kind,NodeKind::Array{binding:element,..} if element.class==Some(base) && (binding.library.is_none() || binding.library==element.library))
                } else if let NodeKind::Object{schema,..}=&node.kind {
                    self.graph.class(node)==binding.class && (binding.library.is_none() || self.graph.schemas[*schema].library==binding.library)
                } else {false}
            }),
            5 => node.is_some_and(|node|matches!(&node.kind,NodeKind::Array{binding:element,..} if element.kind==2)),
            6 => node.is_some_and(|node|matches!(&node.kind,NodeKind::Array{binding:element,..} if element.kind==1)),
            7 => node.is_some_and(|node|matches!(&node.kind,NodeKind::Array{binding:element,..} if element.kind==0 && element.primitive==binding.primitive)),
            _ => false,
        };
        if !valid {
            return Err(malformed(
                "PDN member reference disagrees with its declared type",
            ));
        }
        Ok(())
    }
    pub fn parse(mut self) -> Result<Graph<'a>> {
        if self.u8()? != 0 {
            return Err(malformed(
                "PDN graph does not begin with a serialization header",
            ));
        }
        self.graph.root = self.id()?;
        if self.graph.root < 1 {
            return Err(malformed("PDN root identifier must be positive"));
        }
        if self.i32()? != -1 {
            return Err(unsupported("PDN remoting headers are unsupported"));
        }
        if self.i32()? != 1 || self.i32()? != 0 {
            return Err(unsupported("Unsupported PDN NRBF version"));
        }
        // Bound all pools before reading records. These conservative reservations
        // are included in the graph's declared metadata allowance.
        self.graph.nodes = self.vector(self.limits.objects)?;
        self.graph.schemas = self.vector(self.limits.objects)?;
        self.libraries = self.vector(self.limits.objects)?;
        loop {
            match self.record(0)? {
                Record::End => break,
                Record::Value(Value::Ref(_)) => {}
                _ => return Err(malformed("Invalid standalone PDN graph value")),
            }
        }
        self.graph.consumed = self.position;
        (self.check)()?;
        self.graph.nodes.sort_unstable_by_key(|node| node.id);
        self.libraries.sort_unstable_by_key(|(id, _)| *id);
        (self.check)()?;
        self.graph.node(self.graph.root)?;
        for schema in &self.graph.schemas {
            (self.check)()?;
            for library in schema
                .library
                .into_iter()
                .chain(schema.members.iter().filter_map(|m| m.binding.library))
            {
                if self
                    .libraries
                    .binary_search_by_key(&library, |(id, _)| *id)
                    .is_err()
                {
                    return Err(malformed("Unresolved PDN library identifier"));
                }
            }
        }
        for node in &self.graph.nodes {
            (self.check)()?;
            match &node.kind {
                NodeKind::Object { schema, values } => {
                    for (member, value) in self.graph.schemas[*schema].members.iter().zip(values) {
                        (self.check)()?;
                        self.validate_value(*value, member.binding)?;
                    }
                }
                NodeKind::Array { binding, values } => {
                    for value in values {
                        (self.check)()?;
                        self.validate_value(*value, *binding)?;
                    }
                }
                NodeKind::String(_) => {}
            }
        }
        Ok(self.graph)
    }
}
