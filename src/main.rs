use std::{cell::RefMut, io};

use markup5ever_rcdom::{Handle, NodeData, RcDom, SerializableHandle};
use rusttype::{point, Font, Scale};
use xml5ever::{
    driver::parse_document, namespace_url, serialize::serialize, tendril::TendrilSink, Attribute,
};

// Needed for "Open Sans" for some reason ¯\_(ツ)_/¯
const FIXED_OPEN_SANS_FONT_SCALE_FACTOR: f32 = 1.361_816_5;

fn main() {
    let font = {
        let font_data = include_bytes!("../OpenSans-VariableFont_wdth,wght.ttf");
        Font::try_from_bytes(font_data).expect("error constructing a Font from bytes")
    };

    let stdin = io::stdin();
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut stdin.lock())
        .unwrap();

    walk(&dom.document, &font);

    let document: SerializableHandle = dom.document.clone().into();
    serialize(io::stdout(), &document, Default::default()).expect("serialization failed");
}

fn walk(handle: &Handle, font: &Font) {
    if let NodeData::Element {
        ref name,
        ref attrs,
        ..
    } = handle.data
    {
        if name.local.as_bytes() == b"text" {
            let mut attrs = attrs.borrow_mut();

            {
                if let Some(id_attr) = attrs
                    .iter_mut()
                    .find(|attr| attr.name.local.as_bytes() == b"id")
                {
                    let id = remove_numerical_suffix(id_attr.value.as_bytes());

                    let alignment = if id.ends_with(b"left") {
                        Some(Alignment::Left)
                    } else if id.ends_with(b"center") {
                        Some(Alignment::Center)
                    } else if id.ends_with(b"right") {
                        Some(Alignment::Right)
                    } else {
                        None
                    };
                    if let Some(alignment) = alignment {
                        align_text_element(handle, &mut attrs, alignment, font);
                    }
                };
            }
        }
    }

    for child in handle.children.borrow().iter() {
        walk(child, font);
    }
}

enum Alignment {
    Left,
    Center,
    Right,
}

fn align_text_element(
    handle: &Handle,
    handle_attrs: &mut RefMut<Vec<Attribute>>,
    alignment: Alignment,
    font: &Font,
) {
    handle.children.borrow().iter().for_each(|child_node| {
        if let NodeData::Element {
            ref name,
            ref attrs,
            ..
        } = child_node.data
        {
            if name.local.as_bytes() == b"tspan" {
                let mut children = child_node.children.borrow_mut();

                if children.len() > 1 {
                    eprintln!("Warning: tspan element has more than one child. Skipping...");
                    return;
                }
                if children.len() == 0 {
                    eprintln!("Warning: tspan element has no children. Skipping...");
                    return;
                }

                let child_text_node = &mut children[0];
                if let NodeData::Text { ref contents } = child_text_node.data {
                    let contents = contents.borrow_mut();
                    let text = contents.to_string();
                    let font_size = handle_attrs
                        .iter()
                        .find_map(|attr| {
                            if attr.name.local.as_bytes() == b"font-size" {
                                Some(attr.value.parse::<f32>().unwrap())
                            } else {
                                None
                            }
                        })
                        .unwrap_or(16.0);
                    let width = calculate_text_width(
                        font,
                        font_size,
                        FIXED_OPEN_SANS_FONT_SCALE_FACTOR,
                        &text,
                    );
                    dbg!(width);
                    if let Some(x_attr) = attrs
                        .borrow_mut()
                        .iter_mut()
                        .find(|attr| attr.name.local.as_bytes() == b"x")
                    {
                        let x = x_attr.value.parse::<f32>().unwrap();
                        let new_x = match alignment {
                            Alignment::Left => x,
                            Alignment::Center => x + width / 2.0,
                            Alignment::Right => x + width,
                        };
                        x_attr.value = new_x.to_string().into();
                    };

                    let text_anchor_value = match alignment {
                        Alignment::Left => "start",
                        Alignment::Center => "middle",
                        Alignment::Right => "end",
                    };
                    attrs.borrow_mut().push(xml5ever::Attribute {
                        name: xml5ever::QualName::new(
                            None,
                            xml5ever::ns!(),
                            xml5ever::LocalName::from("text-anchor"),
                        ),
                        value: xml5ever::tendril::Tendril::from(text_anchor_value),
                    });

                    if let Some((i, style_attr)) = handle_attrs
                        .iter_mut()
                        .enumerate()
                        .find(|(_, attr)| attr.name.local.as_bytes() == b"style")
                    {
                        let style = style_attr.value.to_string();
                        // FUCK white-space!
                        if style == "white-space: pre" {
                            handle_attrs.remove(i);
                        }
                    };
                } else {
                    eprintln!("Warning: tspan element has a non-text child? Skipping...");
                }
            }
        }
    });
}

fn remove_numerical_suffix(s: &[u8]) -> &[u8] {
    let mut i = s.len();

    // Remove the digits
    // test_123 -> test_
    while i > 0 && s[i - 1].is_ascii_digit() {
        i -= 1;
    }

    // Remove the _ suffix if it exists
    // test_ -> test
    if i > 0 && s[i - 1] == b'_' {
        i -= 1;
    }

    &s[..i]
}

fn calculate_text_width(font: &Font, font_size: f32, fixed_scale_factor: f32, text: &str) -> f32 {
    let scale = Scale::uniform(font_size * fixed_scale_factor);
    let start = point(0.0, 0.0);
    let glyphs: Vec<_> = font.layout(text, scale, start).collect();

    let width: f32 = glyphs.iter().fold(0.0, |acc, glyph| {
        acc + glyph.unpositioned().h_metrics().advance_width
    });
    width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_numerical_suffix() {
        assert_eq!(remove_numerical_suffix(b"foo"), b"foo");
        assert_eq!(remove_numerical_suffix(b"foo1"), b"foo");
        assert_eq!(remove_numerical_suffix(b"foo123"), b"foo");
        assert_eq!(remove_numerical_suffix(b"foo_123"), b"foo");
        assert_eq!(remove_numerical_suffix(b"foo_"), b"foo");
        assert_eq!(remove_numerical_suffix(b"f_oo_"), b"f_oo");
    }
}
