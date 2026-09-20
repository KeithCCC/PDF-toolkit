//! Conservative geometry analysis. Failure disables downsampling, never discards content.
use anyhow::{Context, Result};
use lopdf::{content::Content, Dictionary, Document, Object, ObjectId};
use std::collections::{HashMap, HashSet};

type Matrix = [f64; 6];
const IDENTITY: Matrix = [1., 0., 0., 1., 0., 0.];
fn matrix(values: &[Object]) -> Result<Matrix> {
    anyhow::ensure!(values.len() == 6, "配置行列が不正です");
    let mut result = IDENTITY;
    for (out, value) in result.iter_mut().zip(values) {
        *out = value.as_float()? as f64;
        anyhow::ensure!(out.is_finite(), "配置行列が不正です");
    }
    Ok(result)
}
fn multiply(a: Matrix, b: Matrix) -> Matrix {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}
fn resolve<'a>(doc: &'a Document, object: &'a Object) -> Result<&'a Object> {
    Ok(doc.dereference(object)?.1)
}
fn resources(doc: &Document, page: ObjectId) -> Result<Dictionary> {
    let mut id = page;
    let mut seen = HashSet::new();
    loop {
        anyhow::ensure!(seen.insert(id), "ページ親参照が循環しています");
        let dict = doc.get_object(id)?.as_dict()?;
        if let Ok(value) = dict.get(b"Resources") {
            return Ok(resolve(doc, value)?.as_dict()?.clone());
        }
        let Ok(parent) = dict.get(b"Parent") else {
            return Ok(Dictionary::new());
        };
        id = parent.as_reference()?;
    }
}
struct Scan<'a> {
    doc: &'a Document,
    sizes: HashMap<ObjectId, (f64, f64)>,
    active: HashSet<ObjectId>,
    operations: usize,
}
impl Scan<'_> {
    fn walk(
        &mut self,
        bytes: &[u8],
        resources: &Dictionary,
        initial: Matrix,
        depth: usize,
    ) -> Result<()> {
        anyhow::ensure!(depth <= 32, "Formの入れ子が深いため解像度を保持しました");
        // Type3 glyphs and patterns can draw an image through paths other than Do.
        if let Ok(fonts) = resources.get(b"Font") {
            for (_, font) in resolve(self.doc, fonts)?.as_dict()?.iter() {
                let font = resolve(self.doc, font)?.as_dict()?;
                anyhow::ensure!(
                    font.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Type3"),
                    "Type3フォントを含むため解像度を保持しました"
                );
            }
        }
        anyhow::ensure!(
            !resources.has(b"Pattern"),
            "パターンを含むため解像度を保持しました"
        );
        let mut ctm = initial;
        let mut stack = Vec::new();
        for op in Content::decode(bytes)?.operations {
            self.operations += 1;
            anyhow::ensure!(
                self.operations <= 1_000_000,
                "配置の解析上限を超えたため解像度を保持しました"
            );
            match op.operator.as_str() {
                "q" => stack.push(ctm),
                "Q" => ctm = stack.pop().context("画像配置の復元が不正です")?,
                "cm" => ctm = multiply(ctm, matrix(&op.operands)?),
                "Do" => {
                    let name = op
                        .operands
                        .first()
                        .context("XObject名がありません")?
                        .as_name()?;
                    let objects = resolve(self.doc, resources.get(b"XObject")?)?.as_dict()?;
                    let id = objects.get(name)?.as_reference()?;
                    let stream = self.doc.get_object(id)?.as_stream()?;
                    match stream.dict.get(b"Subtype")?.as_name()? {
                        b"Image" => {
                            let (w, h) = (ctm[0].hypot(ctm[1]), ctm[2].hypot(ctm[3]));
                            anyhow::ensure!(
                                w.is_finite() && h.is_finite() && w > 0. && h > 0.,
                                "画像配置の寸法が不正です"
                            );
                            let size = self.sizes.entry(id).or_insert((0., 0.));
                            size.0 = size.0.max(w);
                            size.1 = size.1.max(h);
                        }
                        b"Form" => {
                            anyhow::ensure!(self.active.insert(id), "Form参照が循環しています");
                            let transform = match stream.dict.get(b"Matrix") {
                                Ok(value) => matrix(value.as_array()?)?,
                                Err(_) => IDENTITY,
                            };
                            let own_resources = stream
                                .dict
                                .get(b"Resources")
                                .ok()
                                .map(|r| -> Result<&Dictionary> {
                                    Ok(resolve(self.doc, r)?.as_dict()?)
                                })
                                .transpose()?;
                            let content = stream.get_plain_content_with_limit(32 * 1024 * 1024)?;
                            self.walk(
                                &content,
                                own_resources.unwrap_or(resources),
                                multiply(ctm, transform),
                                depth + 1,
                            )?;
                            self.active.remove(&id);
                        }
                        _ => anyhow::bail!("未対応XObjectのため解像度を保持しました"),
                    }
                }
                _ => {}
            }
        }
        anyhow::ensure!(stack.is_empty(), "画像配置の保存と復元が一致しません");
        Ok(())
    }
}

pub(crate) fn image_sizes(doc: &Document) -> Result<HashMap<ObjectId, (f64, f64)>> {
    let mut scan = Scan {
        doc,
        sizes: HashMap::new(),
        active: HashSet::new(),
        operations: 0,
    };
    for id in doc.get_pages().values() {
        let dict = doc.get_object(*id)?.as_dict()?;
        let unit = match dict.get(b"UserUnit") {
            Ok(value) => value.as_float()? as f64,
            Err(_) => 1.,
        };
        anyhow::ensure!(unit.is_finite() && unit > 0., "UserUnitが不正です");
        scan.walk(
            &doc.get_page_content_with_limit(*id, 32 * 1024 * 1024)?,
            &resources(doc, *id)?,
            [unit, 0., 0., unit, 0., 0.],
            0,
        )?;
    }
    Ok(scan.sizes)
}
