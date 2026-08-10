//! Wiki odkazy `[[cíl#sekce|popisek]]` a embedy `![[cíl]]`.

use crate::text::{lines, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    /// Cílem je poznámka (`.txt`), přípona se v odkazu nepíše.
    Note,
    /// Cílem je příloha — u těch se přípona uvádí (`[[attachments/manual.pdf]]`).
    Attachment,
}

/// Kotva uvnitř cílové poznámky.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Anchor {
    /// `[[Docker#Instalace]]` — nadpis sekce, dohledává se case-insensitive.
    Section(String),
    /// `[[Docker#^docker-install]]` — stabilní ID sekce/bloku.
    BlockId(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiLink {
    /// Celý odkaz včetně `[[`, `]]` a případného `!` u embedu.
    pub span: Span,
    /// Obsah mezi `[[` a `]]` v původním zápisu.
    pub raw: String,
    /// Cíl bez kotvy a bez popisku. U poznámek bez přípony `.txt`.
    /// Prázdný řetězec u odkazu do vlastní poznámky (`[[#Instalace]]`).
    pub target: String,
    pub anchor: Option<Anchor>,
    /// Text zobrazený uživateli, pokud byl uveden za `|`.
    pub label: Option<String>,
    /// `![[...]]`
    pub embed: bool,
    pub kind: LinkKind,
    pub line: usize,
}

impl WikiLink {
    /// Text, který se má zobrazit v editoru / preview.
    pub fn display_text(&self) -> String {
        if let Some(label) = &self.label {
            return label.clone();
        }
        match (&self.target, &self.anchor) {
            (t, None) => t.clone(),
            (t, Some(Anchor::Section(s))) if t.is_empty() => s.clone(),
            (t, Some(Anchor::Section(s))) => format!("{t} > {s}"),
            (t, Some(Anchor::BlockId(id))) if t.is_empty() => format!("^{id}"),
            (t, Some(Anchor::BlockId(_))) => t.clone(),
        }
    }

    /// Odkaz do vlastní poznámky, tj. jen kotva.
    pub fn is_internal(&self) -> bool {
        self.target.is_empty() && self.anchor.is_some()
    }

    /// Cesta k souboru, na který odkaz míří, relativně ke kořeni workspace,
    /// pokud je cíl zapsaný jako cesta. U holého názvu (`[[Docker]]`) vrací
    /// `None` — ten musí projít resolverem.
    pub fn explicit_path(&self) -> Option<String> {
        if self.target.is_empty() || !self.target.contains('/') {
            return None;
        }
        Some(match self.kind {
            LinkKind::Note => format!("{}.txt", self.target),
            LinkKind::Attachment => self.target.clone(),
        })
    }
}

/// Rozhodne, jestli cíl odkazuje na přílohu.
///
/// Za příponu považujeme krátký alfanumerický suffix poslední části cesty.
/// `.txt` je vždy poznámka; `[[Verze 1.2]]` příponu nemá (číslice nestačí).
fn classify(target: &str) -> (LinkKind, String) {
    let last = target.rsplit('/').next().unwrap_or(target);
    let Some((stem, ext)) = last.rsplit_once('.') else {
        return (LinkKind::Note, target.to_string());
    };
    let looks_like_ext = !stem.is_empty()
        && !ext.is_empty()
        && ext.len() <= 10
        && ext.chars().all(|c| c.is_ascii_alphanumeric())
        && ext.chars().any(|c| c.is_ascii_alphabetic());
    if !looks_like_ext {
        return (LinkKind::Note, target.to_string());
    }
    if ext.eq_ignore_ascii_case("txt") {
        // Přípona se u poznámek nepíše, ale když ji někdo napíše, odstraníme ji.
        let trimmed = &target[..target.len() - ext.len() - 1];
        return (LinkKind::Note, trimmed.to_string());
    }
    (LinkKind::Attachment, target.to_string())
}

fn parse_anchor(raw: &str) -> Option<Anchor> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    match raw.strip_prefix('^') {
        Some(id) if !id.is_empty() => Some(Anchor::BlockId(id.to_string())),
        _ => Some(Anchor::Section(raw.to_string())),
    }
}

/// Najde všechny wiki odkazy v obsahu.
pub fn parse(content: &str, base: usize) -> Vec<WikiLink> {
    let line_index = lines(content, base);
    let mut out: Vec<WikiLink> = Vec::new();
    let mut cursor = 0usize;

    while let Some(rel_open) = content[cursor..].find("[[") {
        let open = cursor + rel_open;
        let inner_start = open + 2;
        let Some(rel_close) = content[inner_start..].find("]]") else {
            break;
        };
        let inner_end = inner_start + rel_close;
        let close_end = inner_end + 2;
        let inner = &content[inner_start..inner_end];

        // Odkaz nesmí přeskočit konec řádku — `[[` bez uzávěrky by jinak
        // spojilo dva nesouvisející řádky.
        if inner.contains('\n') {
            cursor = inner_start;
            continue;
        }

        let embed = content[..open].ends_with('!');
        let span_start = if embed { open - 1 } else { open };

        let (target_part, label) = match inner.split_once('|') {
            Some((t, l)) => (t, Some(l.trim().to_string())),
            None => (inner, None),
        };
        let (target_raw, anchor_raw) = match target_part.split_once('#') {
            Some((t, a)) => (t, Some(a)),
            None => (target_part, None),
        };

        let target_raw = target_raw.trim();
        let (kind, target) = if target_raw.is_empty() {
            (LinkKind::Note, String::new())
        } else {
            classify(target_raw)
        };

        // Zcela prázdné `[[]]` odkazem není.
        let anchor = anchor_raw.and_then(parse_anchor);
        if target.is_empty() && anchor.is_none() {
            cursor = close_end;
            continue;
        }

        let line = line_index
            .iter()
            .rev()
            .find(|l| base + span_start >= l.start)
            .map(|l| l.number)
            .unwrap_or(1);

        out.push(WikiLink {
            span: Span::new(base + span_start, base + close_end),
            raw: inner.to_string(),
            target,
            anchor,
            label,
            embed,
            kind,
            line,
        });

        cursor = close_end;
    }

    out
}
