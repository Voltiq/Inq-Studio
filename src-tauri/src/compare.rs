//! Comparing two documents, and exporting text.
//!
//! Text is compared word by word. Pages are matched first (pages with the
//! same words pair up, whatever was inserted or removed around them), then
//! the words of each pair are aligned; what only the first document has is
//! "removed", what only the second has is "added". Every difference comes
//! with the place it is drawn at, so both documents can be shown side by
//! side with the differences marked.

use crate::interp::{scan_page, TextLine};
use crate::pages;
use lopdf::Document;
use serde::Serialize;
use std::collections::HashMap;

type Quad = [[f32; 2]; 4];

#[derive(Clone)]
struct Word {
    text: String,
    quad: Quad,
}

fn lerp(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// The words of a line, each with the part of the line's box it covers.
fn line_words(l: &TextLine, out: &mut Vec<Word>) {
    let chars: Vec<char> = l.text.chars().collect();
    let width = l.chars.last().copied().unwrap_or(0.0).max(1e-3);
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < chars.len() && !chars[j].is_whitespace() {
            j += 1;
        }
        let a = l.chars.get(i).copied().unwrap_or(0.0) / width;
        let b = l.chars.get(j).copied().unwrap_or(width) / width;
        let q = l.quad;
        out.push(Word {
            text: chars[i..j].iter().collect(),
            quad: [lerp(q[0], q[1], a), lerp(q[0], q[1], b), lerp(q[3], q[2], b), lerp(q[3], q[2], a)],
        });
        i = j;
    }
}

fn page_words(doc: &Document, index: usize) -> Vec<Word> {
    let mut out = Vec::new();
    if let Ok(pid) = pages::page_id(doc, index) {
        if let Ok(scan) = scan_page(doc, pid) {
            for l in &scan.lines {
                line_words(l, &mut out);
            }
        }
    }
    out
}

