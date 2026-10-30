use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use super::error::{Result, VaultError};

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// Idle minutes before auto-lock; `None` = never.
    pub idle_minutes: Option<u32>,
    pub lock_on_minimize: bool,
    /// "system" | "light" | "dark"
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { idle_minutes: Some(5), lock_on_minimize: false, theme: "system".into() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Page {
    pub id: String,
    pub title: String,
    pub modified: i64,
    /// TipTap / ProseMirror JSON document.
    pub content: Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Section {
    pub id: String,
    pub name: String,
    pub pages: Vec<Page>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Notebook {
    pub id: String,
    pub name: String,
    pub sections: Vec<Section>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct VaultData {
    pub version: u32,
    pub notebooks: Vec<Notebook>,
    pub settings: Settings,
}

impl Default for VaultData {
    fn default() -> Self {
        VaultData { version: 1, notebooks: vec![], settings: Settings::default() }
    }
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Notebook,
    Section,
    Page,
}

fn reorder_vec<T>(v: &mut Vec<T>, ids: &[String], id_of: impl Fn(&T) -> &str) {
    let mut old = std::mem::take(v);
    let mut out = Vec::with_capacity(old.len());
    for id in ids {
        if let Some(pos) = old.iter().position(|x| id_of(x) == id) {
            out.push(old.remove(pos));
        }
    }
    out.extend(old);
    *v = out;
}

pub fn empty_doc() -> Value {
    json!({ "type": "doc", "content": [{ "type": "paragraph" }] })
}

impl VaultData {
    fn notebook_mut(&mut self, id: &str) -> Result<&mut Notebook> {
        self.notebooks.iter_mut().find(|n| n.id == id).ok_or(VaultError::NotFound)
    }

    fn section_mut(&mut self, id: &str) -> Result<&mut Section> {
        self.notebooks
            .iter_mut()
            .flat_map(|n| n.sections.iter_mut())
            .find(|s| s.id == id)
            .ok_or(VaultError::NotFound)
    }

    pub fn page_mut(&mut self, id: &str) -> Result<&mut Page> {
        self.notebooks
            .iter_mut()
            .flat_map(|n| n.sections.iter_mut())
            .flat_map(|s| s.pages.iter_mut())
            .find(|p| p.id == id)
            .ok_or(VaultError::NotFound)
    }

    pub fn add_notebook(&mut self, name: &str) -> String {
        let id = new_id();
        self.notebooks.push(Notebook { id: id.clone(), name: name.into(), sections: vec![] });
        id
    }

    pub fn add_section(&mut self, notebook: &str, name: &str) -> Result<String> {
        let id = new_id();
        self.notebook_mut(notebook)?.sections.push(Section {
            id: id.clone(),
            name: name.into(),
            pages: vec![],
        });
        Ok(id)
    }

    pub fn add_page(&mut self, section: &str, title: &str) -> Result<String> {
        let id = new_id();
        self.section_mut(section)?.pages.push(Page {
            id: id.clone(),
            title: title.into(),
            modified: now_ms(),
            content: empty_doc(),
        });
        Ok(id)
    }

    pub fn rename(&mut self, kind: Kind, id: &str, name: &str) -> Result<()> {
        match kind {
            Kind::Notebook => self.notebook_mut(id)?.name = name.into(),
            Kind::Section => self.section_mut(id)?.name = name.into(),
            Kind::Page => {
                let p = self.page_mut(id)?;
                p.title = name.into();
                p.modified = now_ms();
            }
        }
        Ok(())
    }

    pub fn delete(&mut self, kind: Kind, id: &str) -> Result<()> {
        let found = match kind {
            Kind::Notebook => remove_where(&mut self.notebooks, |n| n.id == id),
            Kind::Section => self.notebooks.iter_mut().any(|n| remove_where(&mut n.sections, |s| s.id == id)),
            Kind::Page => self
                .notebooks
                .iter_mut()
                .flat_map(|n| n.sections.iter_mut())
                .any(|s| remove_where(&mut s.pages, |p| p.id == id)),
        };
        if found {
            Ok(())
        } else {
            Err(VaultError::NotFound)
        }
    }

    /// `parent` is ignored for notebooks, a notebook id for sections, a section id for pages.
    pub fn reorder(&mut self, kind: Kind, parent: &str, ids: &[String]) -> Result<()> {
        match kind {
            Kind::Notebook => reorder_vec(&mut self.notebooks, ids, |n| &n.id),
            Kind::Section => reorder_vec(&mut self.notebook_mut(parent)?.sections, ids, |s| &s.id),
            Kind::Page => reorder_vec(&mut self.section_mut(parent)?.pages, ids, |p| &p.id),
        }
        Ok(())
    }

    pub fn move_page(&mut self, page: &str, to_section: &str) -> Result<()> {
        self.section_mut(to_section)?; // target must exist before we remove anything
        let mut taken = None;
        for s in self.notebooks.iter_mut().flat_map(|n| n.sections.iter_mut()) {
            if let Some(pos) = s.pages.iter().position(|p| p.id == page) {
                if s.id == to_section {
                    return Ok(());
                }
                taken = Some(s.pages.remove(pos));
                break;
            }
        }
        let p = taken.ok_or(VaultError::NotFound)?;
        self.section_mut(to_section)?.pages.push(p);
        Ok(())
    }
}

fn remove_where<T>(v: &mut Vec<T>, f: impl Fn(&T) -> bool) -> bool {
    match v.iter().position(f) {
        Some(i) => {
            v.remove(i);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_operations() {
        let mut d = VaultData::default();
        let nb = d.add_notebook("Work");
        let s1 = d.add_section(&nb, "A").unwrap();
        let s2 = d.add_section(&nb, "B").unwrap();
        let p1 = d.add_page(&s1, "one").unwrap();
        let p2 = d.add_page(&s1, "two").unwrap();

        d.reorder(Kind::Page, &s1, &[p2.clone(), p1.clone()]).unwrap();
        assert_eq!(d.notebooks[0].sections[0].pages[0].id, p2);

        d.move_page(&p1, &s2).unwrap();
        assert_eq!(d.notebooks[0].sections[0].pages.len(), 1);
        assert_eq!(d.notebooks[0].sections[1].pages[0].id, p1);

        d.rename(Kind::Section, &s1, "Renamed").unwrap();
        assert_eq!(d.notebooks[0].sections[0].name, "Renamed");

        d.delete(Kind::Section, &s2).unwrap();
        assert_eq!(d.notebooks[0].sections.len(), 1);
        assert!(d.delete(Kind::Page, "nope").is_err());
    }
}
