//! A document's interactive form: where each field is drawn, what it holds, and writing the values
//! a person filled in. Reading is by the form's own data (pdfrum-doc), because the facade hands
//! out a field's value and not the places that draw it; writing is the facade's, which turns each
//! value into the field's `/V`, the widget's `/AS` and a fresh appearance, as an incremental update.

use crate::document::PdfDocument;
use crate::error::PdfError;
use crate::geometry::displayed;
use anyview_core::{FieldFill, FormWidget, PageIndex, PageRect, WidgetAccess, WidgetKind};
use pdfrum::{SaveOptions, Subtype};
use pdfrum_common::{Diagnostics, Limits};
use pdfrum_doc::form::{Field, FieldKind, Form, field_attr};
use pdfrum_object::{Dict, Name, Object, Resolve, names};

/// A choice field's options (`/Opt`): pdfrum-doc keeps its own copy of the key private.
const OPT: Name = Name::from_static(b"Opt");

/// The state name that is a clear check box or radio button.
const OFF: &str = "Off";

/// Where a document's fields are drawn, page by page and in the order the document lists the
/// widgets on each page (the order the Tab key walks). Empty when the document has no form, and
/// for a field that holds no value (a push button, a signature).
#[must_use]
pub fn form_widgets(doc: &PdfDocument) -> Vec<FormWidget> {
    let parser = doc.inner().parser();
    let Ok(catalog) = parser.catalog() else {
        return Vec::new();
    };
    let mut diagnostics = Diagnostics::default();
    let Some(form) = Form::load(&catalog, parser, &Limits::default(), &mut diagnostics) else {
        return Vec::new();
    };
    if form.fields.is_empty() {
        return Vec::new();
    }
    let mut widgets = Vec::new();
    for index in 0..doc.page_count().get() {
        let page = PageIndex(index);
        let Ok(sheet) = doc.page(page) else {
            continue;
        };
        let (crop, rotation) = (sheet.crop_box(), sheet.rotation());
        for annotation in sheet.annotations() {
            if annotation.subtype() != Subtype::Widget || annotation.is_hidden() {
                continue;
            }
            let Some(field) = form.fields.iter().find(|field| {
                field
                    .widgets
                    .iter()
                    .any(|widget| &widget.dict == annotation.dict())
            }) else {
                continue;
            };
            let rect = displayed(annotation.rect(), crop, rotation);
            if let Some(widget) = widget_of(field, annotation.dict(), page, rect, parser) {
                widgets.push(widget);
            }
        }
    }
    widgets
}

/// The widget `dict` of `field`, drawn at `rect` of `page`; `None` for a field with no value to
/// give.
fn widget_of(
    field: &Field,
    dict: &Dict,
    page: PageIndex,
    rect: PageRect,
    r: &impl Resolve,
) -> Option<FormWidget> {
    let kind = match field.kind {
        FieldKind::Text if field.flags.is_password() => WidgetKind::Secret,
        FieldKind::Text if field.flags.is_multiline() => WidgetKind::TextArea,
        FieldKind::Text => WidgetKind::Text,
        FieldKind::Check => WidgetKind::Check,
        FieldKind::Radio => WidgetKind::Radio,
        FieldKind::Combo => WidgetKind::Choice,
        FieldKind::List => WidgetKind::List,
        FieldKind::Button | FieldKind::Signature => return None,
    };
    let access = if field.flags.is_read_only() {
        WidgetAccess::ReadOnly
    } else {
        WidgetAccess::Editable
    };
    let on = if kind.is_toggle() {
        on_state(dict, r)
    } else {
        String::new()
    };
    let value = field.value(None, r);
    let options = if matches!(kind, WidgetKind::Choice | WidgetKind::List) {
        field.options(r)
    } else {
        Vec::new()
    };
    let exports = exports_of(field, &options, r);
    let value = if kind.is_toggle() && value.is_empty() {
        OFF.to_owned()
    } else {
        value
    };
    Some(FormWidget {
        name: field.name.clone(),
        kind,
        access,
        page,
        rect,
        value,
        on,
        options,
        exports,
    })
}

/// The value each of `options` writes: the export value of an option written as a pair in the
/// field's `/Opt` (its own or one it inherits from a parent, as `options` reads it), else the option
/// itself.
fn exports_of(field: &Field, options: &[String], r: &impl Resolve) -> Vec<String> {
    let (limits, mut diags) = (Limits::default(), Diagnostics::default());
    let opt = field_attr(&field.dict, &OPT, r, &limits, &mut diags);
    let pairs = opt.as_ref().and_then(Object::as_array);
    options
        .iter()
        .enumerate()
        .map(|(at, label)| {
            let export = pairs.as_ref().and_then(|opt| {
                let entry = opt.get(at, r)?;
                match entry.get() {
                    Object::Array(pair) => pair.raw_at(0).map(Object::to_text),
                    Object::Dict(_)
                    | Object::Null
                    | Object::Bool(_)
                    | Object::Int(_)
                    | Object::Real(_)
                    | Object::Str(_)
                    | Object::Name(_)
                    | Object::Stream(_)
                    | Object::Ref(_) => None,
                }
            });
            export.unwrap_or_else(|| label.clone())
        })
        .collect()
}

/// The state that means a check box or radio button is on, from its appearance dictionary (the
/// one state that is not `Off`); `Yes` when it names none.
fn on_state(widget: &Dict, r: &impl Resolve) -> String {
    widget
        .dict(names::AP, r)
        .and_then(|appearance| appearance.dict(names::N, r))
        .and_then(|normal| {
            normal
                .keys()
                .map(|key| String::from_utf8_lossy(key.as_bytes()).into_owned())
                .find(|state| state != OFF)
        })
        .unwrap_or_else(|| "Yes".to_owned())
}

/// `doc` with `fills` written into its fields, as the bytes of the file with the change appended
/// after the original (the original stays byte for byte at the front, which a signature needs).
/// A field the document does not have is [`PdfError::NoField`] and nothing is written.
pub(crate) fn filled(doc: &PdfDocument, fills: &[FieldFill]) -> Result<Vec<u8>, PdfError> {
    let mut form = doc.inner().form().ok_or_else(|| PdfError::NoField {
        name: fills
            .first()
            .map(|fill| fill.name.clone())
            .unwrap_or_default(),
    })?;
    for fill in fills {
        form.set(&fill.name, fill.value.clone())
            .map_err(|unknown| PdfError::NoField { name: unknown.name })?;
    }
    let mut out = Vec::new();
    doc.inner().write_form_to(
        &mut out,
        &form,
        &SaveOptions::builder().incremental().build(),
    )?;
    Ok(out)
}