/// Longest common subsequence of two sequences, as matched index pairs.
/// Common ends are stripped first; what is left is solved exactly when it
/// is small enough and greedily (unique words as anchors) otherwise.
fn lcs<T: PartialEq + std::hash::Hash + Eq + Clone>(a: &[T], b: &[T]) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    let mut start = 0;
    while start < a.len() && start < b.len() && a[start] == b[start] {
        pairs.push((start, start));
        start += 1;
    }
    let (mut ea, mut eb) = (a.len(), b.len());
    let mut tail = Vec::new();
    while ea > start && eb > start && a[ea - 1] == b[eb - 1] {
        ea -= 1;
        eb -= 1;
        tail.push((ea, eb));
    }
    let (ma, mb) = (&a[start..ea], &b[start..eb]);
    if !ma.is_empty() && !mb.is_empty() {
        if ma.len() * mb.len() <= 6_000_000 {
            let (n, m) = (ma.len(), mb.len());
            let w = m + 1;
            let mut t = vec![0u32; (n + 1) * w];
            for i in (0..n).rev() {
                for j in (0..m).rev() {
                    t[i * w + j] = if ma[i] == mb[j] { t[(i + 1) * w + j + 1] + 1 } else { t[(i + 1) * w + j].max(t[i * w + j + 1]) };
                }
            }
            let (mut i, mut j) = (0, 0);
            while i < n && j < m {
                if ma[i] == mb[j] {
                    pairs.push((start + i, start + j));
                    i += 1;
                    j += 1;
                } else if t[(i + 1) * w + j] >= t[i * w + j + 1] {
                    i += 1;
                } else {
                    j += 1;
                }
            }
        } else {
            // Too large for the exact table: anchor on items that occur once in both, in order.
            let mut count_a: HashMap<&T, (usize, usize)> = HashMap::new();
            for (i, x) in ma.iter().enumerate() {
                let e = count_a.entry(x).or_insert((0, i));
                e.0 += 1;
            }
            let mut count_b: HashMap<&T, (usize, usize)> = HashMap::new();
            for (j, x) in mb.iter().enumerate() {
                let e = count_b.entry(x).or_insert((0, j));
                e.0 += 1;
            }
            let mut anchors: Vec<(usize, usize)> = count_a
                .iter()
                .filter(|(_, (n, _))| *n == 1)
                .filter_map(|(k, (_, i))| count_b.get(k).filter(|(n, _)| *n == 1).map(|(_, j)| (*i, *j)))
                .collect();
            anchors.sort();
            // Longest increasing run of anchors by position in `b`.
            let mut best: Vec<usize> = vec![1; anchors.len()];
            let mut prev: Vec<Option<usize>> = vec![None; anchors.len()];
            let mut tails: Vec<usize> = Vec::new();
            for k in 0..anchors.len() {
                let pos = tails.partition_point(|&t| anchors[t].1 < anchors[k].1);
                if pos > 0 {
                    prev[k] = Some(tails[pos - 1]);
                    best[k] = pos + 1;
                }
                if pos == tails.len() {
                    tails.push(k);
                } else {
                    tails[pos] = k;
                }
            }
            let mut chain = Vec::new();
            let mut cur = tails.last().copied();
            while let Some(k) = cur {
                chain.push(anchors[k]);
                cur = prev[k];
            }
            chain.reverse();
            let mut last = (0usize, 0usize);
            for (i, j) in chain.into_iter().chain(std::iter::once((ma.len(), mb.len()))) {
                if i > last.0 && j > last.1 && (i - last.0) * (j - last.1) <= 6_000_000 {
                    for (x, y) in lcs(&ma[last.0..i], &mb[last.1..j]) {
                        pairs.push((start + last.0 + x, start + last.1 + y));
                    }
                }
                if i < ma.len() {
                    pairs.push((start + i, start + j));
                }
                last = (i + 1, j + 1);
            }
        }
    }
    tail.reverse();
    pairs.extend(tail);
    pairs
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PagePair {
    /// Page index in the first document, when the page exists there.
    pub a: Option<usize>,
    pub b: Option<usize>,
    /// Places of words only the first document has (in its page).
    pub removed: Vec<Quad>,
    /// Places of words only the second document has (in its page).
    pub added: Vec<Quad>,
    pub words_removed: usize,
    pub words_added: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    /// "added", "removed" or "replaced"
    pub kind: String,
    /// Index into `pairs`.
    pub pair: usize,
    pub before: String,
    pub after: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub pairs: Vec<PagePair>,
    pub changes: Vec<Change>,
    pub words_added: usize,
    pub words_removed: usize,
    pub pages_added: usize,
    pub pages_removed: usize,
    /// The text of both documents is the same, page for page.
    pub same_text: bool,
}

fn clip(words: &[&str]) -> String {
    let s = words.join(" ");
    if s.chars().count() > 160 {
        format!("{}…", s.chars().take(159).collect::<String>())
    } else {
        s
    }
}

