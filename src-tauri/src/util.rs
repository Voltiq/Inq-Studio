//! Small helpers shared by the PDF modules: object access that follows
//! references, numbers, and 2D affine matrices in PDF's row-vector form.

use lopdf::{Dictionary, Document, Object, ObjectId};

/// `[a b c d e f]`, as in the `cm` and `Tm` operators.
pub type Matrix = [f32; 6];

pub const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `a × b`: apply `a` first, then `b`.
pub fn mul(a: &Matrix, b: &Matrix) -> Matrix {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

pub fn apply(m: &Matrix, x: f32, y: f32) -> (f32, f32) {
    (x * m[0] + y * m[2] + m[4], x * m[1] + y * m[3] + m[5])
}

/// Applies only the linear part (for direction vectors).
pub fn apply_vec(m: &Matrix, x: f32, y: f32) -> (f32, f32) {
    (x * m[0] + y * m[2], x * m[1] + y * m[3])
}

pub fn invert(m: &Matrix) -> Option<Matrix> {
    let det = m[0] * m[3] - m[1] * m[2];
    if det.abs() < 1e-9 {
        return None;
    }
    let (a, b, c, d) = (m[3] / det, -m[1] / det, -m[2] / det, m[0] / det);
    Some([a, b, c, d, -(m[4] * a + m[5] * c), -(m[4] * b + m[5] * d)])
}

pub fn translate(tx: f32, ty: f32) -> Matrix {
    [1.0, 0.0, 0.0, 1.0, tx, ty]
}

pub fn len(v: (f32, f32)) -> f32 {
    (v.0 * v.0 + v.1 * v.1).sqrt()
}

/// A number operand, integer or real.
pub fn num(o: &Object) -> Option<f32> {
    match o {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r),
        _ => None,
    }
}

/// Follows a reference (a few levels at most) to the object behind it.
pub fn deref<'a>(doc: &'a Document, mut o: &'a Object) -> &'a Object {
    for _ in 0..8 {
        match o {
            Object::Reference(id) => match doc.get_object(*id) {
                Ok(next) => o = next,
                Err(_) => return &Object::Null,
            },
            _ => return o,
        }
    }
    o
}

pub fn dict<'a>(doc: &'a Document, o: &'a Object) -> Option<&'a Dictionary> {
    match deref(doc, o) {
        Object::Dictionary(d) => Some(d),
        Object::Stream(s) => Some(&s.dict),
        _ => None,
    }
}

pub fn get<'a>(doc: &'a Document, d: &'a Dictionary, key: &[u8]) -> Option<&'a Object> {
    d.get(key).ok().map(|o| deref(doc, o))
}

pub fn get_num(doc: &Document, d: &Dictionary, key: &[u8]) -> Option<f32> {
    get(doc, d, key).and_then(num)
}

pub fn get_name(doc: &Document, d: &Dictionary, key: &[u8]) -> Option<String> {
    match get(doc, d, key)? {
        Object::Name(n) => Some(String::from_utf8_lossy(n).to_string()),
        _ => None,
    }
}

pub fn get_array<'a>(doc: &'a Document, d: &'a Dictionary, key: &[u8]) -> Option<&'a Vec<Object>> {
    match get(doc, d, key)? {
        Object::Array(a) => Some(a),
        _ => None,
    }
}

pub fn ref_id(d: &Dictionary, key: &[u8]) -> Option<ObjectId> {
    d.get(key).ok().and_then(|o| o.as_reference().ok())
}

/// The decoded bytes of a stream object.
pub fn stream_bytes(doc: &Document, o: &Object) -> Option<Vec<u8>> {
    match deref(doc, o) {
        Object::Stream(s) => {
            if s.dict.get(b"Filter").is_ok() {
                s.decompressed_content().ok().or_else(|| Some(s.content.clone()))
            } else {
                Some(s.content.clone())
            }
        }
        _ => None,
    }
}

/// An inheritable page attribute: the page's own value or its nearest ancestor's.
pub fn inherited<'a>(doc: &'a Document, page_id: ObjectId, key: &[u8]) -> Option<&'a Object> {
    let mut current = doc.get_dictionary(page_id).ok()?;
    for _ in 0..64 {
        if let Ok(v) = current.get(key) {
            return Some(deref(doc, v));
        }
        let parent = current.get(b"Parent").ok()?.as_reference().ok()?;
        current = doc.get_dictionary(parent).ok()?;
    }
    None
}

pub fn rect(doc: &Document, o: &Object) -> Option<[f32; 4]> {
    let Object::Array(a) = deref(doc, o) else { return None };
    if a.len() != 4 {
        return None;
    }
    let v: Vec<f32> = a.iter().filter_map(|x| num(deref(doc, x))).collect();
    if v.len() != 4 {
        return None;
    }
    Some([v[0].min(v[2]), v[1].min(v[3]), v[0].max(v[2]), v[1].max(v[3])])
}

/// Rounds for writing into content streams, so files don't fill with float noise.
pub fn real(v: f32) -> Object {
    let r = (v * 10000.0).round() / 10000.0;
    if r.fract() == 0.0 && r.abs() < 1e9 {
        Object::Integer(r as i64)
    } else {
        Object::Real(r)
    }
}

pub fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrices() {
        let m = mul(&translate(10.0, 5.0), &[2.0, 0.0, 0.0, 2.0, 1.0, 1.0]);
        assert_eq!(apply(&m, 0.0, 0.0), (21.0, 11.0));
        let inv = invert(&m).unwrap();
        let (x, y) = apply(&inv, 21.0, 11.0);
        assert!(x.abs() < 1e-4 && y.abs() < 1e-4);
    }
}
