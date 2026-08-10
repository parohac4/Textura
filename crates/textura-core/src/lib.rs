//! Parser formátu poznámek Textury.
//!
//! Poznámka je obyčejný UTF-8 `.txt` soubor. Parser je čistá funkce nad textem
//! — nepotřebuje index ani žádný skrytý stav, takže poznámku upravenou zvenčí
//! (Vim, Syncthing, shell skript) přečte stejně jako vlastní.
//!
//! ```
//! use textura_core::parse;
//!
//! let note = parse("@title: Docker\n\nDocker\n\n[[MariaDB]]\n\n#docker\n");
//! assert_eq!(note.title(), Some("Docker"));
//! assert_eq!(note.links[0].target, "MariaDB");
//! assert_eq!(note.tags[0].name, "docker");
//! ```

pub mod comments;
pub mod links;
pub mod properties;
pub mod sections;
pub mod tags;
pub mod tasks;
pub mod text;
pub mod urls;

pub use comments::Comment;
pub use links::{Anchor, LinkKind, WikiLink};
pub use properties::Property;
pub use sections::Section;
pub use tags::Tag;
pub use tasks::{Task, TaskAttribute, TaskState};
pub use text::{LineEnding, Span};
pub use urls::Url;

/// Rozparsovaná poznámka. Všechny spany jsou byte offsety do původního vstupu
/// včetně případného BOM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub properties: Vec<Property>,
    pub sections: Vec<Section>,
    pub links: Vec<WikiLink>,
    pub tags: Vec<Tag>,
    pub tasks: Vec<Task>,
    pub urls: Vec<Url>,
    pub comments: Vec<Comment>,
    /// Konce řádků, jak byly v souboru — při ukládání se zachovávají.
    pub line_ending: LineEnding,
    /// Soubor začínal BOM. Nové soubory se zapisují bez něj.
    pub had_bom: bool,
    /// První řádek těla poznámky za blokem properties (1-based).
    pub body_line: usize,
}

impl Note {
    /// Hodnota property, case-insensitive podle klíče. První výskyt vyhrává.
    pub fn property(&self, key: &str) -> Option<&Property> {
        let key = key.to_lowercase();
        self.properties.iter().find(|p| p.key_lower() == key)
    }

    /// `@title`, pokud je uvedený. Jinak název poznámky určuje název souboru.
    pub fn title(&self) -> Option<&str> {
        self.property("title").map(|p| p.value.as_str())
    }

    /// `@aliases: A, B` → `["A", "B"]`.
    pub fn aliases(&self) -> Vec<String> {
        self.property("aliases")
            .map(|p| p.list_values())
            .unwrap_or_default()
    }

    /// Tagy z těla poznámky sloučené s tagy z `@tags:`, bez duplicit,
    /// v pořadí prvního výskytu. Porovnává se case-insensitive.
    pub fn all_tags(&self) -> Vec<String> {
        let from_property = self
            .property("tags")
            .map(|p| p.list_values())
            .unwrap_or_default()
            .into_iter()
            .map(|t| t.trim_start_matches('#').to_string());

        let mut seen: Vec<String> = Vec::new();
        let mut out: Vec<String> = Vec::new();
        for tag in from_property.chain(self.tags.iter().map(|t| t.name.clone())) {
            let key = tag.to_lowercase();
            if tag.is_empty() || seen.contains(&key) {
                continue;
            }
            seen.push(key);
            out.push(tag);
        }
        out
    }

    /// Sekce podle nadpisu, case-insensitive.
    pub fn section(&self, title: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.matches(title))
    }

    /// Sekce podle stabilního ID `^id`.
    pub fn section_by_id(&self, id: &str) -> Option<&Section> {
        self.sections
            .iter()
            .find(|s| s.block_id.as_deref() == Some(id))
    }

    /// Odkazy mířící na jiné poznámky (bez embedů příloh a bez čistě
    /// vnitřních odkazů) — vstup pro budování grafu backlinků.
    pub fn note_links(&self) -> impl Iterator<Item = &WikiLink> {
        self.links
            .iter()
            .filter(|l| l.kind == LinkKind::Note && !l.is_internal())
    }

    /// Odkazy na přílohy.
    pub fn attachments(&self) -> impl Iterator<Item = &WikiLink> {
        self.links.iter().filter(|l| l.kind == LinkKind::Attachment)
    }
}

/// Rozparsuje obsah poznámky.
pub fn parse(input: &str) -> Note {
    let had_bom = input.starts_with(text::BOM);
    let base = if had_bom { text::BOM.len_utf8() } else { 0 };
    let content = &input[base..];
    let line_ending = text::detect_line_ending(content);

    // Komentáře se z textu vymaskují (se zachováním byte offsetů), aby se
    // jejich obsah neinterpretoval jako odkazy, tagy nebo úkoly.
    let comments = comments::parse(content, base);
    let comment_spans: Vec<Span> = comments.iter().map(|c| c.span).collect();
    let masked = text::mask_spans(content, &comment_spans, base);

    let lines = text::lines(&masked, base);
    let (properties, body_line) = properties::parse(&lines);
    let end = base + masked.len();

    // Uvnitř wiki odkazu se `#` ani `http://` samostatně neinterpretuje.
    let links = links::parse(&masked, base);
    let link_spans: Vec<Span> = links.iter().map(|l| l.span).collect();
    let urls = urls::parse(&masked, base, &link_spans);

    let mut tag_exclude = link_spans;
    tag_exclude.extend(urls.iter().map(|u| u.span));

    let body_lines: Vec<text::Line<'_>> = lines
        .iter()
        .copied()
        .filter(|l| l.number >= body_line)
        .collect();

    Note {
        sections: sections::parse(&body_lines, end),
        tags: tags::parse(&masked, base, &tag_exclude),
        tasks: tasks::parse(&body_lines),
        properties,
        links,
        urls,
        comments,
        line_ending,
        had_bom,
        body_line,
    }
}
