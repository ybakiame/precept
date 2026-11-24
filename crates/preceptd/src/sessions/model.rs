use precept_ipc::Error;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

pub const MAX_SESSIONS: usize = 20;
pub const MAX_GOAL: usize = 500;
pub const MAX_ITEMS: usize = 36;
pub const MAX_ITEM: usize = 200;
pub const MAX_ITEM_ID: usize = 24;
pub const MAX_OWNER: usize = 64;
pub const MAX_SUMMARY: usize = 1000;
pub const IN_CONTEXT: usize = 5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    Milestone,
    #[default]
    Task,
}

impl ItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Milestone => "milestone",
            Self::Task => "task",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemState {
    #[default]
    Todo,
    Doing,
    Blocked,
    Deferred,
    Done,
}

impl ItemState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Doing => "doing",
            Self::Blocked => "blocked",
            Self::Deferred => "deferred",
            Self::Done => "done",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: String,
    pub text: String,
    pub kind: ItemKind,
    pub state: ItemState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewItem {
    #[serde(default, deserialize_with = "present")]
    pub id: Option<String>,
    pub text: String,
    #[serde(default)]
    pub kind: ItemKind,
    #[serde(default)]
    pub state: ItemState,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default, deserialize_with = "present")]
    pub owner: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub blocked_reason: Option<String>,
}

