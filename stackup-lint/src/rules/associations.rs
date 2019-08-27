use super::ObjectDefn;
use crate::interface::{Comment, PositionedComment, Severity};
use graphql_parser::{
    self,
    query::Type,
    schema::{Definition, Field},
    Pos,
};
use heck::MixedCase;
use std::collections::HashMap;

pub(crate) fn check_associations(defns: &[Definition]) -> Vec<PositionedComment> {
    let object_defns: Vec<_> = defns.iter().filter_map(ObjectDefn::new).collect();
    let object_defns_map: HashMap<_, _> = object_defns
        .iter()
        .map(|defn| defn.name)
        .zip(&object_defns)
        .collect();

    let fields_with_associations: Vec<_> = object_defns
        .iter()
        .flat_map(|defn| {
            defn.fields
                .iter()
                .map(move |f| (f, defn.name, defn.position))
        })
        .filter(|(f, _, _)| extract_field_type_name(&object_defns_map, &f).is_some())
        .collect();

    let mut comments = Vec::new();
    comments.append(&mut check_belongs_to(&fields_with_associations));
    comments.append(&mut check_fields_for_association(
        &fields_with_associations,
        &object_defns_map,
    ));

    comments
}

fn check_belongs_to<'a>(
    fields_with_associations: &[(&'a Field, &'a String, &'a Pos)],
) -> Vec<PositionedComment> {
    fields_with_associations
        .iter()
        .filter(|(f, _, _)| !f.directives.iter().any(|d| &d.name == "belongsTo"))
        .map(|(f, _, _)| {
            let message = r#"Missing "@belongsTo" directive"#;
            let comment = Comment::new(Severity::Error, message.to_string());
            PositionedComment::new(f.position, comment)
        })
        .collect()
}

fn check_fields_for_association<'a>(
    fields_with_associations: &[(&'a Field, &'a String, &'a Pos)],
    object_defns: &HashMap<&String, &'a ObjectDefn>,
) -> Vec<PositionedComment> {
    fields_with_associations
        .iter()
        .filter_map(|(field, f_object_name, f_object_pos)| {
            extract_field_type_name(object_defns, &field).and_then(|f_type_name| {
                object_defns
                    .get(f_type_name)
                    .map(|defn| ((field, *f_object_name, f_object_pos), *defn))
            })
        })
        .filter_map(|((_, f_object_name, f_object_pos), object_defn)| {
            let plural_field_name = if f_object_name.ends_with('s') {
                (f_object_name.clone() + "es").to_mixed_case()
            } else {
                (f_object_name.clone() + "s").to_mixed_case()
            };

            if !object_defn
                .fields
                .iter()
                .any(|f| f.name == plural_field_name)
            {
                let message = format!(
                    r#"Missing field "{}", due to association on object type {} - {}\n"#,
                    plural_field_name, f_object_name, f_object_pos
                );
                let comment = Comment::new(Severity::Error, message.to_string());
                Some(PositionedComment::new(*object_defn.position, comment))
            } else {
                None
            }
        })
        .collect()
}

fn extract_field_type_name<'a>(
    object_defns_map: &HashMap<&String, &'a ObjectDefn>,
    f: &'a Field,
) -> Option<&'a String> {
    match f.field_type {
        Type::NamedType(ref type_name) if object_defns_map.contains_key(type_name) => {
            Some(type_name)
        }
        Type::NonNullType(ref inner_type) => match **inner_type {
            Type::NamedType(ref type_name) if object_defns_map.contains_key(type_name) => {
                Some(type_name)
            }
            _ => None,
        },
        _ => None,
    }
}
