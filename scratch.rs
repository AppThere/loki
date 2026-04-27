use loro::*;
fn test(doc: LoroDoc) {
    let t = doc.get_text("t");
    let d = t.to_delta();
    for span in d {
        match span {
            TextDelta::Insert { insert, attributes } => {
                let _i: String = insert;
            }
            _ => {}
        }
    }
}