impl NewItem {
    pub fn into_item(self, id: String, default_owner: Option<String>) -> Item {
        Item {
            id,
            text: self.text.trim().to_owned(),
            kind: self.kind,
            state: self.state,
            depends_on: self.depends_on,
            owner: self
                .owner
                .map(|owner| owner.trim().to_owned())
                .filter(|owner| !owner.is_empty())
                .or(default_owner),
            blocked_reason: self.blocked_reason.map(|reason| reason.trim().to_owned()),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
pub enum Operation {
    Add {
        item: NewItem,
    },
    Update {
        id: String,
        #[serde(default, deserialize_with = "present")]
        text: Option<String>,
        #[serde(default, deserialize_with = "present")]
        kind: Option<ItemKind>,
        #[serde(default, deserialize_with = "present")]
        depends_on: Option<Vec<String>>,
        #[serde(default, deserialize_with = "present")]
        owner: Option<String>,
        #[serde(default, deserialize_with = "present")]
        blocked_reason: Option<String>,
    },
    Start {
        id: String,
    },
    Complete {
        id: String,
    },
    Defer {
        id: String,
    },
    Block {
        id: String,
        reason: String,
    },
    Reopen {
        id: String,
    },
    Remove {
        id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub id: String,
    pub agent: Option<String>,
    pub conversation: Option<String>,
    pub status: String,
    pub started_ms: u64,
    pub updated_ms: u64,
    pub closed_ms: Option<u64>,
    pub goal: String,
    pub items: Vec<Item>,
    pub summary: Option<String>,
    pub rev: u64,
}

pub(super) fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    #[serde(default, deserialize_with = "present")]
    pub id: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub conversation: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub goal: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub items: Option<Vec<NewItem>>,
    #[serde(default, deserialize_with = "present")]
    pub operations: Option<Vec<Operation>>,
    #[serde(default, deserialize_with = "present")]
    pub summary: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub expected_rev: Option<u64>,
    #[serde(default, deserialize_with = "present")]
    pub client: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyItem {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacySession {
    pub id: String,
    pub agent: Option<String>,
    pub conversation: Option<String>,
    pub status: String,
    pub started_ms: u64,
    pub updated_ms: u64,
    pub closed_ms: Option<u64>,
    pub goal: String,
    pub done: Vec<LegacyItem>,
    pub doing: Vec<LegacyItem>,
    pub todo: Vec<LegacyItem>,
    pub summary: Option<String>,
    #[serde(default)]
    pub rev: u64,
}

impl LegacySession {
    pub fn into_session(self) -> Session {
        let mut items = Vec::new();
        let mut used = std::collections::HashSet::new();
        convert_legacy(self.doing, ItemState::Doing, "doing", &mut items, &mut used);
        convert_legacy(self.todo, ItemState::Todo, "todo", &mut items, &mut used);
        convert_legacy(self.done, ItemState::Done, "done", &mut items, &mut used);
        let mut session = Session {
            id: self.id,
            agent: self.agent,
            conversation: self.conversation,
            status: self.status,
            started_ms: self.started_ms,
            updated_ms: self.updated_ms,
            closed_ms: self.closed_ms,
            goal: self.goal,
            items,
            summary: self.summary,
            rev: self.rev,
        };
        normalize_legacy_dependencies(&mut session.items);
        session
    }
}

fn convert_legacy(
    list: Vec<LegacyItem>,
    state: ItemState,
    suffix: &str,
    out: &mut Vec<Item>,
    used: &mut std::collections::HashSet<String>,
) {
    let mut ids = HashMap::new();
    for item in &list {
        let id = unique_legacy_id(&item.id, suffix, used);
        used.insert(id.clone());
        ids.insert(item.id.clone(), id);
    }
    for item in list {
        // Format 1 allowed starting before same-list dependencies completed;
        // defer those items instead of making the whole file unreadable.
        let state = if state == ItemState::Doing && !item.depends_on.is_empty() {
            ItemState::Todo
        } else {
            state
        };
        out.push(Item {
            id: ids[&item.id].clone(),
            text: item.text,
            kind: ItemKind::Task,
            state,
            depends_on: item
                .depends_on
                .iter()
                .map(|dep| ids.get(dep).cloned().unwrap_or_else(|| dep.clone()))
                .collect(),
            owner: None,
            blocked_reason: None,
        });
    }
}

fn normalize_legacy_dependencies(items: &mut [Item]) {
    let known: std::collections::HashSet<String> =
        items.iter().map(|item| item.id.clone()).collect();
    for item in items.iter_mut() {
        let mut seen = std::collections::HashSet::new();
        item.depends_on
            .retain(|dep| dep != &item.id && known.contains(dep) && seen.insert(dep.clone()));
    }
    while let Some((from, dependency)) = legacy_cycle_edge(items) {
        items[from].depends_on.remove(dependency);
    }
}

fn legacy_cycle_edge(items: &[Item]) -> Option<(usize, usize)> {
    let by_id: HashMap<String, usize> = items
        .iter()
        .enumerate()
        .map(|(index, item)| (item.id.clone(), index))
        .collect();
    fn visit(
        index: usize,
        items: &[Item],
        by_id: &HashMap<String, usize>,
        colors: &mut [u8],
    ) -> Option<(usize, usize)> {
        colors[index] = 1;
        for (position, dep) in items[index].depends_on.iter().enumerate() {
            let next = by_id[dep];
            match colors[next] {
                1 => return Some((index, position)),
                0 => {
                    if let Some(edge) = visit(next, items, by_id, colors) {
                        return Some(edge);
                    }
                }
                _ => {}
            }
        }
        colors[index] = 2;
        None
    }
    let mut colors = vec![0_u8; items.len()];
    for index in 0..items.len() {
        if colors[index] == 0
            && let Some(edge) = visit(index, items, &by_id, &mut colors)
        {
            return Some(edge);
        }
    }
    None
}

fn unique_legacy_id(base: &str, suffix: &str, used: &std::collections::HashSet<String>) -> String {
    if !used.contains(base) {
        return base.to_owned();
    }
    let stem = base.trim_end_matches('-');
    let room = MAX_ITEM_ID.saturating_sub(suffix.len() + 1);
    let stem: String = stem.chars().take(room).collect();
    let candidate = format!("{stem}-{suffix}");
    if !used.contains(&candidate) {
        return candidate;
    }
    for n in 2..=99 {
        let tail = format!("-{suffix}-{n}");
        let room = MAX_ITEM_ID.saturating_sub(tail.len());
        let stem: String = stem.chars().take(room).collect();
        let candidate = format!("{stem}{tail}");
        if !used.contains(&candidate) {
            return candidate;
        }
    }
    base.to_owned()
}

pub fn invalid(field: impl Into<String>, reason: impl Into<String>) -> Error {
    Error::InvalidWorkState {
        field: field.into(),
        reason: reason.into(),
    }
}

pub fn text(raw: &str, max: usize, field: &str) -> Result<Option<String>, Error> {
    let trimmed = raw.trim();
    if trimmed.chars().count() > max {
        return Err(invalid(field, format!("at most {max} characters")));
    }
    if trimmed.chars().any(|c| c.is_control() && c != '\n') {
        return Err(invalid(field, "control characters are not allowed"));
    }
    Ok((!trimmed.is_empty()).then(|| trimmed.to_owned()))
}

pub fn required_text(raw: &str, max: usize, field: &str) -> Result<String, Error> {
    text(raw, max, field)?.ok_or_else(|| invalid(field, "required"))
}

pub fn owner(raw: &str, field: &str) -> Result<Option<String>, Error> {
    let trimmed = raw.trim();
    if trimmed.chars().count() > MAX_OWNER {
        return Err(invalid(field, format!("at most {MAX_OWNER} characters")));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(invalid(field, "control characters are not allowed"));
    }
    Ok((!trimmed.is_empty()).then(|| trimmed.to_owned()))
}

pub fn valid_item_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ITEM_ID
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub fn validate_items(items: &[Item]) -> Result<(), Error> {
    if items.len() > MAX_ITEMS {
        return Err(invalid("items", format!("at most {MAX_ITEMS} items")));
    }
    let mut by_id = HashMap::new();
    for (index, item) in items.iter().enumerate() {
        let field = format!("items[{index}]");
        if !valid_item_id(&item.id) {
            return Err(invalid(
                format!("{field}.id"),
                "use at most 24 lowercase letters, digits or hyphens",
            ));
        }
        if by_id.insert(item.id.as_str(), index).is_some() {
            return Err(invalid(format!("{field}.id"), "duplicate id"));
        }
        required_text(&item.text, MAX_ITEM, &format!("{field}.text"))?;
        if item.depends_on.len() > MAX_ITEMS {
            return Err(invalid(
                format!("{field}.depends_on"),
                format!("at most {MAX_ITEMS} dependencies"),
            ));
        }
        let mut seen = std::collections::HashSet::new();
        for dep in &item.depends_on {
            if !valid_item_id(dep) || dep == &item.id {
                return Err(invalid(
                    format!("{field}.depends_on"),
                    "dependency ids must be valid and cannot name the item itself",
                ));
            }
            if !seen.insert(dep.as_str()) {
                return Err(invalid(
                    format!("{field}.depends_on"),
                    "duplicate dependency",
                ));
            }
        }
        if let Some(value) = &item.owner {
            owner(value, &format!("{field}.owner"))?;
        }
        match (&item.state, &item.blocked_reason) {
            (ItemState::Blocked, None) => {
                return Err(invalid(
                    format!("{field}.blocked_reason"),
                    "required when state is blocked",
                ));
            }
            (ItemState::Blocked, Some(reason)) => {
                required_text(reason, MAX_ITEM, &format!("{field}.blocked_reason"))?;
            }
            (_, Some(_)) => {
                return Err(invalid(
                    format!("{field}.blocked_reason"),
                    "only allowed when state is blocked",
                ));
            }
            _ => {}
        }
    }
    for (index, item) in items.iter().enumerate() {
        for dep in &item.depends_on {
            if !by_id.contains_key(dep.as_str()) {
                return Err(invalid(
                    format!("items[{index}].depends_on"),
                    "dependency must name an item in this session",
                ));
            }
        }
        if matches!(item.state, ItemState::Doing | ItemState::Done)
            && item
                .depends_on
                .iter()
                .any(|dep| items[by_id[dep.as_str()]].state != ItemState::Done)
        {
            return Err(invalid(
                format!("items[{index}].depends_on"),
                "dependencies must be done before an item starts or completes",
            ));
        }
    }
    let mut colors = vec![0_u8; items.len()];
    for index in 0..items.len() {
        if cycle(index, items, &by_id, &mut colors) {
            return Err(invalid(
                format!("items[{index}].depends_on"),
                "dependency cycle",
            ));
        }
    }
    Ok(())
}

fn cycle(index: usize, items: &[Item], by_id: &HashMap<&str, usize>, colors: &mut [u8]) -> bool {
    if colors[index] == 1 {
        return true;
    }
    if colors[index] == 2 {
        return false;
    }
    colors[index] = 1;
    for dep in &items[index].depends_on {
        let next = by_id[dep.as_str()];
        if cycle(next, items, by_id, colors) {
            return true;
        }
    }
    colors[index] = 2;
    false
}