pub fn compare(a: &Document, b: &Document) -> Result<Comparison, String> {
    let (na, nb) = (a.get_pages().len(), b.get_pages().len());
    let wa: Vec<Vec<Word>> = (0..na).map(|i| page_words(a, i)).collect();
    let wb: Vec<Vec<Word>> = (0..nb).map(|i| page_words(b, i)).collect();
    let key = |w: &Vec<Word>| w.iter().map(|x| x.text.as_str()).collect::<Vec<_>>().join(" ");
    let ka: Vec<String> = wa.iter().map(key).collect();
    let kb: Vec<String> = wb.iter().map(key).collect();

    // Pages with identical words anchor the pairing; pages between two anchors pair up in order.
    let anchors = lcs(&ka, &kb);
    let mut order: Vec<(Option<usize>, Option<usize>)> = Vec::new();
    let mut last = (0usize, 0usize);
    for (i, j) in anchors.into_iter().chain(std::iter::once((na, nb))) {
        let (ga, gb) = (i - last.0, j - last.1);
        for k in 0..ga.max(gb) {
            order.push(((k < ga).then_some(last.0 + k), (k < gb).then_some(last.1 + k)));
        }
        if i < na {
            order.push((Some(i), Some(j)));
        }
        last = (i + 1, j + 1);
    }

    let mut pairs = Vec::new();
    let mut changes = Vec::new();
    let (mut total_added, mut total_removed, mut pages_added, mut pages_removed) = (0, 0, 0, 0);
    for (pa, pb) in order {
        let empty = Vec::new();
        let left = pa.map(|i| &wa[i]).unwrap_or(&empty);
        let right = pb.map(|j| &wb[j]).unwrap_or(&empty);
        let mut pair = PagePair { a: pa, b: pb, ..Default::default() };
        match (pa, pb) {
            (Some(_), None) => pages_removed += 1,
            (None, Some(_)) => pages_added += 1,
            _ => {}
        }
        let lt: Vec<&str> = left.iter().map(|w| w.text.as_str()).collect();
        let rt: Vec<&str> = right.iter().map(|w| w.text.as_str()).collect();
        let matched = if pa.is_some() && pb.is_some() { lcs(&lt, &rt) } else { Vec::new() };
        let mut last = (0usize, 0usize);
        for (i, j) in matched.into_iter().chain(std::iter::once((left.len(), right.len()))) {
            let (gone, new) = (&left[last.0..i], &right[last.1..j]);
            if !gone.is_empty() || !new.is_empty() {
                pair.removed.extend(gone.iter().map(|w| w.quad));
                pair.added.extend(new.iter().map(|w| w.quad));
                let kind = if gone.is_empty() {
                    "added"
                } else if new.is_empty() {
                    "removed"
                } else {
                    "replaced"
                };
                if changes.len() < 2000 {
                    changes.push(Change {
                        kind: kind.into(),
                        pair: pairs.len(),
                        before: clip(&gone.iter().map(|w| w.text.as_str()).collect::<Vec<_>>()),
                        after: clip(&new.iter().map(|w| w.text.as_str()).collect::<Vec<_>>()),
                    });
                }
            }
            last = (i + 1, j + 1);
        }
        pair.words_removed = pair.removed.len();
        pair.words_added = pair.added.len();
        total_added += pair.words_added;
        total_removed += pair.words_removed;
        pairs.push(pair);
    }
    Ok(Comparison {
        same_text: total_added == 0 && total_removed == 0 && pages_added == 0 && pages_removed == 0,
        pairs,
        changes,
        words_added: total_added,
        words_removed: total_removed,
        pages_added,
        pages_removed,
    })
}

// ---------------------------------------------------------------------------
// Text export
// ---------------------------------------------------------------------------

/// The text of one page: paragraphs in the order the page draws them, one
/// per line, a blank line where the gap between them is more than a line.
pub fn page_text(doc: &Document, index: usize) -> Result<String, String> {
    let pid = pages::page_id(doc, index)?;
    let scan = scan_page(doc, pid)?;
    let mut out = String::new();
    let mut last_bottom: Option<(f32, f32)> = None;
    for b in &scan.blocks {
        let Some(first) = b.lines.first().map(|&i| &scan.lines[i]) else { continue };
        let top = first.quad[3][1].max(first.quad[0][1]);
        let bottom = b.quad[0][1].min(b.quad[3][1]);
        if let Some((prev_bottom, em)) = last_bottom {
            if !out.is_empty() && (prev_bottom - top) > 0.9 * em.max(first.em) {
                out.push('\n');
            }
        }
        out.push_str(b.text.trim_end());
        out.push('\n');
        last_bottom = Some((bottom, first.em));
    }
    Ok(out)
}

/// The text of several pages, separated by form feeds.
pub fn document_text(doc: &Document, pages: &[usize]) -> Result<String, String> {
    let mut out = String::new();
    for (n, &p) in pages.iter().enumerate() {
        if n > 0 {
            out.push('\u{c}');
            out.push('\n');
        }
        out.push_str(&page_text(doc, p)?);
    }
    Ok(out)
}
